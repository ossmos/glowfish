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
mod layout;
mod loading;
mod signal;
mod spectrogram;
mod timeline;

use bevy::prelude::*;
use bevy::winit::WinitSettings;

pub use camera::{BlocksViewInput, ViewRig};
pub use config::{
    AxisLabels, CellValue, Controls, DisplayMode, Layout, Perspective, VisualizationConfig,
};
pub use dataset::{DataKind, Dataset, Selection};
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

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
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
            ))
            .add_systems(
                First,
                despawn_previous.run_if(resource_exists_and_changed::<Dataset>),
            );
    }
}

fn despawn_previous(mut commands: Commands) {
    commands.despawn_all::<With<VisualizationEntity>>();
}
