use analyzer_files::StreamFile;
use chrono::{DateTime, Utc};

#[derive(Debug, sqlx::FromRow)]
pub struct StreamFileMetadata {
    pub id: Option<i64>,
    pub filepath: String,
    pub header_size: u32,
    pub project_id: u64,
    pub channel: u8,
    pub process: u32,
    //datamode: ,
    //datakind: ,
    //datatype: ,
    pub process_time: u32,
    pub process_datetime: DateTime<Utc>,
    pub db_header_size: u32, // Is this really useful here?
    pub bytes_per_sample: u8,
    pub db_size: u32, // useful?
    pub frq_bands: u32,
    pub compression_time: u32,
    pub compression_frq: u32,
    pub avg_time: u32,
    pub avg_frq: u32,
    pub spec_duration: u64, // nanoseconds
    pub frq_start: u32,
    pub frq_end: u32,
    pub frq_per_band: u32,
    //adc_type: ,
    pub bit_resolution: u8,
    pub fft_log_shift: u8,
    //max_amplitude_level
    pub streamno: u32,
    pub preamp_gain: u32,
    pub file_size: u64,
    pub db_count: u32,        // needed?
    pub db_sample_count: u32, // needed?
    pub db_spec_count: u32,   // needed?
    pub sample_count: u32,
    pub spec_count: u32,
}

impl From<StreamFile> for StreamFileMetadata {
    fn from(stream_file: StreamFile) -> Self {
        Self {
            id: None,
            filepath: stream_file.filepath.into_os_string().into_string().unwrap(),
            header_size: stream_file.header_size as u32,
            project_id: stream_file.project_id,
            channel: stream_file.channel as u8,
            process: stream_file.process as u32,
            process_time: stream_file.process_time,
            process_datetime: stream_file.process_datetime,
            db_header_size: stream_file.db_header_size as u32,
            bytes_per_sample: stream_file.bytes_per_sample as u8,
            db_size: stream_file.db_size as u32,
            frq_bands: stream_file.frq_bands as u32,
            compression_time: stream_file.compression_time as u32,
            compression_frq: stream_file.compression_frq as u32,
            avg_time: stream_file.avg_time as u32,
            avg_frq: stream_file.avg_frq as u32,
            spec_duration: stream_file.spec_duration as u64,
            frq_start: stream_file.frq_start as u32,
            frq_end: stream_file.frq_end as u32,
            frq_per_band: stream_file.frq_per_band as u32,
            //adc_type: stream_file.//adc_type,
            bit_resolution: stream_file.bit_resolution as u8,
            fft_log_shift: stream_file.fft_log_shift as u8,
            //max_amplitude_level
            streamno: stream_file.streamno,
            preamp_gain: stream_file.preamp_gain as u32,
            file_size: stream_file.file_size,
            db_count: stream_file.db_count as u32,
            db_sample_count: stream_file.db_sample_count as u32,
            db_spec_count: stream_file.db_spec_count as u32,
            sample_count: stream_file.sample_count as u32,
            spec_count: stream_file.spec_count as u32,
        }
    }
}
