use std::fmt::Debug;

/// A downsampling method: how samples are reduced into the cells of coarser levels.
///
/// Cells form a monoid: [`EMPTY`](Downsampler::EMPTY) is the cell without data (rendered as a
/// placeholder) and [`combine`](Downsampler::combine) merges two neighbouring cells. `combine`
/// must be associative, and `combine(EMPTY, x) == combine(x, EMPTY) == x`. Arguments are passed in
/// time (or frequency) order, so order-dependent methods such as first/last are possible.
pub trait Downsampler: Send + Sync + 'static {
    type Cell: Copy + Debug + PartialEq + Send + Sync + 'static;

    const EMPTY: Self::Cell;

    /// The cell for a single sample.
    fn leaf(value: f32) -> Self::Cell;

    /// Merges `earlier` with the following `later` cell.
    fn combine(earlier: Self::Cell, later: Self::Cell) -> Self::Cell;

    fn is_empty(cell: &Self::Cell) -> bool {
        *cell == Self::EMPTY
    }

    /// Combines one row of samples into a row of cells, `cells[i] = combine(cells[i],
    /// leaf(samples[i]))`. This is the hot loop when reducing source data; override it with a
    /// faster equivalent where possible.
    fn accumulate(cells: &mut [Self::Cell], samples: &[f32]) {
        for (cell, &value) in cells.iter_mut().zip(samples) {
            *cell = Self::combine(*cell, Self::leaf(value));
        }
    }
}

/// Keeps the minimum and maximum of every cell, so neither peaks nor gaps disappear when
/// zooming out.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinMax;

/// Minimum and maximum of the samples in a cell; `min > max` for an empty cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinMaxCell {
    pub min: f32,
    pub max: f32,
}

impl MinMaxCell {
    pub fn is_empty(&self) -> bool {
        self.min > self.max
    }
}

impl Downsampler for MinMax {
    type Cell = MinMaxCell;

    const EMPTY: MinMaxCell = MinMaxCell {
        min: f32::INFINITY,
        max: f32::NEG_INFINITY,
    };

    fn leaf(value: f32) -> MinMaxCell {
        MinMaxCell {
            min: value,
            max: value,
        }
    }

    fn combine(earlier: MinMaxCell, later: MinMaxCell) -> MinMaxCell {
        MinMaxCell {
            min: earlier.min.min(later.min),
            max: earlier.max.max(later.max),
        }
    }

    fn is_empty(cell: &MinMaxCell) -> bool {
        cell.is_empty()
    }

    fn accumulate(cells: &mut [MinMaxCell], samples: &[f32]) {
        // plain comparisons vectorize, f32::min/max do not; NaN samples are skipped either way
        for (cell, &value) in cells.iter_mut().zip(samples) {
            cell.min = if value < cell.min { value } else { cell.min };
            cell.max = if value > cell.max { value } else { cell.max };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_max_combine() {
        let a = MinMax::leaf(3.0);
        let b = MinMax::leaf(-1.0);
        assert_eq!(
            MinMax::combine(a, b),
            MinMaxCell {
                min: -1.0,
                max: 3.0
            }
        );
        assert_eq!(MinMax::combine(MinMax::EMPTY, a), a);
        assert_eq!(MinMax::combine(a, MinMax::EMPTY), a);
        assert!(MinMax::is_empty(&MinMax::EMPTY));
        assert!(!MinMax::is_empty(&a));
    }

    #[test]
    fn min_max_accumulate_matches_combine() {
        let samples = [3.0, f32::NAN, -2.0, 7.5];
        let mut fast = [
            MinMax::EMPTY,
            MinMax::leaf(1.0),
            MinMax::leaf(0.0),
            MinMax::EMPTY,
        ];
        let mut generic = fast;
        MinMax::accumulate(&mut fast, &samples);
        for (cell, &value) in generic.iter_mut().zip(&samples) {
            *cell = MinMax::combine(*cell, MinMax::leaf(value));
        }
        assert_eq!(fast, generic);
    }
}
