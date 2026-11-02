use std::ops::Range;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use bevy::window::RequestRedraw;
use glowfish_data::{DataError, Grid, LoadControl, MinMaxCell};

use crate::{DataKind, Dataset, Selection, VisualizationConfig};

/// Most cells along the time axis of a signal plot; about one per pixel.
const MAX_SIGNAL_CELLS: usize = 4096;
/// Most cells along the time axis of a spectrogram. The data texture is one cell wider and
/// must stay within the texture size limit of every GPU (4096 is safe); more cells than
/// screen pixels would not show more anyway. Files with few bands hit this limit first.
const MAX_SPECTROGRAM_CELLS: usize = 4095;

/// Keeps the displayed grid in sync with the [`Selection`]: loads in the background, shows the
/// cached part of the data immediately and cancels loads that are no longer needed.
pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loading>()
            .add_message::<RequestRedraw>()
            .add_systems(
                Update,
                (
                    reset_on_new_dataset.run_if(resource_exists_and_changed::<Dataset>),
                    request_load,
                    poll_load,
                )
                    .chain()
                    .in_set(DataSystems)
                    .run_if(resource_exists::<Dataset>),
            );
    }
}

/// Systems that update [`DisplayedGrid`]; render systems run after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataSystems;

/// The grid currently shown.
#[derive(Resource, Debug, Clone)]
pub struct DisplayedGrid {
    pub grid: Grid<MinMaxCell>,
    /// False while it only contains the cached part of the request and loading continues.
    pub complete: bool,
}

/// The state of background loading.
#[derive(Resource, Default)]
pub struct Loading {
    task: Option<Task<Result<Grid<MinMaxCell>, DataError>>>,
    control: LoadControl,
    request: Option<Request>,
    /// When the running load started.
    started: Option<std::time::Instant>,
}

impl Loading {
    /// Progress `0.0..=1.0` of the running load, `None` when idle.
    pub fn progress(&self) -> Option<f32> {
        self.task.as_ref().map(|_| self.control.progress())
    }

