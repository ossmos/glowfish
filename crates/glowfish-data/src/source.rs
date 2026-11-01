use std::ops::Range;

use analyzer_files::StreamFile;

use crate::DataError;

/// Row major time × band data that a [`Pyramid`](crate::Pyramid) is built from.
///
/// A "row" is one spectrum, or one sample for signals (`bands() == 1`).
pub trait Source: Send + Sync {
    fn rows(&self) -> u64;

    fn bands(&self) -> usize;

    /// Appends rows `range` (`range.len() * bands()` values) to `out`.
    fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError>;
}

impl Source for StreamFile {
    fn rows(&self) -> u64 {
        self.spec_count()
    }

    fn bands(&self) -> usize {
        self.header().frq_bands as usize
    }

    fn read(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), DataError> {
        Ok(self.read_f32(range, out)?)
    }
}
