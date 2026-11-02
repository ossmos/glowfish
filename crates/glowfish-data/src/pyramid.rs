use std::cell::RefCell;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use lru::LruCache;
use rayon::prelude::*;

use crate::load::Load;
use crate::{DataError, Downsampler, LoadControl, LoadPool, MinMax, Source};

/// Cells per tile along the time axis.
const TILE_CELLS: u64 = 256;
/// Highest level; a level 55 tile covers 2^63 rows.
const MAX_LEVEL: u32 = 55;
/// Tiles up to this level are reduced directly from source rows instead of being merged from
/// child tiles. Merging every level from single rows costs far more memory traffic than one pass
/// over the samples.
const DIRECT_MAX_LEVEL: u32 = 4;
/// Rows read from the source at once when reducing directly.
const READ_ROWS: u64 = 1024;
/// A query caches tiles of its own level and of this many finer levels, so zooming in a few
/// steps is served from the cache. Finer tiles are intermediate results: a whole-file query
/// produces thousands of them, and caching them would evict the few coarse tiles that matter.
const CACHED_FINER_LEVELS: u32 = 3;

/// A lazily built mipmap over the time axis of a [`Source`].
///
/// A cell at level `l` reduces `2^l` rows with the [`Downsampler`]. Cells are grouped into tiles
/// of [`TILE_CELLS`] cells; a tile is computed on demand, at level 0 from the source and above
/// from its two child tiles, and kept in an LRU cache with a byte budget.
///
/// Tiles only contain the rows that were requested: rows outside of a query are not read, and
/// their cells stay [`Downsampler::EMPTY`]. The frequency axis is reduced when querying.
///
/// Queries run on a [`LoadPool`] and can be cancelled and observed through a [`LoadControl`].
pub struct Pyramid<S, D: Downsampler = MinMax> {
    source: S,
    rows: u64,
    bands: usize,
    cache: Mutex<TileCache<D::Cell>>,
    pool: LoadPool,
}

/// A time × frequency grid of cells, the result of a query.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid<C> {
    /// The queried rows.
    pub range: Range<u64>,
    /// Pyramid level; every time cell covers `2^level` rows.
    pub level: u32,
    /// Index of the first time cell at `level`, it starts at row `first_cell << level`.
    pub first_cell: u64,
    pub time_cells: usize,
    pub freq_cells: usize,
    /// Bands combined into one frequency cell; the last cell may have fewer.
    pub bands_per_cell: usize,
    pub bands: usize,
    /// Row major: `time_cells` rows of `freq_cells` cells.
    pub cells: Vec<C>,
}

impl<C: Copy> Grid<C> {
    pub fn rows_per_cell(&self) -> u64 {
        1 << self.level
    }

    pub fn get(&self, time: usize, freq: usize) -> Option<C> {
        if time >= self.time_cells || freq >= self.freq_cells {
            return None;
        }
        Some(self.cells[time * self.freq_cells + freq])
    }

    /// Rows of time cell `time`. The first and last cell may extend beyond the queried range;
    /// they contain at least the queried rows and the rest of their rows only if those were
    /// already loaded.
    pub fn cell_rows(&self, time: usize) -> Range<u64> {
        let start = (self.first_cell + time as u64) << self.level;
        start..start + self.rows_per_cell()
    }

    /// Bands covered by frequency cell `freq`.
    pub fn cell_bands(&self, freq: usize) -> Range<usize> {
        let start = freq * self.bands_per_cell;
        start..(start + self.bands_per_cell).min(self.bands)
    }
}

