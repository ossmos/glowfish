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
mod labels;
mod layout;
mod loading;
mod signal;
mod spectrogram;
mod timeline;

use bevy::prelude::*;
use bevy::winit::WinitSettings;

pub use camera::{BlocksViewInput, ViewRig};
pub use config::{CellValue, Controls, DisplayMode, Layout, Perspective, VisualizationConfig};
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
            ))
            .add_systems(
                First,
                despawn_previous.run_if(resource_exists_and_changed::<Dataset>),
            );
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

fn despawn_previous(mut commands: Commands) {
    commands.despawn_all::<With<VisualizationEntity>>();
}
