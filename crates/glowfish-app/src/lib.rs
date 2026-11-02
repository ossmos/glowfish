//! Folder mode of glowfish: reads the metadata of all measurement files in a folder and adds a
//! panel to select a project, measurement, channel and file, shown with the visualization.

pub mod catalog;
mod panel;
mod scan;

use std::path::PathBuf;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use glowfish_data::LoadPool;
use visualization::{Dataset, Selection, VisualizationConfig};

pub use catalog::{Catalog, Select};
pub use panel::PANEL_WIDTH;
pub use scan::Database;

use crate::scan::{Scan, poll_scan};

/// A render layer without any content: the panel's cameras only draw UI.
const UI_LAYER: usize = 31;

/// Opens a folder; add after [`visualization::VisualizationPlugin`].
pub struct FolderPlugin {
    pub root: PathBuf,
    /// Include files in subfolders.
    pub recursive: bool,
    pub database: Database,
    /// Memory for cached data per opened file.
    pub cache_bytes: usize,
    /// Threads that load data.
    pub threads: usize,
}

/// How files of the folder are opened.
#[derive(Resource, Clone)]
struct OpenSettings {
    cache_bytes: usize,
    pool: LoadPool,
}

impl Plugin for FolderPlugin {
    fn build(&self, app: &mut App) {
        let root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        if let Some(mut config) = app.world_mut().get_resource_mut::<VisualizationConfig>() {
            config.view_left = PANEL_WIDTH;
        }
        app.insert_resource(OpenSettings {
            cache_bytes: self.cache_bytes,
            pool: LoadPool::new(self.threads),
        })
        .insert_resource(Scan::start(root, self.recursive, self.database.clone()))
        .add_systems(Startup, (spawn_cameras, panel::spawn_panel))
        .add_systems(
            Update,
            (
                poll_scan,
                open_selected.run_if(resource_exists_and_changed::<Catalog>),
                panel::update_panel,
                (panel::style_buttons, panel::scroll_list),
            )
                .chain()
                .before(visualization::DataSystems),
        );
    }
}

/// The view's cameras only cover the part right of the panel: one camera clears the whole
/// window, another draws the UI (panel, HUD, timeline) on top of everything.
fn spawn_cameras(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            ..default()
        },
        RenderLayers::layer(UI_LAYER),
    ));
    commands.spawn((
        Camera2d,
        Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(UI_LAYER),
        IsDefaultUiCamera,
    ));
}

/// Opens the selected file. Within a measurement the displayed time window is kept, so the
/// same moment can be compared in different files; another measurement starts with the whole
/// file.
#[allow(clippy::too_many_arguments)]
fn open_selected(
    mut commands: Commands,
    catalog: Res<Catalog>,
    settings: Res<OpenSettings>,
    dataset: Option<Res<Dataset>>,
    selection: Option<Res<Selection>>,
    mut window: Single<&mut Window>,
    mut opened: Local<Option<(PathBuf, u64, u32)>>,
) {
    let (Some(path), Some(project), Some(measurement)) = (
        catalog.file.clone(),
        catalog.current_project(),
        catalog.current_measurement(),
    ) else {
        return;
    };
    let key = (path.clone(), project.id, measurement.process);
    if opened.as_ref() == Some(&key) {
        return;
    }
    let same_measurement = opened
        .as_ref()
        .is_some_and(|(_, p, m)| (*p, *m) == (project.id, measurement.process));
    let new = match Dataset::open(&path, settings.cache_bytes, settings.pool.clone()) {
        Ok(new) => new,
        Err(error) => {
            error!("cannot open {}: {error}", path.display());
            return;
        }
    };
    // the time window in ms, converted to the rows of the new file
    let window_ms = match (same_measurement, &dataset, &selection) {
        (true, Some(old), Some(selection)) => Some((
            old.row_time_ms(selection.start),
            old.row_time_ms(selection.end),
        )),
        _ => None,
    };
    match window_ms {
        Some((start, end)) => {
            let to_row = |ms: f64| ms * 1e6 / new.row_duration;
            commands.insert_resource(Selection::new(to_row(start), to_row(end)));
        }
        None => commands.remove_resource::<Selection>(),
    }
    window.title = format!("glowfish – {}", new.name);
    commands.insert_resource(new);
    *opened = Some(key);
}
