use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ring::digest::{Context, Digest, SHA256};

#[derive(Debug)]
pub enum Datamode {
    Undef = -1,
    CounterUnused = 0,
    Signal = 1,
    FFT = 2,
    Plot = 3,
    Other = 4,
    Video = 5,
    Count = 6,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("missing header key {0}")]
    MissingKey(&'static str),
    #[error("invalid value for {0}: {1}")]
    InvalidValue(&'static str, &'static str),
    #[error("the file is not a valid stream file")]
    InvalidFile,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unable to compute hash value")]
    HashCompute,
}

fn read_i32(buf: &[u8], at: usize) -> Result<i32, ParseError> {
    let bytes = buf.get(at..at + 4).ok_or(ParseError::UnexpectedEof)?;
    Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(buf: &[u8], at: usize) -> Result<u32, ParseError> {
    let bytes = buf.get(at..at + 4).ok_or(ParseError::UnexpectedEof)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u64(buf: &[u8], at: usize) -> Result<u64, ParseError> {
    let bytes = buf.get(at..at + 8).ok_or(ParseError::UnexpectedEof)?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_f64(buf: &[u8], at: usize) -> Result<f64, ParseError> {
    let bytes = buf.get(at..at + 8).ok_or(ParseError::UnexpectedEof)?;
    Ok(f64::from_le_bytes(bytes.try_into().unwrap()))
}

impl fmt::Display for Datamode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Datamode::Undef => write!(f, "UNDEF"),
            Datamode::CounterUnused => write!(f, "COUNTER_UNUSED"),
            Datamode::Signal => write!(f, "SIGNAL"),
            Datamode::FFT => write!(f, "FFT"),
            Datamode::Plot => write!(f, "PLOT"),
            Datamode::Other => write!(f, "OTHER"),
            Datamode::Video => write!(f, "VIDEO"),
            Datamode::Count => write!(f, "COUNT"),
        }
    }
}

impl From<i32> for Datamode {
    fn from(datamode: i32) -> Datamode {
        match datamode {
            -1 => Datamode::Undef,
            0 => Datamode::CounterUnused,
            1 => Datamode::Signal,
            2 => Datamode::FFT,
            3 => Datamode::Plot,
            4 => Datamode::Other,
            5 => Datamode::Video,
            6 => Datamode::Count,
            _ => Datamode::Undef, // TODO this might be bad
        }
    }
}

#[derive(Debug)]
pub enum Datakind {
    Undef = -1,
    None = 0,
    SensorTest = 1,
    FreeDatablockTiming = 2,
    PatternRefObjCompr = 3,
    PatternRefObjMask = 4,
    PatternRefObjExtra = 5,
    Free6 = 6,
    Free7 = 7,
    Count = 8,
    User = 100,
}

impl From<i32> for Datakind {
    fn from(datakind: i32) -> Datakind {
        match datakind {
            -1 => Datakind::Undef,
            0 => Datakind::None,
            1 => Datakind::SensorTest,
            2 => Datakind::FreeDatablockTiming,
            3 => Datakind::PatternRefObjCompr,
            4 => Datakind::PatternRefObjMask,
            5 => Datakind::PatternRefObjExtra,
            6 => Datakind::Free6,
            7 => Datakind::Free7,
            8 => Datakind::Count,
            100 => Datakind::User,
            _ => Datakind::Undef, // TODO this might be bad
        }
    }
}

#[derive(Debug)]
pub enum Datatype {
    Invalid = -2,
    Undef = -1,
    Raw = 0,
    Downsample = 1,
    Maximum = 2,
    Average = 3,
    StdDeviation = 4,
    Envelope = 5,
    MovingAverage = 6,
    ExternalData = 7,
    AnalyzeOverview = 8,
    MovingAverageOpt = 9,
    Collection = 10,
    ImportSig = 11,
    ScopeRaw = 12,
    MovingAverageFrq = 13,
    SlopeChange = 14,
    Other = 15,
    IOSignal = 16,
    Energy = 17,
    AudioRaw = 18,
    XYPlot = 19,
    SecondFFT = 20,
    AudioComment = 21,
    CPEnergySig = 22,
    VideoMeasure = 23,
    VideoScreen = 24,
    VideoExtLink = 25,
    EnvelopeUpper = 26,
    EnvelopeLower = 27,
    PatternRefObj = 28,
    Significance = 29,
    Significance32 = 30,
    PatternMask = 31,
    GradientFrq = 32,
    CSVExport = 33,
    Count = 34,
}

