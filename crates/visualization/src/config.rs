use bevy::prelude::*;
use glowfish_data::Scaling;

use crate::Labels;

/// How spectrograms are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    Surface,
    Cubes,
}

/// Which part of a cell's min/max is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellValue {
    /// Columns from zero up to the maximum: keeps peaks.
    #[default]
    Max,
    /// Boxes from the minimum to the maximum: shows peaks and gaps.
    Range,
}

/// What is shown around the plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// Buttons, timeline and hover readout for working with the data.
    #[default]
    Interactive,
    /// Only the plot, legend and labels, e.g. for exported images.
    Report,
}

/// Mouse buttons of the view controls. The default is map style: the data is dragged under a
/// fixed camera with the left button and the view is rotated with the right button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controls {
    pub pan: MouseButton,
    pub rotate: MouseButton,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            pan: MouseButton::Left,
            rotate: MouseButton::Right,
        }
    }
}

/// The camera orientation; angles in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perspective {
    /// Rotation around the vertical axis; 0 looks along the frequency axis.
    pub yaw: f32,
    /// Elevation above the ground plane.
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Perspective {
    fn default() -> Self {
        Self {
            yaw: (-30.0f32).to_radians(),
            pitch: 35.0f32.to_radians(),
            distance: 22.0,
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct VisualizationConfig {
    /// Maximum number of cells (cubes / surface vertices) drawn for a spectrogram.
    pub max_cells: usize,
    pub mode: DisplayMode,
    pub cell_value: CellValue,
    pub scaling: Scaling,
    pub controls: Controls,
    pub perspective: Perspective,
    pub layout: Layout,
    /// Axes with ticks and labels: time and frequency for spectrograms, time and amplitude for
    /// signals.
    pub axes: bool,
    /// Every text shown to users.
    pub labels: Labels,
    /// Width of a panel left of the view in UI pixels (scaled by `UiScale`); the view, HUD and
    /// timeline start right of it. The panel's owner provides a full-window UI camera.
    pub view_left: f32,
}

impl VisualizationConfig {
    /// The left inset in logical window pixels.
    pub fn view_left_px(&self, ui_scale: f32) -> f32 {
        self.view_left * ui_scale
    }
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            max_cells: 512 * 128,
            mode: DisplayMode::default(),
            cell_value: CellValue::default(),
            scaling: Scaling::default(),
            controls: Controls::default(),
            perspective: Perspective::default(),
            layout: Layout::default(),
            axes: false,
            labels: Labels::default(),
            view_left: 0.0,
        }
    }
}