impl<S: Source, D: Downsampler> Pyramid<S, D> {
    /// `cache_bytes` limits the memory used by cached tiles; queries run on `pool`.
    pub fn new(source: S, cache_bytes: usize, pool: LoadPool) -> Self {
        let rows = source.rows();
        let bands = source.bands().max(1);
        Self {
            source,
            rows,
            bands,
            cache: Mutex::new(TileCache::new(cache_bytes)),
            pool,
        }
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn rows(&self) -> u64 {
        self.rows
    }

    pub fn bands(&self) -> usize {
        self.bands
    }

    /// Bytes currently used by cached tiles.
    pub fn cached_bytes(&self) -> usize {
        self.cache.lock().expect("cache lock poisoned").bytes
    }

    /// The finest level at which `range` fits into `time_cells` cells.
    pub fn level_for(&self, range: &Range<u64>, time_cells: usize) -> u32 {
        let time_cells = time_cells.max(1) as u64;
        (0..MAX_LEVEL)
            .find(|&level| cell_span(range, level).count() as u64 <= time_cells)
            .unwrap_or(MAX_LEVEL)
    }

    /// Returns `range` reduced to at most `time_cells` (+1 for alignment) × `freq_cells` cells,
    /// reading rows from the source that are not cached yet.
    pub fn query(
        &self,
        range: Range<u64>,
        time_cells: usize,
        freq_cells: usize,
    ) -> Result<Grid<D::Cell>, DataError> {
        self.query_with(range, time_cells, freq_cells, &LoadControl::new())
    }

    /// Like [`query`](Self::query), reporting progress to and stopping on cancellation of
    /// `control`. Blocks until done; call it from a background thread.
    pub fn query_with(
        &self,
        range: Range<u64>,
        time_cells: usize,
        freq_cells: usize,
        control: &LoadControl,
    ) -> Result<Grid<D::Cell>, DataError> {
        let range = self.clamp(range);
        control.start(range.end - range.start);
        let load = Load {
            control,
            query: range.clone(),
            level: self.level_for(&range, time_cells),
        };
        let grid = self
            .pool
            .install(|| self.grid(range, time_cells, freq_cells, Some(&load)))?;
        control.finish();
        Ok(grid)
    }

    /// Like [`query`](Self::query) but only uses cached data and never reads from the source.
    /// Cells without cached data are [`Downsampler::EMPTY`].
    ///
    /// Runs on the calling thread, so it never waits for a load that occupies the pool: it is
    /// meant to be called from a UI thread while loading.
    pub fn peek(&self, range: Range<u64>, time_cells: usize, freq_cells: usize) -> Grid<D::Cell> {
        let range = self.clamp(range);
        self.grid(range, time_cells, freq_cells, None)
            .expect("peeking does not read from the source")
    }

    fn clamp(&self, range: Range<u64>) -> Range<u64> {
        let end = range.end.min(self.rows);
        range.start.min(end)..end
    }

    /// `load` is `None` for peeking.
    fn grid(
        &self,
        range: Range<u64>,
        time_cells: usize,
        freq_cells: usize,
        load: Option<&Load>,
    ) -> Result<Grid<D::Cell>, DataError> {
        let bands_per_cell = self.bands.div_ceil(freq_cells.clamp(1, self.bands));
        let freq_cells = self.bands.div_ceil(bands_per_cell);
        let level = self.level_for(&range, time_cells);
        let cells_range = cell_span(&range, level);
        let mut grid = Grid {
            range: range.clone(),
            level,
            first_cell: cells_range.start,
            time_cells: cells_range.clone().count(),
            freq_cells,
            bands_per_cell,
            bands: self.bands,
            cells: vec![D::EMPTY; cells_range.clone().count() * freq_cells],
        };
        if range.is_empty() {
            return Ok(grid);
        }

        let first_tile = cells_range.start / TILE_CELLS;
        let last_tile = (cells_range.end - 1) / TILE_CELLS;
        let tile = |index| Ok((index, self.tile(level, index, range.clone(), load)?));
        let tiles: Vec<(u64, Option<TileRef<D::Cell>>)> = match load {
            // loading runs on the pool; peeking must not touch it (see `peek`)
            Some(_) => (first_tile..=last_tile)
                .into_par_iter()
                .map(tile)
                .collect::<Result<_, DataError>>()?,
            None => (first_tile..=last_tile)
                .map(tile)
                .collect::<Result<_, DataError>>()?,
        };

        for (index, tile) in tiles {
            let Some(tile) = tile else { continue };
            let tile_cells = index * TILE_CELLS..(index + 1) * TILE_CELLS;
            let start = tile_cells.start.max(cells_range.start);
            let end = tile_cells.end.min(cells_range.end);
            for cell in start..end {
                let row = tile.row((cell - tile_cells.start) as usize, self.bands);
                let out_start = (cell - cells_range.start) as usize * freq_cells;
                let out = &mut grid.cells[out_start..out_start + freq_cells];
                for (out, bands) in out.iter_mut().zip(row.chunks(bands_per_cell)) {
                    *out = bands.iter().copied().fold(D::EMPTY, D::combine);
                }
            }
        }
        Ok(grid)
    }

    fn tile_rows(&self, level: u32, index: u64) -> Range<u64> {
        let span = TILE_CELLS << level;
        let start = index.saturating_mul(span).min(self.rows);
        start..start.saturating_add(span).min(self.rows)
    }

    /// The tile `(level, index)` containing at least the rows `need`. Returns `None` if the tile
    /// has no data: `need` does not overlap it, or nothing is cached and `load` is `None`.
    ///
    /// Progress accounting: every queried row is counted exactly once, either where it is found
    /// in the cache or where it is read.
    fn tile(
        &self,
        level: u32,
        index: u64,
        need: Range<u64>,
        load: Option<&Load>,
    ) -> Result<Option<TileRef<D::Cell>>, DataError> {
        let need = intersect(&need, &self.tile_rows(level, index));
        if need.is_empty() {
            return Ok(None);
        }
        if let Some(load) = load {
            load.control.check()?;
        }
        let cached = self
            .cache
            .lock()
            .expect("cache lock poisoned")
            .get(level, index);
        if let Some(tile) = &cached
            && contains(&tile.known, &need)
        {
            if let Some(load) = load {
                load.advance(&need);
            }
            return Ok(cached);
        }

        let Some(load) = load else {
            // combine whatever is cached below; partial results are not cached
            if cached.is_some() || level == 0 {
                return Ok(cached);
            }
            let earlier = self.tile(level - 1, 2 * index, need.clone(), None)?;
            let later = self.tile(level - 1, 2 * index + 1, need, None)?;
            if earlier.is_none() && later.is_none() {
                return Ok(None);
            }
            let known = hull_of(earlier.as_deref(), later.as_deref());
            return Ok(Some(Arc::new(self.merge(earlier, later, known))));
        };

        // extend a partially cached tile; the gap between both parts is read as well so a tile
        // always covers one contiguous range
        let want = match &cached {
            Some(tile) => hull(&tile.known, &need),
            None => need,
        };
        let tile = if level <= DIRECT_MAX_LEVEL {
            self.direct(level, index, want, cached.as_deref(), load)?
        } else {
            let (earlier, later) = rayon::join(
                || self.tile(level - 1, 2 * index, want.clone(), Some(load)),
                || self.tile(level - 1, 2 * index + 1, want.clone(), Some(load)),
            );
            self.merge(earlier?, later?, want)
        };
        let tile = Arc::new(tile);
        if level + CACHED_FINER_LEVELS >= load.level {
            self.cache
                .lock()
                .expect("cache lock poisoned")
                .insert(level, index, tile.clone());
        }
        Ok(Some(tile))
    }

    /// A tile with the rows `rows` reduced directly from the source. Rows already in `cached`
    /// are not read again.
    fn direct(
        &self,
        level: u32,
        index: u64,
        rows: Range<u64>,
        cached: Option<&Tile<D::Cell>>,
        load: &Load,
    ) -> Result<Tile<D::Cell>, DataError> {
        let Some(cached) = cached else {
            let mut tile = self.empty_tile(rows.clone());
            self.reduce_rows(level, index, rows, &mut tile.cells, load)?;
            return Ok(tile);
        };
        load.advance(&cached.known);
        // rows before and after the cached ones, combined in time order
        let mut before = self.empty_tile(rows.clone());
        self.reduce_rows(
            level,
            index,
            rows.start..cached.known.start,
            &mut before.cells,
            load,
        )?;
        let mut tile = before;
        for (cell, &old) in tile.cells.iter_mut().zip(&cached.cells) {
            *cell = D::combine(*cell, old);
        }
        let mut after = vec![D::EMPTY; tile.cells.len()];
        self.reduce_rows(level, index, cached.known.end..rows.end, &mut after, load)?;
        for (cell, new) in tile.cells.iter_mut().zip(after) {
            *cell = D::combine(*cell, new);
        }
        Ok(tile)
    }

    fn empty_tile(&self, known: Range<u64>) -> Tile<D::Cell> {
        Tile {
            known,
            cells: vec![D::EMPTY; TILE_CELLS as usize * self.bands],
        }
    }

    /// Reads `rows` and combines every sample into its cell of tile `(level, index)`.
    fn reduce_rows(
        &self,
        level: u32,
        index: u64,
        rows: Range<u64>,
        cells: &mut [D::Cell],
        load: &Load,
    ) -> Result<(), DataError> {
        thread_local! {
            // reused so concurrent tiles do not allocate (and page fault) a buffer per read
            static VALUES: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
        }
        let tile_start = index * (TILE_CELLS << level);
        VALUES.with_borrow_mut(|values| {
            let mut start = rows.start;
            while start < rows.end {
                load.control.check()?;
                let end = (start + READ_ROWS).min(rows.end);
                values.clear();
                self.source.read(start..end, values)?;
                for (row, samples) in (start..end).zip(values.chunks_exact(self.bands)) {
                    let cell = ((row - tile_start) >> level) as usize;
                    D::accumulate(
                        &mut cells[cell * self.bands..(cell + 1) * self.bands],
                        samples,
                    );
                }
                load.advance(&(start..end));
                start = end;
            }
            Ok(())
        })
    }

    /// The parent of two child tiles: parent cell `j` combines child cells `2j` and `2j + 1`.
    fn merge(
        &self,
        earlier: Option<TileRef<D::Cell>>,
        later: Option<TileRef<D::Cell>>,
        known: Range<u64>,
    ) -> Tile<D::Cell> {
        let bands = self.bands;
        let half = TILE_CELLS as usize / 2;
        let mut cells = vec![D::EMPTY; TILE_CELLS as usize * bands];
        for (child, parent_cells) in [earlier, later].iter().zip(cells.chunks_mut(half * bands)) {
            let Some(child) = child else { continue };
            for (j, parent) in parent_cells.chunks_mut(bands).enumerate() {
                let (a, b) = (child.row(2 * j, bands), child.row(2 * j + 1, bands));
                for ((parent, &a), &b) in parent.iter_mut().zip(a).zip(b) {
                    *parent = D::combine(a, b);
                }
            }
        }
        Tile { known, cells }
    }
}

type TileRef<C> = Arc<Tile<C>>;

/// Cells of one tile; `TILE_CELLS` rows of `bands` cells.
struct Tile<C> {
    /// Rows the tile was computed from; cells outside of it are empty.
    known: Range<u64>,
    cells: Vec<C>,
}

impl<C> Tile<C> {
    fn row(&self, cell: usize, bands: usize) -> &[C] {
        &self.cells[cell * bands..(cell + 1) * bands]
    }
}

/// Least recently used tiles, limited by the bytes of their cells.
struct TileCache<C> {
    tiles: LruCache<(u32, u64), Arc<Tile<C>>>,
    bytes: usize,
    budget: usize,
}

impl<C> TileCache<C> {
    fn new(budget: usize) -> Self {
        Self {
            tiles: LruCache::unbounded(),
            bytes: 0,
            budget,
        }
    }

