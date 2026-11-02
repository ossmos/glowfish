//! The configuration file (`~/.config/glowfish/config.toml` on Linux,
//! `%APPDATA%\glowfish\config\config.toml` on Windows). Command line options override it.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use bevy::input::mouse::MouseButton;
use clap::ValueEnum;
use glowfish_data::Scaling;
use serde::{Deserialize, Serialize};
use visualization::{CellValue, Controls, DisplayMode, Labels, Perspective, VisualizationConfig};

/// The default configuration with comments, written by `glowfish config --init`.
pub const DEFAULT_CONFIG: &str = r#"# glowfish configuration. Command line options override these values.

[view]
# "surface" or "cubes"
mode = "surface"
# "max": columns up to the maximum of each cell (keeps peaks)
# "range": boxes from the minimum to the maximum (keeps peaks and gaps)
cells = "max"
# Most cells (cubes or surface vertices) drawn; lower it for older hardware.
# 65536 gives 512 (time) x 128 (frequency) cells.
max_cells = 65536
# Axes with ticks and labels: time and frequency for spectrograms, time and amplitude for
# signals
axes = false
# Initial camera: rotation around the vertical axis and elevation in degrees, distance
yaw = -30.0
pitch = 35.0
distance = 22.0

[labels]
# Every text of the user interface, e.g. for translations. Values are filled into the
# placeholders in braces; their order can change. Units (ms, Hz, kHz) stay as they are.

# Axis titles (view.axes); an empty title is not shown. {unit} is Hz or kHz.
time = "time [ms]"
frequency = "frequency [{unit}]"
amplitude = "amplitude"
# Buttons: display mode and saving the window as PNG
surface = "Surface"
cubes = "Cubes"
save_png = "PNG"
# Shown instead of the axes
hint_spectrogram = "time: right   frequency: depth   amplitude: up"
hint_signal = "time: right   amplitude: up"
# Readout under the cursor; {value} for one value, {max} and {min} for several
readout_time = "t = {time}"
readout_frequency = "f = {frequency}"
readout_value = "amplitude {value}"
readout_range = "max {max}   min {min}"
readout_not_loaded = "not loaded"
# Below the timeline
timeline = "start {start}    end {end}    span {span}    of {length}"
# Title of the file dialog
open_file = "Open a measurement file"
# Folder panel: section titles, the title of the file list, the second line of a file
# ({time} and {frequency} compression, {bands} frequency bands), and status messages
project = "Project"
measurement = "Measurement"
channels = "Channels"
files = "Files on channel {channel}"
file_details = "t ×{time} · f ×{frequency} · {bands} bands"
signal_details = "signal · t ×{time}"
scanning = "Reading {folder} ..."
no_files = "No measurement files in {folder}"
# Font for all texts. The bundled Fira Mono covers Latin, Greek and Cyrillic; set a font file
# (TrueType / OpenType) for other scripts, e.g. "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"
# font = ""

[scaling]
# "linear": 0 to the full scale of the file
# "auto": minimum to maximum of the displayed data
# "fixed": from min to max below
mode = "auto"
# min = 0.0
# max = 65536.0

[controls]
# Mouse buttons: "left", "right" or "middle"
pan = "left"
rotate = "right"

[loading]
# Threads that read and reduce data; 0 picks up to 4. More rarely helps, loading is limited by
# memory and disk bandwidth.
threads = 0
# Memory for cached, reduced data per open file
cache_mb = 512

[export]
# Image size of `glowfish export` in pixels
width = 1920
height = 1080

[metadata]
# Database of the metadata cache used when opening folders: a file path, or ":memory:"
database = ":memory:"
# Include files in subfolders when opening a folder
recursive = true
"#;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub view: ViewConfig,
    pub labels: Labels,
    pub scaling: ScalingConfig,
    pub controls: ControlsConfig,
    pub loading: LoadingConfig,
    pub export: ExportConfig,
    pub metadata: MetadataConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewConfig {
    pub mode: Mode,
    pub cells: Cells,
    pub max_cells: usize,
    pub axes: bool,
    /// Degrees.
    pub yaw: f32,
    /// Degrees.
    pub pitch: f32,
    pub distance: f32,
}