impl From<i32> for Datatype {
    fn from(datatype: i32) -> Datatype {
        match datatype {
            -2 => Datatype::Invalid,
            -1 => Datatype::Undef,
            0 => Datatype::Raw,
            1 => Datatype::Downsample,
            2 => Datatype::Maximum,
            3 => Datatype::Average,
            4 => Datatype::StdDeviation,
            5 => Datatype::Envelope,
            6 => Datatype::MovingAverage,
            7 => Datatype::ExternalData,
            8 => Datatype::AnalyzeOverview,
            9 => Datatype::MovingAverageOpt,
            10 => Datatype::Collection,
            11 => Datatype::ImportSig,
            12 => Datatype::ScopeRaw,
            13 => Datatype::MovingAverageFrq,
            14 => Datatype::SlopeChange,
            15 => Datatype::Other,
            16 => Datatype::IOSignal,
            17 => Datatype::Energy,
            18 => Datatype::AudioRaw,
            19 => Datatype::XYPlot,
            20 => Datatype::SecondFFT,
            21 => Datatype::AudioComment,
            22 => Datatype::CPEnergySig,
            23 => Datatype::VideoMeasure,
            24 => Datatype::VideoScreen,
            25 => Datatype::VideoExtLink,
            26 => Datatype::EnvelopeUpper,
            27 => Datatype::EnvelopeLower,
            28 => Datatype::PatternRefObj,
            29 => Datatype::Significance,
            30 => Datatype::Significance32,
            31 => Datatype::PatternMask,
            32 => Datatype::GradientFrq,
            33 => Datatype::CSVExport,
            34 => Datatype::Count,
            _ => Datatype::Invalid, // TODO this might be bad
        }
    }
}

#[derive(Debug)]
pub enum ADCType {
    NotUsed = 0,
    ADC16Bit = 1,
    ADC24Bit = 2,
}

impl From<i32> for ADCType {
    fn from(adc_type: i32) -> ADCType {
        match adc_type {
            0 => ADCType::NotUsed,
            1 => ADCType::ADC16Bit,
            2 => ADCType::ADC24Bit,
            _ => ADCType::NotUsed, // TODO this might be bad
        }
    }
}

#[derive(Default)]
struct HeaderBuilder {
    header_size: Option<i32>,
    project_id: Option<u64>,
    process: Option<i32>,
    channel: Option<i32>,
    datamode: Option<Datamode>,
    datakind: Option<Datakind>,
    datatype: Option<Datatype>,
    process_time: Option<u32>,
    process_datetime: Option<DateTime<Utc>>,
    db_header_size: Option<i32>,
    bytes_per_sample: Option<i32>,
    db_size: Option<i32>,
    frq_bands: Option<i32>,
    compression_frq: Option<i32>,
    compression_time: Option<i32>,
    avg_time: Option<i32>,
    avg_frq: Option<i32>,
    spec_duration: Option<f64>,
    frq_start: Option<i32>, // This is optional but defaults Option<to> 0
    frq_end: Option<i32>,   // This is optional but defaults to frq_bands * Option<frq_per_band>
    frq_per_band: Option<f64>,
    adc_type: Option<ADCType>,
    bit_resolution: Option<i32>,
    fft_log_shift: Option<i32>,
    max_amplitude_level: Option<i32>,
    streamno: Option<u32>,
    preamp_gain: Option<i32>,
    hash: Option<Digest>,
}

