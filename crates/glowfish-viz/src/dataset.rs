use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;
use glowfish_data::{LoadPool, Pyramid, Source};
use qass_files::{Datamode, ParseError, StreamFile};

/// What kind of data a dataset holds; decides how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    /// Time × frequency × amplitude, drawn in 3D.
    Spectrogram,
    /// Time × amplitude, drawn as a flat line plot.
    Signal,
}

/// The data that is visualized. Replacing the resource shows another dataset.
#[derive(Resource, Clone)]
pub struct Dataset {
    pub name: String,
    pub kind: DataKind,
    pub pyramid: Arc<Pyramid<Box<dyn Source>>>,
    /// Duration of one row (spectrum or sample) in ns.
    pub row_duration: f64,
    /// Lower frequency of band 0 in Hz.
    pub frq_start: f64,
    pub frq_per_band: f64,
    /// Largest possible sample value, used by linear scaling.
    pub full_scale: f32,
}

impl Dataset {
    pub fn open(
        path: impl AsRef<Path>,
        cache_bytes: usize,
        pool: LoadPool,
    ) -> Result<Self, ParseError> {
        let path = path.as_ref();
        let file = StreamFile::open(path)?;
        let h = file.header();
        let kind = match h.datamode {
            Datamode::Signal => DataKind::Signal,
            _ => DataKind::Spectrogram,
        };
        Ok(Self {
            name: path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into(),
            ),
            kind,
            row_duration: h.spec_duration,
            frq_start: h.frq_start,
            frq_per_band: h.frq_per_band,
            full_scale: h.max_amplitude_level as f32,
            pyramid: Arc::new(Pyramid::new(Box::new(file), cache_bytes, pool)),
        })
    }

    pub fn rows(&self) -> u64 {
        self.pyramid.rows()
    }

    pub fn bands(&self) -> usize {
        self.pyramid.bands()
    }

    /// Time in ms at the start of `row`.
    pub fn row_time_ms(&self, row: f64) -> f64 {
        row * self.row_duration / 1e6
    }

    /// Frequency in Hz at the lower edge of `band`.
    pub fn band_frequency(&self, band: f64) -> f64 {
        self.frq_start + band * self.frq_per_band
    }
}

/// The displayed time window in rows. Fractional so that panning is smooth when zoomed in.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub start: f64,
    pub end: f64,
}

impl Selection {
    /// Fewest rows that can be selected.
    pub const MIN_SPAN: f64 = 8.0;

    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }

    pub fn all(rows: u64) -> Self {
        Self::new(0.0, rows as f64)
    }

    pub fn span(&self) -> f64 {
        self.end - self.start
    }

    /// The rows to load: every row that is at least partly visible.
    pub fn rows(&self) -> Range<u64> {
        self.start.max(0.0).floor() as u64..self.end.max(0.0).ceil() as u64
    }

    /// Limits the selection to `0..rows` and to at least [`MIN_SPAN`](Self::MIN_SPAN) rows,
    /// keeping the span when it is moved against a border.
    pub fn clamped(self, rows: u64) -> Self {
        let rows = rows as f64;
        let span = self.span().clamp(Self::MIN_SPAN.min(rows), rows);
        let start = self.start.clamp(0.0, rows - span);
        Self::new(start, start + span)
    }

    /// The part of the selection inside `0..rows`, at least [`MIN_SPAN`](Self::MIN_SPAN) rows.
    pub fn clipped(self, rows: u64) -> Self {
        let total = rows as f64;
        Self::new(self.start.max(0.0), self.end.min(total)).clamped(rows)
    }

    /// Scales the selection by `factor` keeping the row at fraction `at` of the window in
    /// place; `factor < 1` zooms in.
    pub fn zoomed(self, at: f64, factor: f64, rows: u64) -> Self {
        let at = at.clamp(0.0, 1.0);
        let pivot = self.start + self.span() * at;
        let span = self.span() * factor;
        Self::new(pivot - span * at, pivot - span * at + span).clamped(rows)
    }

    /// Moves the selection by `rows`, stopping at the borders of the data.
    pub fn shifted(self, rows: f64, total: u64) -> Self {
        Self::new(self.start + rows, self.end + rows).clamped(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_clamping() {
        let s = Selection::new(-10.0, 50.0).clamped(100);
        assert_eq!(s, Selection::new(0.0, 60.0));
        let s = Selection::new(90.0, 150.0).clamped(100);
        assert_eq!(s, Selection::new(40.0, 100.0));
        let s = Selection::new(10.0, 11.0).clamped(100);
        assert_eq!(s.span(), Selection::MIN_SPAN);
        let s = Selection::new(0.0, 500.0).clamped(100);
        assert_eq!(s, Selection::all(100));
        // tiny files
        assert_eq!(Selection::new(0.0, 1.0).clamped(3), Selection::all(3));
    }

    #[test]
    fn selection_clipping() {
        assert_eq!(
            Selection::new(90.0, 150.0).clipped(100),
            Selection::new(90.0, 100.0)
        );
        assert_eq!(
            Selection::new(200.0, 300.0).clipped(100).span(),
            Selection::MIN_SPAN
        );
    }

    #[test]
    fn selection_shift_keeps_span() {
        let s = Selection::new(10.0, 30.0).shifted(-50.0, 100);
        assert_eq!(s, Selection::new(0.0, 20.0));
        let s = Selection::new(10.0, 30.0).shifted(5.5, 100);
        assert_eq!(s, Selection::new(15.5, 35.5));
    }

    #[test]
    fn zoom_keeps_the_pivot() {
        let s = Selection::new(100.0, 200.0).zoomed(0.25, 0.5, 1000);
        assert_eq!(s, Selection::new(112.5, 162.5));
        let s = Selection::new(0.0, 100.0).zoomed(0.5, 100.0, 1000);
        assert_eq!(s, Selection::all(1000));
    }

    #[test]
    fn selection_rows_cover_partial_rows() {
        assert_eq!(Selection::new(1.5, 4.2).rows(), 1..5);
    }
}