impl Default for ViewConfig {
    fn default() -> Self {
        let perspective = Perspective::default();
        Self {
            mode: Mode::Surface,
            cells: Cells::Max,
            max_cells: VisualizationConfig::default().max_cells,
            axes: VisualizationConfig::default().axes,
            yaw: perspective.yaw.to_degrees().round(),
            pitch: perspective.pitch.to_degrees().round(),
            distance: perspective.distance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Surface,
    Cubes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Cells {
    Max,
    Range,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScalingConfig {
    pub mode: ScalingMode,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ScalingMode {
    Linear,
    #[default]
    Auto,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ControlsConfig {
    pub pan: Button,
    pub rotate: Button,
}

impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            pan: Button::Left,
            rotate: Button::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl From<Button> for MouseButton {
    fn from(button: Button) -> Self {
        match button {
            Button::Left => MouseButton::Left,
            Button::Right => MouseButton::Right,
            Button::Middle => MouseButton::Middle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoadingConfig {
    pub threads: usize,
    pub cache_mb: usize,
}

impl Default for LoadingConfig {
    fn default() -> Self {
        Self {
            threads: 0,
            cache_mb: 512,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExportConfig {
    pub width: u32,
    pub height: u32,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetadataConfig {
    pub database: String,
    pub recursive: bool,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            database: ":memory:".into(),
            recursive: true,
        }
    }
}

/// Where the configuration file is expected.
pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "glowfish")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

impl Config {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let config: Config = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    /// Reads `path`, or the default location if `path` is `None`. A missing file at the default
    /// location gives the defaults; a missing file that was asked for explicitly is an error.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let (path, explicit) = match path {
            Some(path) => (path.to_owned(), true),
            None => match default_path() {
                Some(path) => (path, false),
                None => return Ok(Self::default()),
            },
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text)
                .with_context(|| format!("invalid configuration {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.view.max_cells == 0 {
            bail!("view.max_cells must be at least 1");
        }
        if !self.view.distance.is_finite() || self.view.distance <= 0.0 {
            bail!("view.distance must be positive");
        }
        if self.export.width == 0 || self.export.height == 0 {
            bail!("export.width and export.height must be at least 1");
        }
        if self.controls.pan == self.controls.rotate {
            bail!("controls.pan and controls.rotate must use different buttons");
        }
        if let Some(font) = &self.labels.font
            && !font.is_file()
        {
            bail!("labels.font: {} is not a file", font.display());
        }
        self.scaling()?;
        Ok(())
    }

    pub fn scaling(&self) -> anyhow::Result<Scaling> {
        let s = &self.scaling;
        Ok(match s.mode {
            ScalingMode::Linear => Scaling::Linear,
            ScalingMode::Auto => Scaling::Auto,
            ScalingMode::Fixed => match (s.min, s.max) {
                (Some(min), Some(max)) if min < max => Scaling::Fixed { min, max },
                (Some(_), Some(_)) => bail!("scaling.min must be smaller than scaling.max"),
                _ => bail!("fixed scaling needs scaling.min and scaling.max"),
            },
        })
    }

    /// The visualization settings; call [`validate`](Self::validate) first.
    pub fn visualization(&self) -> anyhow::Result<VisualizationConfig> {
        let v = &self.view;
        Ok(VisualizationConfig {
            max_cells: v.max_cells,
            mode: match v.mode {
                Mode::Surface => DisplayMode::Surface,
                Mode::Cubes => DisplayMode::Cubes,
            },
            cell_value: match v.cells {
                Cells::Max => CellValue::Max,
                Cells::Range => CellValue::Range,
            },
            scaling: self.scaling()?,
            axes: v.axes,
            labels: self.labels.clone(),
            controls: Controls {
                pan: self.controls.pan.into(),
                rotate: self.controls.rotate.into(),
            },
            perspective: Perspective {
                yaw: v.yaw.to_radians(),
                pitch: v.pitch.to_radians(),
                distance: v.distance,
            },
            ..VisualizationConfig::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_matches_the_defaults() {
        assert_eq!(Config::parse(DEFAULT_CONFIG).unwrap(), Config::default());
    }

    #[test]
    fn documentation_shows_the_default_file() {
        let page = include_str!("../../../docs/configuration.md");
        assert!(
            page.contains(&format!("```toml\n{DEFAULT_CONFIG}```")),
            "update the defaults in docs/configuration.md"
        );
    }

    #[test]
    fn default_file_lists_every_setting() {
        let defaults = toml::to_string(&Config::default()).unwrap();
        for line in defaults.lines() {
            if let Some((key, _)) = line.split_once(" = ") {
                assert!(
                    DEFAULT_CONFIG
                        .lines()
                        .any(|l| l.starts_with(&format!("{key} = "))),
                    "`{key}` is missing in DEFAULT_CONFIG"
                );
            }
        }
    }

    #[test]
    fn partial_files_keep_defaults() {
        let config = Config::parse("[view]\nmode = \"cubes\"\n").unwrap();
        assert_eq!(config.view.mode, Mode::Cubes);
        assert_eq!(config.view.max_cells, ViewConfig::default().max_cells);
        assert_eq!(config.loading, LoadingConfig::default());
    }

    #[test]
    fn translated_labels() {
        let config =
            Config::parse("[labels]\ntime = \"Zeit [ms]\"\nfrequency = \"Frequenz [{unit}]\"\n")
                .unwrap();
        let labels = config.visualization().unwrap().labels;
        assert_eq!(labels.time, "Zeit [ms]");
        assert_eq!(labels.frequency_in("kHz"), "Frequenz [kHz]");
        assert_eq!(labels.amplitude, "amplitude");
        assert_eq!(labels.cubes, Labels::default().cubes);
        assert!(
            Config::parse("[labels]\nsurfaces = \"x\"\n").is_err(),
            "unknown label"
        );
        let config = Config::parse("[labels.datatypes]\nRAW = \"Rohdaten\"\n").unwrap();
        assert_eq!(config.labels.datatypes["RAW"], "Rohdaten");
    }

    #[test]
    fn rejects_mistakes() {
        assert!(
            Config::parse("[view]\nmod = \"cubes\"\n").is_err(),
            "unknown key"
        );
        assert!(
            Config::parse("[view]\nmode = \"bars\"\n").is_err(),
            "unknown value"
        );
        assert!(Config::parse("[scaling]\nmode = \"fixed\"\nmin = 1.0\n").is_err());
        assert!(Config::parse("[scaling]\nmode = \"fixed\"\nmin = 2.0\nmax = 1.0\n").is_err());
        assert!(Config::parse("[controls]\npan = \"left\"\nrotate = \"left\"\n").is_err());
        assert!(Config::parse("[view]\nmax_cells = 0\n").is_err());
        assert!(Config::parse("[labels]\nfont = \"/nonexistent.ttf\"\n").is_err());
    }

    #[test]
    fn converts_to_visualization_settings() {
        let config = Config::parse(
            "[view]\nyaw = 90.0\ncells = \"range\"\n[scaling]\nmode = \"fixed\"\nmin = 1.0\nmax = 5.0\n[controls]\npan = \"middle\"\n",
        )
        .unwrap();
        let v = config.visualization().unwrap();
        assert!((v.perspective.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(v.cell_value, CellValue::Range);
        assert_eq!(v.scaling, Scaling::Fixed { min: 1.0, max: 5.0 });
        assert_eq!(v.controls.pan, MouseButton::Middle);
    }

    #[test]
    fn missing_default_file_is_fine_but_explicit_file_is_not() {
        assert!(Config::load(Some(Path::new("/nonexistent/glowfish.toml"))).is_err());
    }
}