impl HeaderBuilder {
    fn build(self) -> Result<Header, ParseError> {
        let frq_bands = self.frq_bands.ok_or(ParseError::MissingKey("s_p_fram"))?;
        let frq_per_band = self
            .frq_per_band
            .ok_or(ParseError::MissingKey("frqpband"))?;
        let bytes_per_sample = self
            .bytes_per_sample
            .ok_or(ParseError::MissingKey("b_p_samp"))?;
        let header_size = self.header_size.ok_or(ParseError::MissingKey("qassdata"))?;
        let db_size = self.db_size.ok_or(ParseError::MissingKey("db__size"))?;
        let db_header_size = self
            .db_header_size
            .ok_or(ParseError::MissingKey("dbhdsize"))?;
        let frq_start = self.frq_start.unwrap_or(0);
        let frq_end = self
            .frq_end
            .unwrap_or_else(|| frq_bands * frq_per_band as i32);

        //let file_size_no_header = file_size as i64 - header_size as i64;

        //let db_count =
        //    (file_size_no_header as u64).div_ceil((db_size + db_header_size) as u64) as i32;
        //let sample_count =
        //    (file_size_no_header as i32 - db_count * db_header_size) / bytes_per_sample;
        let db_sample_count = db_size / bytes_per_sample;
        let db_spec_count = db_sample_count / frq_bands;
        //let spec_count = sample_count / frq_bands;
        Ok(Header {
            header_size,
            project_id: self.project_id.ok_or(ParseError::MissingKey("proj__id"))?,
            process: self.process.ok_or(ParseError::MissingKey("proc_cnt"))?,
            channel: self.channel.ok_or(ParseError::MissingKey("dumpchan"))?,
            datamode: self.datamode.ok_or(ParseError::MissingKey("datamode"))?,
            datakind: self.datakind.ok_or(ParseError::MissingKey("datakind"))?,
            datatype: self.datatype.ok_or(ParseError::MissingKey("datatype"))?,
            process_time: self
                .process_time
                .ok_or(ParseError::MissingKey("proctime"))?,
            process_datetime: self
                .process_datetime
                .ok_or(ParseError::MissingKey("proctime"))?,
            db_header_size,
            bytes_per_sample,
            db_size,
            frq_bands,
            compression_frq: self
                .compression_frq
                .ok_or(ParseError::MissingKey("frqratio"))?,
            compression_time: self
                .compression_time
                .ok_or(ParseError::MissingKey("comratio"))?,
            avg_time: self.avg_time.unwrap_or(0),
            avg_frq: self.avg_frq.unwrap_or(0),
            spec_duration: self
                .spec_duration
                .ok_or(ParseError::MissingKey("framedur"))?,
            frq_start,
            frq_end,
            frq_per_band,
            adc_type: self.adc_type.ok_or(ParseError::MissingKey("adc_type"))?,
            bit_resolution: self
                .bit_resolution
                .ok_or(ParseError::MissingKey("adbitres"))?,
            fft_log_shift: self
                .fft_log_shift
                .ok_or(ParseError::MissingKey("fftlogsh"))?,
            max_amplitude_level: self
                .max_amplitude_level
                .ok_or(ParseError::MissingKey("max_ampl"))?,
            streamno: self.streamno.unwrap_or(0),
            preamp_gain: self.preamp_gain.ok_or(ParseError::MissingKey("pampgain"))?,
            db_sample_count,
            db_spec_count,
            hash: self.hash.ok_or(ParseError::HashCompute)?,
        })
    }
}

#[derive(Debug)]
pub struct Header {
    header_size: i32,
    project_id: u64,
    process: i32,
    channel: i32,
    datamode: Datamode,
    datakind: Datakind,
    datatype: Datatype,
    process_time: u32,
    process_datetime: DateTime<Utc>,
    db_header_size: i32,
    bytes_per_sample: i32,
    db_size: i32,
    frq_bands: i32,
    compression_frq: i32,
    compression_time: i32,
    avg_time: i32,
    avg_frq: i32,
    spec_duration: f64,
    frq_start: i32, // This is optional but defaults Option<to 0
    frq_end: i32,   // This is optional but defaults to frq_bands * Option<frq_per_band
    frq_per_band: f64,
    adc_type: ADCType,
    bit_resolution: i32,
    fft_log_shift: i32,
    max_amplitude_level: i32,
    streamno: u32,
    preamp_gain: i32,

    db_sample_count: i32,
    db_spec_count: i32,
    hash: Digest,
}

