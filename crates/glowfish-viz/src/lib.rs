//! Visualization of spectrograms (3D) and signals (2D) with a timeline to select the displayed
//! time window. Add [`VisualizationPlugin`] and insert a [`Dataset`] resource to show data;
//! replacing the resource shows another dataset.

mod axes;
mod camera;
mod colormap;
mod config;
mod dataset;
mod format;
mod hud;
mod indicator;
mod labels;
mod layout;
mod loading;
mod signal;
mod spectrogram;
mod timeline;

use bevy::prelude::*;
use bevy::winit::WinitSettings;

pub use camera::{BlocksViewInput, ViewRig};
pub use config::{
    CellValue, Colormap, Controls, DisplayMode, Layout, Perspective, VisualizationConfig,
};
pub use dataset::{DataKind, Dataset, Selection};
pub use labels::{Labels, fill};
pub use loading::{DataPlugin, DataSystems, DisplayedGrid, Loading, grid_size};
pub use spectrogram::SpectrogramMaterial;

/// Render into this image instead of the window, e.g. for offscreen exports. Insert it before the
/// dataset; the image needs `RENDER_ATTACHMENT` usage (`Image::new_target_texture`).
#[derive(Resource, Debug, Clone)]
pub struct RenderToImage(pub Handle<Image>);

/// Everything spawned for the current dataset; despawned when the dataset changes.
#[derive(Component, Default)]
pub struct VisualizationEntity;

/// The 3D/2D view, timeline and HUD for the [`Dataset`] resource.
///
/// Renders reactively: frames are only drawn on input or while data is loading.
pub struct VisualizationPlugin;

/// Fira Mono Medium, the full version of Bevy's built-in font, which only covers ASCII.
/// SIL Open Font License, see `assets/fonts/FiraMono-OFL.txt`.
const FONT: &[u8] = include_bytes!("../assets/fonts/FiraMono-Medium.ttf");

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        // every text without an explicit font uses the default font handle
        if let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() {
            fonts
                .insert(AssetId::default(), Font::from_bytes(FONT.to_vec()))
                .expect("the default font id is valid");
        }
        app.add_systems(Startup, load_custom_font);
        app.insert_resource(WinitSettings::desktop_app())
            .insert_resource(ClearColor(Color::srgb(0.09, 0.09, 0.1)))
            .init_resource::<VisualizationConfig>()
            .add_plugins((
                DataPlugin,
                axes::AxesPlugin,
                camera::ViewCameraPlugin,
                spectrogram::SpectrogramPlugin,
                signal::SignalPlugin,
                timeline::TimelinePlugin,
                hud::HudPlugin,
                indicator::IndicatorPlugin,
            ))
            .add_systems(
                First,
                despawn_previous.run_if(resource_exists_and_changed::<Dataset>),
            )
            .add_systems(Last, end_rebuild.run_if(resource_exists::<Rebuild>));
    }
}

/// Replaces the default font by `labels.font` from the configuration.
fn load_custom_font(config: Res<VisualizationConfig>, fonts: Option<ResMut<Assets<Font>>>) {
    let (Some(path), Some(mut fonts)) = (&config.labels.font, fonts) else {
        return;
    };
    match std::fs::read(path) {
        Ok(bytes) => {
            fonts
                .insert(AssetId::default(), Font::from_bytes(bytes))
                .expect("the default font id is valid");
        }
        Err(error) => error!("cannot read the font {}: {error}", path.display()),
    }
}

/// Present for one frame after the dataset changed: the entities of the previous dataset are
/// gone and the systems that spawn the scene run.
///
/// Spawning is tied to this marker rather than to the dataset's change detection, so a dataset
/// inserted in the middle of a frame (e.g. by a file selection) is not spawned and then
/// despawned again at the start of the next frame.
#[derive(Resource)]
pub(crate) struct Rebuild;

///
/// The dataset usually changed in an earlier frame (e.g. in `Update` after a file selection), so
/// the selection and the grid of the new dataset were already set when the scene is spawned.
/// They are marked as changed again, so that the systems that update the new entities (placing
/// the plot, the timeline window and labels, the legend) run for them. Without this, a new plot
/// keeps its identity transform until the data finished loading, which stretches it over the
/// whole window.
fn despawn_previous(
    mut commands: Commands,
    selection: Option<ResMut<Selection>>,
    displayed: Option<ResMut<DisplayedGrid>>,
) {
    commands.despawn_all::<With<VisualizationEntity>>();
    commands.insert_resource(Rebuild);
    if let Some(mut selection) = selection {
        selection.set_changed();
    }
    if let Some(mut displayed) = displayed {
        displayed.set_changed();
    }
}

fn end_rebuild(mut commands: Commands) {
    commands.remove_resource::<Rebuild>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loading::tests::dataset;

    #[derive(Resource, Default)]
    struct Seen(Vec<(bool, bool)>);

    /// Records whether the selection and the grid count as changed while the scene is spawned.
    /// Runs every frame, so changes are relative to the previous frame like in the render systems.
    fn record(
        mut seen: ResMut<Seen>,
        rebuild: Option<Res<Rebuild>>,
        selection: Option<Res<Selection>>,
        displayed: Option<Res<DisplayedGrid>>,
    ) {
        if rebuild.is_none() {
            return;
        }
        seen.0.push((
            selection.is_some_and(|s| s.is_changed()),
            displayed.is_some_and(|d| d.is_changed()),
        ));
    }

    #[test]
    fn a_dataset_opened_during_update_is_spawned_with_changed_data() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, DataPlugin))
            .init_resource::<VisualizationConfig>()
            .init_resource::<Seen>()
            .add_systems(
                First,
                despawn_previous.run_if(resource_exists_and_changed::<Dataset>),
            )
            .add_systems(Update, record.after(DataSystems))
            .add_systems(Last, end_rebuild.run_if(resource_exists::<Rebuild>));
        app.insert_resource(dataset(DataKind::Spectrogram, 1_000, 4));
        for _ in 0..5 {
            app.update();
        }
        // a file selection inserts the next dataset in `Update` before the data systems (like the
        // folder panel); the scene follows a frame later
        app.add_systems(
            Update,
            (|mut commands: Commands, mut done: Local<bool>| {
                if !std::mem::replace(&mut *done, true) {
                    commands.insert_resource(dataset(DataKind::Spectrogram, 2_000, 4));
                }
            })
            .before(DataSystems),
        );
        for _ in 0..3 {
            app.update();
        }
        let seen = &app.world().resource::<Seen>().0;
        assert_eq!(seen.len(), 2, "the scene is spawned once per dataset");
        assert_eq!(seen[1], (true, true));
    }
}
