use crate::{Grid, MinMaxCell};

/// How sample values are mapped to the displayed amplitude. Values are unitless.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Scaling {
    /// Raw values against the full scale of the file (`0..=max_amplitude_level`).
    #[default]
    Linear,
    /// Minimum and maximum of the data that is currently displayed.
    Auto,
    /// A fixed, user given range.
    Fixed { min: f32, max: f32 },
}

impl Scaling {
    /// The value range to display. `data` is the range of the displayed data, if any is
    /// loaded; `Auto` falls back to the full scale without data.
    pub fn resolve(&self, full_scale: f32, data: Option<ValueRange>) -> ValueRange {
        match *self {
            Scaling::Linear => ValueRange::new(0.0, full_scale),
            Scaling::Auto => data.unwrap_or(ValueRange::new(0.0, full_scale)),
            Scaling::Fixed { min, max } => ValueRange::new(min, max),
        }
    }
}

/// A closed value range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueRange {
    pub min: f32,
    pub max: f32,
}

impl ValueRange {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    /// Maps `value` to `0.0..=1.0`, clamping values outside of the range. An empty range maps
    /// everything to 0.
    pub fn normalize(&self, value: f32) -> f32 {
        let span = self.max - self.min;
        if span <= 0.0 || !span.is_finite() {
            return 0.0;
        }
        ((value - self.min) / span).clamp(0.0, 1.0)
    }
}

impl Grid<MinMaxCell> {
    /// Minimum and maximum over all non-empty cells, `None` if no cell has data.
    pub fn value_range(&self) -> Option<ValueRange> {
        self.cells
            .iter()
            .filter(|c| !c.is_empty())
            .fold(None, |range: Option<ValueRange>, c| {
                Some(match range {
                    None => ValueRange::new(c.min, c.max),
                    Some(r) => ValueRange::new(r.min.min(c.min), r.max.max(c.max)),
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_modes() {
        let data = Some(ValueRange::new(10.0, 20.0));
        assert_eq!(
            Scaling::Linear.resolve(100.0, data),
            ValueRange::new(0.0, 100.0)
        );
        assert_eq!(
            Scaling::Auto.resolve(100.0, data),
            ValueRange::new(10.0, 20.0)
        );
        assert_eq!(
            Scaling::Auto.resolve(100.0, None),
            ValueRange::new(0.0, 100.0)
        );
        assert_eq!(
            Scaling::Fixed { min: 1.0, max: 2.0 }.resolve(100.0, data),
            ValueRange::new(1.0, 2.0)
        );
    }

    #[test]
    fn normalize_clamps() {
        let r = ValueRange::new(10.0, 20.0);
        assert_eq!(r.normalize(15.0), 0.5);
        assert_eq!(r.normalize(0.0), 0.0);
        assert_eq!(r.normalize(30.0), 1.0);
        assert_eq!(ValueRange::new(5.0, 5.0).normalize(5.0), 0.0);
    }
}