#[derive(Debug)]
pub struct StreamFile {
    pub filepath: PathBuf,
    pub header_size: i32,
    pub project_id: u64,
    pub process: i32,
    pub channel: i32,
    pub datamode: Datamode,
    pub datakind: Datakind,
    pub datatype: Datatype,
    pub process_time: u32,
    pub process_datetime: DateTime<Utc>,
    pub db_header_size: i32,
    pub bytes_per_sample: i32,
    pub db_size: i32,
    pub frq_bands: i32,
    pub compression_frq: i32,
    pub compression_time: i32,
    pub avg_time: i32,
    pub avg_frq: i32,
    pub spec_duration: f64,
    pub frq_start: i32, // This is optional but defaults to 0
    pub frq_end: i32,   // This is optional but defaults to frq_bands * frq_per_band
    pub frq_per_band: f64,
    pub adc_type: ADCType,
    pub bit_resolution: i32,
    pub fft_log_shift: i32,
    pub max_amplitude_level: i32,
    pub streamno: u32,
    pub preamp_gain: i32,
    pub file_size: u64,

    // CALCULATED FIELDS
    pub db_count: i32,
    pub db_sample_count: i32,
    pub db_spec_count: i32,
    pub sample_count: i32,
    pub spec_count: i32,
    pub hash: Digest, // CALCULATE LATER
                      // ref energy
}

impl TryFrom<&Path> for StreamFile {
    type Error = ParseError;

    fn try_from(path: &Path) -> std::result::Result<Self, Self::Error> {
        let mut file: File = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();
        if file_size < 2000 {
            return Err(ParseError::InvalidFile);
        }
        let mut header_size_buffer = vec![0; 12];
        file.read_exact(&mut header_size_buffer)?;
        if &header_size_buffer[0..8] != b"qassdata" {
            return Err(ParseError::InvalidFile);
        }
        let header_size = i32::from_le_bytes(
            header_size_buffer[8..12]
                .try_into()
                .expect("Parsing header size to i32 failed"),
        );
        file.rewind()?;
        let mut file_buffer = vec![0; header_size as usize];
        file.read_exact(&mut file_buffer)?;

        let header: Header = StreamFile::parse_header(&file_buffer)?;

        let file_size_no_header = file_size as i64 - header_size as i64;

        let db_count = (file_size_no_header as u64)
            .div_ceil((header.db_size + header.db_header_size) as u64)
            as i32;
        let sample_count = (file_size_no_header as i32 - db_count * header.db_header_size)
            / header.bytes_per_sample;
        let spec_count = sample_count / header.frq_bands;

        Ok(Self {
            filepath: path.to_owned(),
            header_size: header.header_size,
            project_id: header.project_id,
            channel: header.channel,
            process: header.process,
            datamode: header.datamode,
            datakind: header.datakind,
            datatype: header.datatype,
            process_time: header.process_time,
            process_datetime: header.process_datetime,
            db_header_size: header.db_header_size,
            bytes_per_sample: header.bytes_per_sample,
            db_size: header.db_size,
            frq_bands: header.frq_bands,
            compression_frq: header.compression_frq,
            compression_time: header.compression_time,
            avg_time: header.avg_time,
            avg_frq: header.avg_frq,
            spec_duration: header.spec_duration,
            frq_start: header.frq_start,
            frq_end: header.frq_end, // This is optional but defaults to frq_bands * frq_per_band
            frq_per_band: header.frq_per_band,
            adc_type: header.adc_type,
            bit_resolution: header.bit_resolution,
            fft_log_shift: header.fft_log_shift,
            max_amplitude_level: header.max_amplitude_level,
            streamno: header.streamno,
            preamp_gain: header.preamp_gain,
            db_count,
            db_sample_count: header.db_sample_count,
            db_spec_count: header.db_spec_count,
            sample_count,
            spec_count,
            file_size,
            hash: header.hash,
        })
    }
}

