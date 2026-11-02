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

/// Titles of the axes, e.g. for translations. An empty title is not shown.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisLabels {
    pub time: String,
    /// `{unit}` is replaced by the unit of the frequency ticks (Hz or kHz).
    pub frequency: String,
    pub amplitude: String,
}

impl Default for AxisLabels {
    fn default() -> Self {
        Self {
            time: "time [ms]".into(),
            frequency: "frequency [{unit}]".into(),
            amplitude: "amplitude".into(),
        }
    }
}

impl AxisLabels {
    /// The frequency title for ticks in `unit`.
    pub fn frequency_in(&self, unit: &str) -> String {
        self.frequency.replace("{unit}", unit)
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
    pub axis_labels: AxisLabels,
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
            axis_labels: AxisLabels::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_title_gets_the_unit() {
        let labels = AxisLabels::default();
        assert_eq!(labels.frequency_in("kHz"), "frequency [kHz]");
        let german = AxisLabels {
            frequency: "Frequenz in {unit}".into(),
            ..AxisLabels::default()
        };
        assert_eq!(german.frequency_in("Hz"), "Frequenz in Hz");
    }
}
