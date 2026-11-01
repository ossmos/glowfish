use bevy::prelude::*;
use glowfish_data::Scaling;

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
            yaw: -0.5,
            pitch: 0.6,
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
        }
    }
}