impl StreamFile {
    pub fn parse_header(buf: &[u8]) -> Result<Header, ParseError> {
        let mut i: usize = 0;
        let mut h = HeaderBuilder::default();
        loop {
            let key: &[u8; 8] = buf
                .get(i..i + 8)
                .ok_or(ParseError::UnexpectedEof)?
                .try_into()
                .unwrap();
            if key == b"headsend" {
                break;
            }
            let at = i + 8; // already append the key width to the index
            let value_size = match key {
                b"qassdata" => {
                    h.header_size = Some(read_i32(buf, at)?);
                    12
                }
                b"proj__id" => {
                    h.project_id = Some(read_u64(buf, at)?);
                    16
                }
                b"proc_cnt" => {
                    h.process = Some(read_i32(buf, at)?);
                    12
                }
                b"dumpchan" => {
                    h.channel = Some(read_i32(buf, at)?);
                    12
                }
                b"datamode" => {
                    h.datamode = Some(read_i32(buf, at)?.into());
                    12
                }
                b"datakind" => {
                    h.datakind = Some(read_i32(buf, at)?.into());
                    12
                }
                b"datatype" => {
                    h.datatype = Some(read_i32(buf, at)?.into());
                    12
                }
                b"proctime" => {
                    let time = read_u32(buf, at)?;
                    h.process_time = Some(time);
                    h.process_datetime = DateTime::from_timestamp_millis(time.into());
                    12
                }
                b"dbhdsize" => {
                    h.db_header_size = Some(read_i32(buf, at)?);
                    12
                }
                b"b_p_samp" => {
                    h.bytes_per_sample = Some(read_i32(buf, at)?);
                    12
                }
                b"db__size" => {
                    h.db_size = Some(read_i32(buf, at)?);
                    12
                }
                b"s_p_fram" => {
                    h.frq_bands = Some(read_i32(buf, at)?);
                    12
                }
                b"frqratio" => {
                    h.compression_frq = Some(read_i32(buf, at)?);
                    12
                }
                b"comratio" => {
                    h.compression_time = Some(read_i32(buf, at)?);
                    12
                }
                b"auxpara1" => {
                    h.avg_time = Some(read_i32(buf, at)?);
                    12
                }
                b"auxpara2" => {
                    h.avg_frq = Some(read_i32(buf, at)?);
                    12
                }
                b"framedur" => {
                    h.spec_duration = Some(read_f64(buf, at)?);
                    12
                }
                b"startfrq" => {
                    h.frq_start = Some(read_i32(buf, at)?);
                    12
                }
                b"end__frq" => {
                    h.frq_end = Some(read_i32(buf, at)?);
                    12
                }
                b"frqpband" => {
                    h.frq_per_band = Some(read_f64(buf, at)?);
                    16
                }
                b"adc_type" => {
                    h.adc_type = Some(read_i32(buf, at)?.into());
                    12
                }
                b"adbitres" => {
                    h.bit_resolution = Some(read_i32(buf, at)?);
                    12
                }
                b"fftlogsh" => {
                    h.fft_log_shift = Some(read_i32(buf, at)?);
                    12
                }
                b"max_ampl" => {
                    h.max_amplitude_level = Some(read_i32(buf, at)?);
                    12
                }
                b"streamno" => {
                    h.streamno = Some(read_u32(buf, at)?);
                    12
                }
                b"pampgain" => {
                    h.preamp_gain = Some(read_i32(buf, at)?);
                    12
                }
                _ => 1,
            };

            i += value_size;
        }

        let mut context = Context::new(&SHA256);
        context.update(buf);
        let hash = context.finish();
        h.hash = Some(hash);
        h.build()
    }

    pub fn get_hash(&self) -> Result<Digest, std::io::Error> {
        let mut f = File::open(&self.filepath)?;
        let mut buffer = vec![0; self.header_size as usize];
        let _ = f.read_exact(&mut buffer);
        let mut context = Context::new(&SHA256);
        context.update(&buffer);
        let hash = context.finish();
        Ok(hash)
    }

    pub fn get_data(
        &self,
        start_spec: i32,
        end_spec: i32,
    ) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        let start_byte = start_spec * self.frq_bands * self.bytes_per_sample;
        let end_byte = end_spec * self.frq_bands * self.bytes_per_sample;

        let start_block = start_byte / self.db_size;
        let end_block = (end_byte as u32).div_ceil(self.db_size as u32) as i32;

        let mut result: Vec<u32> = vec![0; ((end_spec - start_spec) * self.frq_bands) as usize];

        let mut file = File::open(&self.filepath)?;

        let mut result_idx: usize = 0;