    fn size(tile: &Tile<C>) -> usize {
        tile.cells.len() * size_of::<C>()
    }

    fn get(&mut self, level: u32, index: u64) -> Option<Arc<Tile<C>>> {
        self.tiles.get(&(level, index)).cloned()
    }

    fn insert(&mut self, level: u32, index: u64, tile: Arc<Tile<C>>) {
        self.bytes += Self::size(&tile);
        if let Some(old) = self.tiles.put((level, index), tile) {
            self.bytes -= Self::size(&old);
        }
        while self.bytes > self.budget {
            match self.tiles.pop_lru() {
                Some((_, old)) => self.bytes -= Self::size(&old),
                None => break,
            }
        }
    }
}

/// Cell indices at `level` that overlap `range`.
fn cell_span(range: &Range<u64>, level: u32) -> Range<u64> {
    if range.is_empty() {
        return 0..0;
    }
    (range.start >> level)..((range.end - 1) >> level) + 1
}

fn intersect(a: &Range<u64>, b: &Range<u64>) -> Range<u64> {
    let start = a.start.max(b.start);
    start..a.end.min(b.end).max(start)
}

fn contains(outer: &Range<u64>, inner: &Range<u64>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn hull(a: &Range<u64>, b: &Range<u64>) -> Range<u64> {
    a.start.min(b.start)..a.end.max(b.end)
}

fn hull_of<C>(a: Option<&Tile<C>>, b: Option<&Tile<C>>) -> Range<u64> {
    match (a, b) {
        (Some(a), Some(b)) => hull(&a.known, &b.known),
        (Some(t), None) | (None, Some(t)) => t.known.clone(),
        (None, None) => 0..0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::MinMaxCell;

    /// An in-memory source that records which rows were read.
    struct VecSource {
        bands: usize,
        values: Vec<f32>,
        reads: Mutex<Vec<Range<u64>>>,
    }

    impl VecSource {
        fn new(rows: u64, bands: usize, value: impl Fn(u64, usize) -> f32) -> Self {
            let values = (0..rows)
                .flat_map(|r| (0..bands).map(move |b| (r, b)))
                .map(|(r, b)| value(r, b))
                .collect();
            Self {
                bands,
                values,
                reads: Mutex::new(Vec::new()),
            }
        }

        fn take_reads(&self) -> Vec<Range<u64>> {
            std::mem::take(&mut self.reads.lock().unwrap())
        }
    }

    impl Source for VecSource {
        fn rows(&self) -> u64 {
            (self.values.len() / self.bands) as u64
        }

        fn bands(&self) -> usize {
            self.bands
        }

        fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError> {
            self.reads.lock().unwrap().push(range.clone());
            let (start, end) = (range.start as usize, range.end as usize);
            out.extend_from_slice(&self.values[start * self.bands..end * self.bands]);
            Ok(())
        }
    }

    /// Deterministic noise with a single spike and a single gap.
    fn value(row: u64, band: usize) -> f32 {
        let noise = ((row * 7919 + band as u64 * 104_729) % 1000) as f32;
        match (row, band) {
            (123_457, 3) => 1e6,
            (77_777, 10) => -1e6,
            _ => noise,
        }
    }

    /// Min/max over `rows` of the cell computed directly from the source.
    fn brute_force(
        source: &VecSource,
        grid: &Grid<MinMaxCell>,
        rows: Range<u64>,
        f: usize,
    ) -> MinMaxCell {
        let mut cell = MinMax::EMPTY;
        for row in intersect(&rows, &(0..source.rows())) {
            for band in grid.cell_bands(f) {
                let v = source.values[row as usize * source.bands + band];
                cell = MinMax::combine(cell, MinMax::leaf(v));
            }
        }
        cell
    }

    fn assert_matches_brute_force(source: &VecSource, grid: &Grid<MinMaxCell>) {
        for t in 0..grid.time_cells {
            for f in 0..grid.freq_cells {
                let cell = grid.get(t, f).unwrap();
                let full = brute_force(source, grid, grid.cell_rows(t), f);
                let queried =
                    brute_force(source, grid, intersect(&grid.cell_rows(t), &grid.range), f);
                assert!(
                    cell == full || cell == queried,
                    "cell ({t}, {f}) at level {}: {cell:?} is neither {full:?} nor {queried:?}",
                    grid.level
                );
            }
        }
    }

    #[test]
    fn queries_match_brute_force() {
        // odd row and band counts so tiles, cells and band groups do not line up
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(200_003, 13, value),
            usize::MAX,
            LoadPool::new(2),
        );
        for (range, time_cells, freq_cells) in [
            (0..200_003, 100, 4),
            (0..200_003, 1, 1),
            (1_000..1_500, 1000, 13),
            (77_000..124_000, 37, 5),
            (5..6, 10, 3),
            (199_000..200_003, 64, 2),
        ] {
            let grid = pyramid
                .query(range.clone(), time_cells, freq_cells)
                .unwrap();
            assert!(grid.time_cells <= time_cells + 1);
            assert!(grid.freq_cells <= freq_cells);
            assert_matches_brute_force(pyramid.source(), &grid);
        }
    }

    #[test]
    fn keeps_peaks_and_gaps_at_the_coarsest_level() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(200_003, 13, value),
            usize::MAX,
            LoadPool::new(2),
        );
        let grid = pyramid.query(0..200_003, 1, 1).unwrap();
        assert_eq!(grid.time_cells, 1);
        assert_eq!(
            grid.get(0, 0),
            Some(MinMaxCell {
                min: -1e6,
                max: 1e6
            })
        );
    }

