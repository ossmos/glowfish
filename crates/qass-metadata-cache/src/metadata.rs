use chrono::{DateTime, Utc};
use qass_files::StreamFile;

use crate::error::StoreError;

#[derive(Debug, sqlx::FromRow)]
pub struct StreamFileMetadata {
    pub id: Option<i64>,
    pub filepath: String,
    pub header_size: u32,
    pub project_id: u64,
    pub channel: u32,
    pub process: u32,
    pub datamode: i32,
    pub datatype: i32,
    pub datakind: i32,
    pub process_datetime: DateTime<Utc>,
    pub measure_start: Option<DateTime<Utc>>,
    pub db_header_size: u32,
    pub bytes_per_sample: u8,
    pub db_size: u32,
    pub frq_bands: u32,
    pub compression_time: u32,
    pub compression_frq: u32,
    pub avg_time: i32,
    pub avg_frq: i32,
    pub spec_duration: f64, // nanoseconds
    pub frq_start: f64,
    pub frq_end: f64,
    pub frq_per_band: f64,
    pub adc_type: i32,
    pub bit_resolution: u32,
    pub fft_log_shift: i32,
    pub streamno: Option<u32>,
    pub preamp_gain: i32,
    pub file_size: u64,
    pub db_count: u64,
    pub spec_count: u64,
}

impl TryFrom<&StreamFile> for StreamFileMetadata {
    type Error = StoreError;

    fn try_from(stream_file: &StreamFile) -> Result<Self, StoreError> {
        let h = stream_file.header();
        let filepath = stream_file
            .path()
            .to_str()
            .ok_or_else(|| StoreError::NonUtf8Path(stream_file.path().to_owned()))?
            .to_owned();
        Ok(Self {
            id: None,
            filepath,
            header_size: h.header_size,
            project_id: h.project_id,
            channel: h.channel,
            process: h.process,
            datamode: h.datamode.as_i32(),
            datatype: h.datatype.as_i32(),
            datakind: h.datakind.as_i32(),
            process_datetime: h.process_time,
            measure_start: h.measure_start,
            db_header_size: h.db_header_size,
            bytes_per_sample: h.sample_type.size() as u8,
            db_size: h.db_size,
            frq_bands: h.frq_bands,
            compression_time: h.compression_time,
            compression_frq: h.compression_frq,
            avg_time: h.avg_time,
            avg_frq: h.avg_frq,
            spec_duration: h.spec_duration,
            frq_start: h.frq_start,
            frq_end: h.frq_end,
            frq_per_band: h.frq_per_band,
            adc_type: h.adc_type.as_i32(),
            bit_resolution: h.bit_resolution,
            fft_log_shift: h.fft_log_shift,
            streamno: h.streamno,
            preamp_gain: h.preamp_gain,
            file_size: stream_file.file_size(),
            db_count: stream_file.db_count(),
            spec_count: stream_file.spec_count(),
        })
    }
}