    fn cancel(&mut self) {
        self.control.cancel();
        self.task = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    range: Range<u64>,
    time_cells: usize,
    freq_cells: usize,
}

/// Time and frequency cells for at most `max_cells` cells. Spectrograms get a 4:1 ratio, so the
/// default budget of 65536 gives 512 × 128.
pub fn grid_size(kind: DataKind, max_cells: usize, bands: usize) -> (usize, usize) {
    match kind {
        DataKind::Signal => (max_cells.clamp(1, MAX_SIGNAL_CELLS), 1),
        DataKind::Spectrogram => {
            let freq = ((max_cells as f64 / 4.0).sqrt() as usize).clamp(1, bands.max(1));
            ((max_cells / freq).clamp(1, MAX_SPECTROGRAM_CELLS), freq)
        }
    }
}

fn reset_on_new_dataset(
    mut commands: Commands,
    dataset: Res<Dataset>,
    mut loading: ResMut<Loading>,
    selection: Option<Res<Selection>>,
) {
    loading.cancel();
    loading.request = None;
    commands.remove_resource::<DisplayedGrid>();
    // a selection inserted together with the dataset (e.g. from the command line) is kept
    let selection = selection.map_or(Selection::all(dataset.rows()), |s| *s);
    commands.insert_resource(selection.clipped(dataset.rows()));
}

fn request_load(
    mut commands: Commands,
    dataset: Res<Dataset>,
    selection: Option<Res<Selection>>,
    config: Res<VisualizationConfig>,
    mut loading: ResMut<Loading>,
) {
    let Some(selection) = selection else { return };
    let (time_cells, freq_cells) = grid_size(dataset.kind, config.max_cells, dataset.bands());
    let request = Request {
        range: selection.rows(),
        time_cells,
        freq_cells,
    };
    if loading.request.as_ref() == Some(&request) {
        return;
    }
    loading.cancel();

    // show what is cached right away; placeholders mark the rest
    let peeked = dataset
        .pyramid
        .peek(request.range.clone(), time_cells, freq_cells);
    let complete = peeked.cells.iter().all(|c| !c.is_empty());
    commands.insert_resource(DisplayedGrid {
        grid: peeked,
        complete,
    });

    if !complete {
        let control = LoadControl::new();
        let pyramid = dataset.pyramid.clone();
        let (task_control, range) = (control.clone(), request.range.clone());
        loading.task = Some(AsyncComputeTaskPool::get().spawn(async move {
            pyramid.query_with(range, time_cells, freq_cells, &task_control)
        }));
        loading.control = control;
        loading.started = Some(std::time::Instant::now());
    }
    loading.request = Some(request);
}

/// How often the view shows what is loaded so far while a load runs.
const PROGRESS_INTERVAL: f32 = 0.25;

fn poll_load(
    mut commands: Commands,
    time: Res<Time>,
    dataset: Res<Dataset>,
    mut loading: ResMut<Loading>,
    displayed: Option<Res<DisplayedGrid>>,
    mut redraw: MessageWriter<RequestRedraw>,
    mut since_progress: Local<f32>,
) {
    let Some(task) = loading.task.as_mut() else {
        return;
    };
    let Some(result) = block_on(poll_once(task)) else {
        // reactive rendering only wakes on input; keep updating until the data is there
        redraw.write(RequestRedraw);
        // show what is loaded so far, e.g. for large files on slow disks
        *since_progress += time.delta_secs();
        if *since_progress >= PROGRESS_INTERVAL
            && let Some(request) = &loading.request
        {
            *since_progress = 0.0;
            let peeked = dataset.pyramid.peek(
                request.range.clone(),
                request.time_cells,
                request.freq_cells,
            );
            let loaded =
                |grid: &Grid<MinMaxCell>| grid.cells.iter().filter(|c| !c.is_empty()).count();
            if displayed.is_none_or(|d| loaded(&peeked) > loaded(&d.grid)) {
                commands.insert_resource(DisplayedGrid {
                    grid: peeked,
                    complete: false,
                });
            }
        }
        return;
    };
    loading.task = None;
    *since_progress = 0.0;
    let elapsed = loading.started.take().map(|s| s.elapsed());
    match result {
        Ok(grid) => {
            if let Some(elapsed) = elapsed {
                debug!(
                    "loaded rows {:?} of {} in {elapsed:.2?}",
                    grid.range, dataset.name
                );
            }
            commands.insert_resource(DisplayedGrid {
                grid,
                complete: true,
            })
        }
        Err(DataError::Cancelled) => {}
        Err(error) => {
            error!("loading data failed: {error}");
            // allow retrying the same request
            loading.request = None;
        }
    }
    redraw.write(RequestRedraw);
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use glowfish_data::{LoadPool, Pyramid, Source};

    use super::*;

    pub(crate) struct Ramp {
        pub rows: u64,
        pub bands: usize,
    }

    impl Source for Ramp {
        fn rows(&self) -> u64 {
            self.rows
        }

        fn bands(&self) -> usize {
            self.bands
        }

        fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError> {
            for row in range {
                out.extend((0..self.bands).map(|band| (row + band as u64) as f32));
            }
            Ok(())
        }
    }

    pub(crate) fn dataset(kind: DataKind, rows: u64, bands: usize) -> Dataset {
        Dataset {
            name: "ramp".into(),
            kind,
            pyramid: Arc::new(Pyramid::new(
                Box::new(Ramp { rows, bands }),
                64 << 20,
                LoadPool::new(2),
            )),
            row_duration: 1e6,
            frq_start: 0.0,
            frq_per_band: 100.0,
            full_scale: 1e6,
        }
    }

    fn app(dataset: Dataset) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, DataPlugin))
            .init_resource::<VisualizationConfig>()
            .insert_resource(dataset);
        app
    }

    fn wait_for_complete(app: &mut App) -> Grid<MinMaxCell> {
        for _ in 0..1000 {
            app.update();
            if let Some(displayed) = app.world().get_resource::<DisplayedGrid>()
                && displayed.complete
            {
                return displayed.grid.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("loading did not complete");
    }

    /// A ramp that takes 10 ms per read.
    struct SlowRamp(Ramp);

    impl Source for SlowRamp {
        fn rows(&self) -> u64 {
            self.0.rows()
        }

        fn bands(&self) -> usize {
            self.0.bands()
        }

        fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError> {
            std::thread::sleep(std::time::Duration::from_millis(10));
            self.0.read(range, out)
        }
    }

    #[test]
    fn shows_partial_data_while_loading() {
        let mut slow = dataset(DataKind::Spectrogram, 1, 1);
        slow.pyramid = Arc::new(Pyramid::new(
            Box::new(SlowRamp(Ramp {
                rows: 300_000,
                bands: 4,
            })),
            64 << 20,
            LoadPool::new(2),
        ));
        let mut app = app(slow);
        let mut partial = false;
        for _ in 0..2000 {
            app.update();
            if let Some(displayed) = app.world().get_resource::<DisplayedGrid>() {
                let loaded = displayed.grid.cells.iter().any(|c| !c.is_empty());
                if !displayed.complete && loaded {
                    partial = true;
                    break;
                }
                if displayed.complete {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(partial, "no partial data was shown while loading");
    }

    #[test]
    fn grid_sizes() {
        assert_eq!(grid_size(DataKind::Spectrogram, 65536, 512), (512, 128));
        assert_eq!(grid_size(DataKind::Spectrogram, 65536, 16), (4095, 16));
        // few bands would make the data texture wider than GPUs allow
        assert_eq!(grid_size(DataKind::Spectrogram, 65536, 2), (4095, 2));
        assert_eq!(grid_size(DataKind::Spectrogram, 65536, 1), (4095, 1));
        assert_eq!(grid_size(DataKind::Signal, 65536, 1), (MAX_SIGNAL_CELLS, 1));
        assert_eq!(grid_size(DataKind::Spectrogram, 0, 16), (1, 1));
    }

    #[test]
    fn loads_the_whole_dataset_initially() {
        let mut app = app(dataset(DataKind::Spectrogram, 100_000, 8));
        let grid = wait_for_complete(&mut app);
        assert_eq!(grid.range, 0..100_000);
        assert_eq!(
            *app.world().resource::<Selection>(),
            Selection::all(100_000)
        );
        // the last cell holds the largest value of the ramp
        let last = grid.get(grid.time_cells - 1, grid.freq_cells - 1).unwrap();
        assert_eq!(last.max, (99_999 + 7) as f32);
    }

    #[test]
    fn follows_the_selection() {
        let mut app = app(dataset(DataKind::Spectrogram, 100_000, 8));
        wait_for_complete(&mut app);
        app.insert_resource(Selection::new(1_000.0, 2_000.5));
        let grid = wait_for_complete(&mut app);
        assert_eq!(grid.range, 1_000..2_001);
        assert!(app.world().resource::<Loading>().progress().is_none());
    }

    #[test]
    fn keeps_a_configured_selection_and_clamps_it() {
        let mut app = app(dataset(DataKind::Signal, 10_000, 1));
        app.insert_resource(Selection::new(9_000.0, 20_000.0));
        let grid = wait_for_complete(&mut app);
        assert_eq!(grid.range, 9_000..10_000);
    }
}