    #[test]
    fn reads_only_the_requested_rows() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(200_003, 13, value),
            usize::MAX,
            LoadPool::new(2),
        );
        let range = 50_100..60_200;
        let grid = pyramid.query(range.clone(), 50, 13).unwrap();
        let reads = pyramid.source().take_reads();
        assert!(!reads.is_empty());
        for read in &reads {
            assert!(contains(&range, read), "read {read:?} outside of {range:?}");
        }
        let total: u64 = reads.iter().map(|r| r.end - r.start).sum();
        assert_eq!(total, range.end - range.start);
        assert_matches_brute_force(pyramid.source(), &grid);
    }

    #[test]
    fn cached_queries_do_not_read_again() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(100_000, 8, value),
            usize::MAX,
            LoadPool::new(2),
        );
        pyramid.query(0..100_000, 200, 8).unwrap();
        pyramid.source().take_reads();
        let grid = pyramid.query(10_000..90_000, 150, 4).unwrap();
        assert!(pyramid.source().take_reads().is_empty());
        assert_matches_brute_force(pyramid.source(), &grid);
    }

    #[test]
    fn widening_a_selection_reads_only_the_new_rows() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(100_000, 8, value),
            usize::MAX,
            LoadPool::new(2),
        );
        pyramid.query(40_000..50_000, 100, 8).unwrap();
        pyramid.source().take_reads();
        let grid = pyramid.query(30_000..60_000, 100, 8).unwrap();
        // the new rows plus at most the gaps that make cached tiles contiguous again
        for read in pyramid.source().take_reads() {
            assert!(
                read.end <= 40_000 || read.start >= 50_000,
                "read {read:?} again"
            );
        }
        assert_matches_brute_force(pyramid.source(), &grid);
    }

    #[test]
    fn peek_shows_only_loaded_rows() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(100_000, 8, value),
            usize::MAX,
            LoadPool::new(2),
        );
        assert!(
            pyramid
                .peek(0..100_000, 100, 1)
                .cells
                .iter()
                .all(MinMaxCell::is_empty)
        );

        pyramid.query(20_000..30_000, 100, 1).unwrap();
        pyramid.source().take_reads();
        let grid = pyramid.peek(0..100_000, 100, 1);
        assert!(pyramid.source().take_reads().is_empty());
        for t in 0..grid.time_cells {
            let rows = grid.cell_rows(t);
            let loaded = rows.start < 30_000 && rows.end > 20_000;
            assert_eq!(!grid.get(t, 0).unwrap().is_empty(), loaded, "cell {rows:?}");
        }
    }

    #[test]
    fn small_cache_budget_stays_correct() {
        let tile_bytes = TILE_CELLS as usize * 8 * size_of::<MinMaxCell>();
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(100_000, 8, value),
            3 * tile_bytes,
            LoadPool::new(2),
        );
        for (range, cells) in [(0..100_000, 64), (5_000..9_000, 500), (0..100_000, 7)] {
            let grid = pyramid.query(range, cells, 8).unwrap();
            assert_matches_brute_force(pyramid.source(), &grid);
            assert!(pyramid.cached_bytes() <= 3 * tile_bytes);
        }
    }

    #[test]
    fn whole_file_queries_keep_their_coarse_tiles() {
        // a budget for only a few tiles: fine intermediate tiles must not evict the coarse ones
        let tile_bytes = TILE_CELLS as usize * 4 * size_of::<MinMaxCell>();
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(1_000_000, 4, value),
            8 * tile_bytes,
            LoadPool::new(2),
        );
        pyramid.query(0..1_000_000, 100, 4).unwrap();
        let grid = pyramid.peek(0..1_000_000, 100, 4);
        assert!(grid.cells.iter().all(|c| !c.is_empty()));
        assert_matches_brute_force(pyramid.source(), &grid);
    }

    #[test]
    fn progress_counts_read_and_cached_rows() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(100_000, 8, value),
            usize::MAX,
            LoadPool::new(2),
        );
        let control = LoadControl::new();
        pyramid
            .query_with(20_000..30_000, 100, 8, &control)
            .unwrap();
        assert_eq!(
            (control.rows_done(), control.rows_total()),
            (10_000, 10_000)
        );

        // half cached, half new
        pyramid
            .query_with(25_000..35_000, 100, 8, &control)
            .unwrap();
        assert_eq!(
            (control.rows_done(), control.rows_total()),
            (10_000, 10_000)
        );
        assert_eq!(control.progress(), 1.0);
    }

    /// Cancels its control after `after` reads.
    struct CancellingSource {
        inner: VecSource,
        control: LoadControl,
        after: usize,
    }

    impl Source for CancellingSource {
        fn rows(&self) -> u64 {
            self.inner.rows()
        }

        fn bands(&self) -> usize {
            self.inner.bands()
        }

        fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError> {
            if self.inner.reads.lock().unwrap().len() + 1 >= self.after {
                self.control.cancel();
            }
            self.inner.read(range, out)
        }
    }

    #[test]
    fn cancelled_queries_stop_and_keep_finished_tiles() {
        let control = LoadControl::new();
        let source = CancellingSource {
            inner: VecSource::new(200_000, 4, value),
            control: control.clone(),
            after: 20,
        };
        let pyramid: Pyramid<_> = Pyramid::new(source, usize::MAX, LoadPool::new(2));
        let result = pyramid.query_with(0..200_000, 64, 4, &control);
        assert!(matches!(result, Err(DataError::Cancelled)));
        let reads_before = pyramid.source().inner.take_reads().len();
        assert!(reads_before < 200_000 / READ_ROWS as usize, "stopped early");
        assert!(control.rows_done() < control.rows_total());

        // a new query with a fresh control completes and reuses what was loaded
        let grid = pyramid.query(0..200_000, 64, 4).unwrap();
        assert_matches_brute_force(&pyramid.source().inner, &grid);
    }

    #[test]
    fn already_cancelled_control_fails_immediately() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(1_000, 2, value),
            usize::MAX,
            LoadPool::new(2),
        );
        let control = LoadControl::new();
        control.cancel();
        assert!(matches!(
            pyramid.query_with(0..1_000, 10, 2, &control),
            Err(DataError::Cancelled)
        ));
        assert!(pyramid.source().take_reads().is_empty());
    }

    /// A source that takes `delay` per read, like a slow disk.
    struct SlowSource {
        inner: VecSource,
        delay: std::time::Duration,
    }

    impl Source for SlowSource {
        fn rows(&self) -> u64 {
            self.inner.rows()
        }

        fn bands(&self) -> usize {
            self.inner.bands()
        }

        fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError> {
            std::thread::sleep(self.delay);
            self.inner.read(range, out)
        }
    }

    #[test]
    fn peeking_does_not_wait_for_a_running_load() {
        let source = SlowSource {
            inner: VecSource::new(400_000, 4, value),
            delay: std::time::Duration::from_millis(20),
        };
        let pyramid = Arc::new(Pyramid::<_>::new(source, usize::MAX, LoadPool::new(2)));
        let control = LoadControl::new();
        let loading = {
            let (pyramid, control) = (pyramid.clone(), control.clone());
            // about 400 reads of 20 ms on 2 threads: several seconds
            std::thread::spawn(move || pyramid.query_with(0..400_000, 100, 4, &control))
        };
        std::thread::sleep(std::time::Duration::from_millis(100));

        let started = std::time::Instant::now();
        pyramid.peek(0..400_000, 100, 4);
        let waited = started.elapsed();

        control.cancel();
        assert!(matches!(loading.join().unwrap(), Err(DataError::Cancelled)));
        assert!(
            waited < std::time::Duration::from_millis(50),
            "peek waited {waited:?} for the load"
        );
    }

    #[test]
    fn empty_and_clamped_ranges() {
        let pyramid: Pyramid<_> = Pyramid::new(
            VecSource::new(1_000, 2, value),
            usize::MAX,
            LoadPool::new(2),
        );
        let grid = pyramid.query(500..500, 10, 2).unwrap();
        assert_eq!(grid.time_cells, 0);
        let grid = pyramid.query(900..5_000, 10, 2).unwrap();
        assert_eq!(grid.range, 900..1_000);
        assert_matches_brute_force(pyramid.source(), &grid);
    }
}