        for datablock in start_block..end_block {
            let block_start_idx = if datablock == start_block {
                start_byte - start_block * self.db_size
            } else {
                0
            };
            let block_end_idx = if datablock == end_block - 1 {
                end_byte - (end_block - 1) * self.db_size
            } else {
                self.db_size
            };

            let block_file_pos =
                self.header_size + datablock * (self.db_size + self.db_header_size);
            let file_pos = block_file_pos + block_start_idx + self.db_header_size;
            let file_buffer_size = block_end_idx - block_start_idx;

            let mut file_buffer = vec![0; file_buffer_size as usize];
            let _ = file.seek(std::io::SeekFrom::Start(file_pos as u64));
            let _ = file.read_exact(&mut file_buffer);

            for chunk in file_buffer.chunks_exact(self.bytes_per_sample as usize) {
                match chunk.len() {
                    2 => result[result_idx] = u32::from_le_bytes([chunk[0], chunk[1], 0, 0]),
                    4 => {
                        result[result_idx] =
                            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                    }
                    _ => (),
                }
                result_idx += 1;
            }
        }
        //let shape = ((end_spec - start_spec) as usize, self.frq_bands as usize);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let compressed_buffer_header: Vec<u8> = vec![
            113, 97, 115, 115, 100, 97, 116, 97, 208, 7, 0, 0, 102, 105, 108, 101, 118, 101, 114,
            115, 26, 0, 0, 0, 97, 110, 52, 100, 118, 101, 114, 115, 32, 153, 3, 2, 115, 97, 118,
            101, 102, 114, 111, 109, 2, 0, 0, 0, 100, 97, 116, 97, 109, 111, 100, 101, 2, 0, 0, 0,
            100, 97, 116, 97, 116, 121, 112, 101, 13, 0, 0, 0, 100, 97, 116, 97, 107, 105, 110,
            100, 255, 255, 255, 255, 109, 95, 117, 95, 109, 97, 115, 107, 8, 0, 0, 0, 0, 0, 0, 0,
            98, 95, 112, 95, 115, 97, 109, 112, 2, 0, 0, 0, 115, 95, 112, 95, 102, 114, 97, 109,
            64, 0, 0, 0, 100, 98, 95, 95, 115, 105, 122, 101, 0, 0, 15, 0, 109, 97, 120, 95, 97,
            109, 112, 108, 0, 0, 1, 0, 115, 97, 109, 112, 108, 101, 114, 116, 8, 175, 47, 0, 100,
            97, 116, 97, 114, 97, 116, 101, 0, 0, 0, 0, 132, 215, 23, 65, 115, 97, 109, 112, 108,
            101, 102, 114, 194, 235, 11, 0, 102, 114, 113, 115, 104, 105, 102, 116, 7, 0, 0, 0,
            102, 102, 116, 111, 118, 101, 114, 115, 3, 0, 0, 0, 102, 102, 116, 108, 111, 103, 115,
            104, 14, 0, 0, 0, 102, 102, 116, 119, 105, 110, 102, 117, 0, 0, 0, 0, 100, 98, 104,
            100, 115, 105, 122, 101, 160, 128, 2, 0, 99, 111, 109, 114, 97, 116, 105, 111, 8, 0, 0,
            0, 102, 114, 113, 114, 97, 116, 105, 111, 8, 0, 0, 0, 102, 99, 95, 95, 114, 101, 97,
            108, 0, 0, 0, 0, 0, 0, 32, 64, 112, 114, 111, 106, 95, 95, 105, 100, 6, 224, 86, 102,
            12, 0, 0, 0, 102, 105, 108, 101, 95, 95, 105, 100, 253, 143, 131, 102, 10, 238, 97,
            121, 112, 97, 114, 101, 110, 116, 105, 100, 253, 143, 131, 102, 6, 83, 97, 121, 112,
            114, 111, 99, 95, 99, 110, 116, 11, 35, 0, 0, 112, 111, 108, 121, 95, 99, 110, 116, 0,
            0, 0, 0, 100, 117, 109, 112, 99, 104, 97, 110, 0, 0, 0, 0, 112, 114, 111, 99, 116, 105,
            109, 101, 253, 143, 131, 102, 108, 109, 111, 100, 116, 105, 109, 101, 253, 143, 131,
            102, 101, 112, 111, 99, 116, 105, 109, 101, 151, 117, 234, 113, 144, 1, 0, 0, 109, 117,
            120, 95, 112, 111, 114, 116, 0, 0, 0, 0, 112, 97, 109, 112, 103, 97, 105, 110, 251, 3,
            0, 0, 100, 105, 115, 112, 103, 97, 105, 110, 32, 3, 0, 0, 108, 105, 110, 102, 103, 97,
            105, 110, 96, 109, 0, 0, 97, 117, 120, 112, 97, 114, 97, 49, 4, 0, 0, 0, 97, 117, 120,
            112, 97, 114, 97, 50, 4, 0, 0, 0, 115, 107, 105, 112, 115, 97, 109, 112, 0, 0, 0, 0, 0,
            0, 0, 0, 115, 107, 105, 112, 116, 105, 109, 101, 0, 0, 0, 0, 0, 0, 0, 0, 116, 114, 117,
            110, 115, 97, 109, 112, 0, 0, 0, 0, 0, 0, 0, 0, 116, 114, 117, 110, 116, 105, 109, 101,
            0, 0, 0, 0, 0, 0, 0, 0, 115, 107, 105, 112, 108, 102, 114, 113, 0, 0, 0, 0, 116, 114,
            117, 110, 104, 102, 114, 113, 0, 0, 0, 0, 102, 114, 113, 112, 98, 97, 110, 100, 0, 0,
            0, 0, 0, 208, 183, 64, 102, 114, 97, 109, 101, 100, 117, 114, 0, 0, 0, 0, 0, 0, 52, 65,
            112, 95, 95, 102, 108, 97, 103, 115, 0, 0, 0, 0, 114, 101, 97, 108, 102, 114, 113, 99,
            1, 0, 0, 0, 115, 117, 98, 95, 100, 97, 116, 97, 1, 0, 0, 0, 115, 100, 95, 100, 105,
            109, 101, 110, 0, 16, 0, 0, 115, 100, 95, 114, 115, 105, 122, 101, 240, 0, 0, 0, 115,
            100, 95, 114, 115, 105, 122, 102, 0, 0, 0, 0, 0, 0, 110, 64, 115, 100, 95, 100, 115,
            105, 122, 101, 80, 0, 0, 0, 97, 100, 99, 95, 116, 121, 112, 101, 1, 0, 0, 0, 97, 100,
            98, 105, 116, 114, 101, 115, 16, 0, 0, 0, 105, 110, 116, 101, 114, 112, 111, 108, 0, 0,
            0, 0, 100, 105, 115, 112, 114, 97, 104, 105, 208, 7, 0, 0, 112, 97, 114, 116, 110, 111,
            105, 100, 4, 0, 0, 0, 97, 115, 99, 95, 112, 97, 114, 116, 4, 0, 0, 0, 49, 95, 49, 0,
            99, 111, 109, 109, 101, 110, 116, 115, 28, 0, 0, 0, 98, 97, 115, 101, 100, 32, 111,
            110, 58, 32, 80, 56, 57, 55, 49, 95, 67, 49, 32, 70, 70, 84, 58, 82, 97, 119, 0, 0,
            115, 112, 97, 114, 101, 109, 101, 109, 148, 4, 0, 0, 66, 67, 80, 58, 123, 78, 67, 78,
            84, 58, 49, 59, 78, 69, 83, 84, 58, 91, 78, 76, 69, 86, 58, 49, 59, 78, 73, 68, 88, 58,
            48, 59, 66, 67, 80, 58, 123, 99, 104, 58, 48, 59, 100, 109, 58, 50, 59, 100, 116, 58,
            48, 59, 115, 112, 102, 58, 53, 49, 50, 59, 97, 100, 99, 98, 114, 58, 49, 54, 59, 114,
            97, 116, 101, 58, 51, 49, 50, 53, 48, 48, 48, 59, 115, 102, 114, 113, 58, 55, 56, 49,
            50, 53, 48, 125, 93, 125, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 104, 101, 97, 100, 115, 101, 110, 100,
        ];
        let header: Header =
            StreamFile::parse_header(&compressed_buffer_header).expect("Parsing Header failed");
        assert_eq!(header.header_size, 2000);
        assert!(matches!(header.datamode, Datamode::FFT));
        assert!(matches!(header.datatype, Datatype::MovingAverageFrq));
        assert!(matches!(header.datakind, Datakind::Undef));
        assert_eq!(header.bytes_per_sample, 2);
        assert_eq!(header.frq_bands, 64);
        assert_eq!(header.db_size, 983040);
        assert_eq!(header.max_amplitude_level, 65536);
    }
}
