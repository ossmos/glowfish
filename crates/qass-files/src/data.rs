use crate::SampleType;

/// A sample type that can be stored in a stream file.
pub trait Sample: Copy + Send + Sync + 'static {
    const SAMPLE_TYPE: SampleType;

    /// Decodes one little endian sample; `bytes` has exactly `SAMPLE_TYPE.size()` bytes.
    fn from_le_slice(bytes: &[u8]) -> Self;

    fn to_f64(self) -> f64;
}

impl Sample for u16 {
    const SAMPLE_TYPE: SampleType = SampleType::U16;

    fn from_le_slice(bytes: &[u8]) -> Self {
        u16::from_le_bytes([bytes[0], bytes[1]])
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }
}

impl Sample for u32 {
    const SAMPLE_TYPE: SampleType = SampleType::U32;

    fn from_le_slice(bytes: &[u8]) -> Self {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }
}

impl Sample for f32 {
    const SAMPLE_TYPE: SampleType = SampleType::F32;

    fn from_le_slice(bytes: &[u8]) -> Self {
        f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }
}

/// Owned samples in the type they are stored in the file, so no precision is lost.
#[derive(Debug, Clone, PartialEq)]
pub enum Samples {
    U16(Vec<u16>),
    U32(Vec<u32>),
    F32(Vec<f32>),
}

impl Samples {
    pub fn sample_type(&self) -> SampleType {
        match self {
            Samples::U16(_) => SampleType::U16,
            Samples::U32(_) => SampleType::U32,
            Samples::F32(_) => SampleType::F32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Samples::U16(v) => v.len(),
            Samples::U32(v) => v.len(),
            Samples::F32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<f64> {
        match self {
            Samples::U16(v) => v.get(index).map(|s| s.to_f64()),
            Samples::U32(v) => v.get(index).map(|s| s.to_f64()),
            Samples::F32(v) => v.get(index).map(|s| s.to_f64()),
        }
    }

    /// Converts all samples to `f32`. Exact for `u16` and `f32`; `u32` values above 2^24 are
    /// rounded.
    pub fn to_f32_vec(&self) -> Vec<f32> {
        match self {
            Samples::U16(v) => v.iter().map(|&s| f32::from(s)).collect(),
            Samples::U32(v) => v.iter().map(|&s| s as f32).collect(),
            Samples::F32(v) => v.clone(),
        }
    }
}

/// A range of spectra, stored row major: `spec_count` rows of `frq_bands` samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrogram {
    samples: Samples,
    first_spec: u64,
    frq_bands: usize,
    spec_duration: f64,
    frq_start: f64,
    frq_per_band: f64,
}

impl Spectrogram {
    /// `samples.len()` must be a multiple of `frq_bands`.
    pub fn new(
        samples: Samples,
        first_spec: u64,
        frq_bands: usize,
        spec_duration: f64,
        frq_start: f64,
        frq_per_band: f64,
    ) -> Self {
        assert!(frq_bands > 0, "a spectrogram needs at least one band");
        assert_eq!(
            samples.len() % frq_bands,
            0,
            "samples must contain whole spectra"
        );
        Self {
            samples,
            first_spec,
            frq_bands,
            spec_duration,
            frq_start,
            frq_per_band,
        }
    }

    pub fn samples(&self) -> &Samples {
        &self.samples
    }

    pub fn into_samples(self) -> Samples {
        self.samples
    }

    /// Index of the first spectrum in the file.
    pub fn first_spec(&self) -> u64 {
        self.first_spec
    }

    pub fn spec_count(&self) -> usize {
        self.samples.len() / self.frq_bands
    }

    pub fn frq_bands(&self) -> usize {
        self.frq_bands
    }

    /// `(spec_count, frq_bands)`
    pub fn shape(&self) -> (usize, usize) {
        (self.spec_count(), self.frq_bands)
    }

    /// Duration of one spectrum in ns.
    pub fn spec_duration(&self) -> f64 {
        self.spec_duration
    }

    pub fn get(&self, spec: usize, band: usize) -> Option<f64> {
        if band >= self.frq_bands {
            return None;
        }
        self.samples.get(spec * self.frq_bands + band)
    }

    /// Start time in ns (relative to the file start) of spectrum `spec` of this range.
    pub fn time_of(&self, spec: usize) -> f64 {
        (self.first_spec + spec as u64) as f64 * self.spec_duration
    }

    /// Lower edge frequency in Hz of `band`.
    pub fn frequency_of(&self, band: usize) -> f64 {
        self.frq_start + band as f64 * self.frq_per_band
    }
}

/// A range of a time-amplitude signal.
#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    samples: Samples,
    first_sample: u64,
    sample_duration: f64,
    zero_level: Option<f64>,
}

impl Signal {
    pub fn new(
        samples: Samples,
        first_sample: u64,
        sample_duration: f64,
        zero_level: Option<f64>,
    ) -> Self {
        Self {
            samples,
            first_sample,
            sample_duration,
            zero_level,
        }
    }

    pub fn samples(&self) -> &Samples {
        &self.samples
    }

    pub fn into_samples(self) -> Samples {
        self.samples
    }

    /// Index of the first sample in the file.
    pub fn first_sample(&self) -> u64 {
        self.first_sample
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Duration of one sample in ns.
    pub fn sample_duration(&self) -> f64 {
        self.sample_duration
    }

    /// The sample value that represents zero amplitude, if the file states it.
    pub fn zero_level(&self) -> Option<f64> {
        self.zero_level
    }

    pub fn get(&self, index: usize) -> Option<f64> {
        self.samples.get(index)
    }

    /// Time in ns (relative to the file start) of sample `index` of this range.
    pub fn time_of(&self, index: usize) -> f64 {
        (self.first_sample + index as u64) as f64 * self.sample_duration
    }
}

/// Data read from a stream file; the variant follows the file's datamode.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamData {
    Spectrogram(Spectrogram),
    Signal(Signal),
}

impl StreamData {
    pub fn samples(&self) -> &Samples {
        match self {
            StreamData::Spectrogram(s) => s.samples(),
            StreamData::Signal(s) => s.samples(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrogram_indexing() {
        let s = Spectrogram::new(
            Samples::U16(vec![1, 2, 3, 4, 5, 6]),
            10,
            3,
            2.5,
            100.0,
            50.0,
        );
        assert_eq!(s.shape(), (2, 3));
        assert_eq!(s.get(1, 0), Some(4.0));
        assert_eq!(s.get(0, 3), None);
        assert_eq!(s.get(2, 0), None);
        assert_eq!(s.time_of(1), 27.5);
        assert_eq!(s.frequency_of(2), 200.0);
    }

    #[test]
    #[should_panic(expected = "whole spectra")]
    fn spectrogram_rejects_partial_spectrum() {
        Spectrogram::new(Samples::U16(vec![1, 2, 3]), 0, 2, 1.0, 0.0, 1.0);
    }

    #[test]
    fn samples_conversion() {
        let s = Samples::U32(vec![1, 16_777_217]);
        assert_eq!(s.to_f32_vec(), vec![1.0, 16_777_216.0]);
        assert_eq!(s.get(1), Some(16_777_217.0));
        assert_eq!(s.sample_type(), SampleType::U32);
    }
}
