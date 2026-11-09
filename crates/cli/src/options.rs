use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::Args;
use glowfish_data::LoadPool;
use glowfish_viz::{Dataset, Selection};

use crate::config::{Cells, ColormapName, Config, Mode, ScalingMode};

/// Options shared by the viewer and the export; they override the configuration file.
#[derive(Debug, Clone, Default, Args)]
pub struct ViewOptions {
    /// Configuration file to use instead of the default location
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// How spectrograms are drawn
    #[arg(long, value_enum)]
    pub mode: Option<Mode>,
    /// "max": columns up to the maximum; "range": boxes from minimum to maximum
    #[arg(long, value_enum)]
    pub cells: Option<Cells>,
    /// Colours of the amplitude
    #[arg(long, value_enum)]
    pub colormap: Option<ColormapName>,
    /// Most cells (cubes / surface vertices) drawn
    #[arg(long, value_name = "N")]
    pub max_cells: Option<usize>,

    /// Show axes with ticks and labels
    #[arg(long, overrides_with = "no_axes")]
    pub axes: bool,
    /// Hide the axes
    #[arg(long, overrides_with = "axes")]
    pub no_axes: bool,

    /// How values are scaled
    #[arg(long, value_enum)]
    pub scaling: Option<ScalingMode>,
    /// Lower value for fixed scaling
    #[arg(long, allow_negative_numbers = true)]
    pub min: Option<f32>,
    /// Upper value for fixed scaling
    #[arg(long, allow_negative_numbers = true)]
    pub max: Option<f32>,

    /// Start of the displayed time window in ms (default: start of the file)
    #[arg(long, value_name = "MS")]
    pub start: Option<f64>,
    /// End of the displayed time window in ms (default: end of the file)
    #[arg(long, value_name = "MS")]
    pub end: Option<f64>,

    /// Camera rotation around the vertical axis in degrees
    #[arg(long, allow_negative_numbers = true)]
    pub yaw: Option<f32>,
    /// Camera elevation in degrees
    #[arg(long)]
    pub pitch: Option<f32>,
    /// Camera distance
    #[arg(long)]
    pub distance: Option<f32>,

    /// Threads that load data (0: up to 4)
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,
    /// Memory for cached data in MiB
    #[arg(long, value_name = "MIB")]
    pub cache_mb: Option<usize>,
}

impl ViewOptions {
    /// The configuration file with these options applied.
    pub fn config(&self) -> anyhow::Result<Config> {
        let mut config = Config::load(self.config.as_deref())?;
        self.apply(&mut config);
        config.validate().context("invalid options")?;
        Ok(config)
    }

    fn apply(&self, config: &mut Config) {
        let v = &mut config.view;
        set(&mut v.mode, self.mode);
        set(&mut v.cells, self.cells);
        set(&mut v.colormap, self.colormap);
        set(&mut v.max_cells, self.max_cells);
        set(
            &mut v.axes,
            match (self.axes, self.no_axes) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            },
        );
        set(&mut v.yaw, self.yaw);
        set(&mut v.pitch, self.pitch);
        set(&mut v.distance, self.distance);
        set(&mut config.scaling.mode, self.scaling);
        if self.min.is_some() {
            config.scaling.min = self.min;
        }
        if self.max.is_some() {
            config.scaling.max = self.max;
        }
        // --min/--max alone mean fixed scaling
        if self.scaling.is_none() && (self.min.is_some() || self.max.is_some()) {
            config.scaling.mode = ScalingMode::Fixed;
        }
        set(&mut config.loading.threads, self.threads);
        set(&mut config.loading.cache_mb, self.cache_mb);
    }

    /// The time window from `--start`/`--end`, `None` for the whole file.
    pub fn selection(&self, dataset: &Dataset) -> anyhow::Result<Option<Selection>> {
        if self.start.is_none() && self.end.is_none() {
            return Ok(None);
        }
        let to_row = |ms: f64| ms * 1e6 / dataset.row_duration;
        let start = self.start.unwrap_or(0.0);
        let end = self
            .end
            .unwrap_or_else(|| dataset.row_time_ms(dataset.rows() as f64));
        let length = dataset.row_time_ms(dataset.rows() as f64);
        if start >= length {
            bail!("--start {start} ms is after the end of the file ({length:.1} ms)");
        }
        if start < 0.0 || end <= start {
            bail!("--start must be at least 0 and smaller than --end");
        }
        Ok(Some(Selection::new(to_row(start), to_row(end))))
    }
}

fn set<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

/// Opens a measurement file.
pub fn open_dataset(path: &Path, config: &Config) -> anyhow::Result<Dataset> {
    if path.is_dir() {
        bail!("{} is a folder; open a measurement file", path.display());
    }
    let pool = LoadPool::new(config.loading.threads);
    Dataset::open(path, config.loading.cache_mb << 20, pool)
        .with_context(|| format!("cannot open {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_flags() {
        let mut config = Config::parse("[view]\naxes = true\n").unwrap();
        let options = ViewOptions {
            no_axes: true,
            ..Default::default()
        };
        options.apply(&mut config);
        assert!(!config.view.axes);
        ViewOptions {
            axes: true,
            ..Default::default()
        }
        .apply(&mut config);
        assert!(config.view.axes);
    }

    #[test]
    fn options_override_the_file() {
        let mut config = Config::parse("[view]\nmode = \"cubes\"\nmax_cells = 100\n").unwrap();
        let options = ViewOptions {
            mode: Some(Mode::Surface),
            colormap: Some(ColormapName::Jet),
            min: Some(1.0),
            max: Some(9.0),
            ..Default::default()
        };
        options.apply(&mut config);
        assert_eq!(config.view.mode, Mode::Surface);
        assert_eq!(config.view.colormap, ColormapName::Jet);
        assert_eq!(config.view.max_cells, 100);
        assert_eq!(config.scaling.mode, ScalingMode::Fixed);
        assert_eq!(
            (config.scaling.min, config.scaling.max),
            (Some(1.0), Some(9.0))
        );
    }
}
