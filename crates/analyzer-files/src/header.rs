use chrono::{DateTime, Utc};
use ring::digest::{SHA256, digest};

use crate::{AdcType, Datakind, Datamode, Datatype, ParseError};

/// Length of every header key in bytes.
const KEY_LEN: usize = 8;
/// `p__flags` bit that marks 4 byte samples as `f32` instead of `u32`.
const FLAG_FLOAT_SAMPLES: u32 = 1 << 3;

/// The in-memory type of one stored sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleType {
    U16,
    U32,
    F32,
}

impl SampleType {
    pub fn size(self) -> usize {
        match self {
            SampleType::U16 => 2,
            SampleType::U32 | SampleType::F32 => 4,
        }
    }
}

/// The parsed and validated file header of a stream file.
///
/// Integer fields that must not be negative are stored unsigned; the conversion fails instead of
/// wrapping. Times are in nanoseconds, frequencies in Hz.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub header_size: u32,
    pub project_id: u64,
    pub process: u32,
    pub channel: u32,
    pub datamode: Datamode,
    pub datatype: Datatype,
    pub datakind: Datakind,
    /// Creation time of the file (`proctime`, seconds).
    pub process_time: DateTime<Utc>,
    /// Start of the measurement (`epoctime`, milliseconds). Missing in older files.
    pub measure_start: Option<DateTime<Utc>>,
    pub db_header_size: u32,
    pub db_size: u32,
    pub sample_type: SampleType,
    pub frq_bands: u32,
    pub compression_frq: u32,
    pub compression_time: u32,
    /// Moving average factor over time (`auxpara1`); values < 1 mean no averaging.
    pub avg_time: i32,
    /// Moving average factor over frequency (`auxpara2`); values < 1 mean no averaging.
    pub avg_frq: i32,
    /// Duration of one spectrum (or one sample for signals) in nanoseconds.
    ///
    /// Kept as `f64` because it is not always a whole number, e.g. a 97656 Hz signal has
    /// 10240.026… ns per sample.
    pub spec_duration: f64,
    pub frq_start: f64,
    pub frq_end: f64,
    pub frq_per_band: f64,
    pub adc_type: AdcType,
    pub bit_resolution: u32,
    /// Logarithm shift of FFT data; -1 or 0 if the data is not logarithmized.
    pub fft_log_shift: i32,
    pub max_amplitude_level: u32,
    /// The sample value that represents zero amplitude (`nul_ampl`), e.g. 32768 for signals.
    pub zero_amplitude_level: Option<i32>,
    pub streamno: Option<u32>,
    pub preamp_gain: i32,
    pub flags: u32,
    /// SHA-256 of the raw header bytes; identifies a file independently of its path.
    pub hash: [u8; 32],
}

impl Header {
    /// Bytes of one spectrum (one sample for signals).
    pub fn spec_bytes(&self) -> u64 {
        self.sample_type.size() as u64 * u64::from(self.frq_bands)
    }

    /// Number of spectra in a completely filled data block.
    pub fn db_spec_count(&self) -> u64 {
        u64::from(self.db_size) / self.spec_bytes()
    }

    /// Parses a complete header, `buf` must contain exactly `header_size` bytes.
    pub fn parse(buf: &[u8]) -> Result<Header, ParseError> {
        let mut raw = RawHeader::default();
        let mut at = 0;
        loop {
            let key = read_key(buf, at)?;
            if key == b"headsend" {
                break;
            }
            let value_at = at + KEY_LEN;
            at = match value_kind(key) {
                Some(kind) => {
                    let value = read_value(buf, value_at, kind)?;
                    raw.set(key, value)?;
                    value_at + kind.len() + value.text_len()
                }
                None => skip_unknown_key(buf, at)?,
            };
            if at + KEY_LEN > buf.len() {
                return Err(ParseError::MissingHeaderEnd(buf.len()));
            }
        }
        raw.build(buf)
    }
}

/// The value kinds that occur in a header. Every value is little endian.
#[derive(Debug, Clone, Copy)]
enum Kind {
    I32,
    U32,
    I64,
    U64,
    F64,
    /// A u32 byte length followed by that many bytes.
    Text,
}

impl Kind {
    fn len(self) -> usize {
        match self {
            Kind::I32 | Kind::U32 | Kind::Text => 4,
            Kind::I64 | Kind::U64 | Kind::F64 => 8,
        }
    }
}

/// All header keys known to the reference implementation (`buffer_parser.py`).
fn value_kind(key: &[u8; KEY_LEN]) -> Option<Kind> {
    Some(match key {
        b"qassdata" | b"filevers" | b"datavers" | b"savefrom" | b"datamode" | b"datatype"
        | b"datakind" | b"framsize" | b"smplsize" | b"frqbands" | b"db_words" | b"avgtimba"
        | b"avgfrqba" | b"b_p_samp" | b"s_p_fram" | b"db__size" | b"max_ampl" | b"nul_ampl"
        | b"samplert" | b"samplefr" | b"frqshift" | b"fftovers" | b"fftlogsh" | b"fftwinfu"
        | b"dbhdsize" | b"comratio" | b"frqratio" | b"proc_cnt" | b"proc_rng" | b"proc_sub"
        | b"poly_cnt" | b"polycyid" | b"dumpchan" | b"del_lock" | b"mux_port" | b"pampgain"
        | b"dispgain" | b"linfgain" | b"auxpara0" | b"auxpara1" | b"auxpara2" | b"auxpara3"
        | b"auxpara4" | b"skiplfrq" | b"trunhfrq" | b"startfrq" | b"end__frq" | b"frameoff"
        | b"p__flags" | b"realfrqc" | b"sub_data" | b"sd_dimen" | b"sd_rsize" | b"sd_dsize"
        | b"adc_type" | b"adbitres" | b"interpol" | b"dcoffset" | b"dispralo" | b"disprahi"
        | b"sim_mode" => Kind::I32,
        // an4dvers is a 4 byte version stamp, read as u32
        b"proctime" | b"lmodtime" | b"frqmasks" | b"frqinmas" | b"eheaderf" | b"partnoid"
        | b"streamno" | b"an4dvers" => Kind::U32,
        b"parentid" | b"epoctime" | b"auxpara5" | b"skipsamp" | b"skiptime" | b"trunsamp"
        | b"truntime" | b"baserate" | b"rngtibeg" | b"rngtiend" | b"realsoff" => Kind::I64,
        b"m_u_mask" | b"proj__id" | b"file__id" | b"extendid" => Kind::U64,
        b"datarate" | b"tc__real" | b"fc__real" | b"frqpband" | b"framedur" | b"sd_rsizf" => {
            Kind::F64
        }
        b"asc_part" | b"asc_desc" | b"comments" | b"sparemem" => Kind::Text,
        _ => return None,
    })
}

fn bytes<const N: usize>(buf: &[u8], at: usize) -> Result<[u8; N], ParseError> {
    buf.get(at..at + N)
        .map(|b| b.try_into().expect("slice has length N"))
        .ok_or(ParseError::UnexpectedEof(at))
}

fn read_key(buf: &[u8], at: usize) -> Result<&[u8; KEY_LEN], ParseError> {
    buf.get(at..at + KEY_LEN)
        .map(|b| b.try_into().expect("slice has length KEY_LEN"))
        .ok_or(ParseError::UnexpectedEof(at))
}

#[derive(Debug, Clone, Copy)]
enum Value {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
    Text(u32),
}

impl Value {
    /// Length of the variable sized payload that follows the value.
    fn text_len(self) -> usize {
        match self {
            Value::Text(len) => len as usize,
            _ => 0,
        }
    }
}

fn read_value(buf: &[u8], at: usize, kind: Kind) -> Result<Value, ParseError> {
    Ok(match kind {
        Kind::I32 => Value::I32(i32::from_le_bytes(bytes(buf, at)?)),
        Kind::U32 => Value::U32(u32::from_le_bytes(bytes(buf, at)?)),
        Kind::I64 => Value::I64(i64::from_le_bytes(bytes(buf, at)?)),
        Kind::U64 => Value::U64(u64::from_le_bytes(bytes(buf, at)?)),
        Kind::F64 => Value::F64(f64::from_le_bytes(bytes(buf, at)?)),
        Kind::Text => Value::Text(u32::from_le_bytes(bytes(buf, at)?)),
    })
}

fn is_key_char(c: u8) -> bool {
    c.is_ascii_digit() || (b'_'..=b'z').contains(&c)
}

/// Skips the value of a key that is not in the table. The value width is unknown, so like the
/// reference implementation we accept a 4 or 8 byte value if a plausible key follows it;
/// otherwise the header is rejected instead of guessing further.
fn skip_unknown_key(buf: &[u8], at: usize) -> Result<usize, ParseError> {
    for value_len in [4, 8] {
        let next = at + KEY_LEN + value_len;
        if let Some(key) = buf.get(next..next + KEY_LEN)
            && key.iter().copied().all(is_key_char)
        {
            return Ok(next);
        }
    }
    Err(ParseError::UnknownKey {
        key: String::from_utf8_lossy(&buf[at..at + KEY_LEN]).into_owned(),
        offset: at,
    })
}

/// Header values as read from the file, before validation.
#[derive(Default)]
struct RawHeader {
    header_size: Option<i32>,
    project_id: Option<u64>,
    process: Option<i32>,
    channel: Option<i32>,
    datamode: Option<i32>,
    datatype: Option<i32>,
    datakind: Option<i32>,
    process_time: Option<u32>,
    epoch_time: Option<i64>,
    db_header_size: Option<i32>,
    bytes_per_sample: Option<i32>,
    db_size: Option<i32>,
    frq_bands: Option<i32>,
    compression_frq: Option<i32>,
    compression_time: Option<i32>,
    avg_time: Option<i32>,
    avg_frq: Option<i32>,
    spec_duration: Option<f64>,
    sample_frq: Option<i32>,
    fft_oversampling: Option<i32>,
    frq_start: Option<i32>,
    frq_end: Option<i32>,
    frq_per_band: Option<f64>,
    adc_type: Option<i32>,
    bit_resolution: Option<i32>,
    fft_log_shift: Option<i32>,
    max_amplitude_level: Option<i32>,
    zero_amplitude_level: Option<i32>,
    streamno: Option<u32>,
    preamp_gain: Option<i32>,
    flags: Option<i32>,
}

fn store<T>(slot: &mut Option<T>, name: &'static str, value: T) -> Result<(), ParseError> {
    if slot.replace(value).is_some() {
        return Err(ParseError::DuplicateKey(name));
    }
    Ok(())
}

fn required<T>(value: Option<T>, name: &'static str) -> Result<T, ParseError> {
    value.ok_or(ParseError::MissingKey(name))
}

fn unsigned(value: i32, name: &'static str) -> Result<u32, ParseError> {
    u32::try_from(value).map_err(|_| ParseError::invalid(name, format!("{value} is negative")))
}

fn positive(value: i32, name: &'static str) -> Result<u32, ParseError> {
    match unsigned(value, name)? {
        0 => Err(ParseError::invalid(name, "must not be 0")),
        v => Ok(v),
    }
}

impl RawHeader {
    fn set(&mut self, key: &[u8; KEY_LEN], value: Value) -> Result<(), ParseError> {
        match (key, value) {
            (b"qassdata", Value::I32(v)) => store(&mut self.header_size, "qassdata", v),
            (b"proj__id", Value::U64(v)) => store(&mut self.project_id, "proj__id", v),
            (b"proc_cnt", Value::I32(v)) => store(&mut self.process, "proc_cnt", v),
            (b"dumpchan", Value::I32(v)) => store(&mut self.channel, "dumpchan", v),
            (b"datamode", Value::I32(v)) => store(&mut self.datamode, "datamode", v),
            (b"datatype", Value::I32(v)) => store(&mut self.datatype, "datatype", v),
            (b"datakind", Value::I32(v)) => store(&mut self.datakind, "datakind", v),
            (b"proctime", Value::U32(v)) => store(&mut self.process_time, "proctime", v),
            (b"epoctime", Value::I64(v)) => store(&mut self.epoch_time, "epoctime", v),
            (b"dbhdsize", Value::I32(v)) => store(&mut self.db_header_size, "dbhdsize", v),
            (b"b_p_samp", Value::I32(v)) => store(&mut self.bytes_per_sample, "b_p_samp", v),
            (b"db__size", Value::I32(v)) => store(&mut self.db_size, "db__size", v),
            (b"s_p_fram", Value::I32(v)) => store(&mut self.frq_bands, "s_p_fram", v),
            (b"frqratio", Value::I32(v)) => store(&mut self.compression_frq, "frqratio", v),
            (b"comratio", Value::I32(v)) => store(&mut self.compression_time, "comratio", v),
            (b"auxpara1", Value::I32(v)) => store(&mut self.avg_time, "auxpara1", v),
            (b"auxpara2", Value::I32(v)) => store(&mut self.avg_frq, "auxpara2", v),
            (b"framedur", Value::F64(v)) => store(&mut self.spec_duration, "framedur", v),
            (b"samplefr", Value::I32(v)) => store(&mut self.sample_frq, "samplefr", v),
            (b"fftovers", Value::I32(v)) => store(&mut self.fft_oversampling, "fftovers", v),
            (b"startfrq", Value::I32(v)) => store(&mut self.frq_start, "startfrq", v),
            (b"end__frq", Value::I32(v)) => store(&mut self.frq_end, "end__frq", v),
            (b"frqpband", Value::F64(v)) => store(&mut self.frq_per_band, "frqpband", v),
            (b"adc_type", Value::I32(v)) => store(&mut self.adc_type, "adc_type", v),
            (b"adbitres", Value::I32(v)) => store(&mut self.bit_resolution, "adbitres", v),
            (b"fftlogsh", Value::I32(v)) => store(&mut self.fft_log_shift, "fftlogsh", v),
            (b"max_ampl", Value::I32(v)) => store(&mut self.max_amplitude_level, "max_ampl", v),
            (b"nul_ampl", Value::I32(v)) => store(&mut self.zero_amplitude_level, "nul_ampl", v),
            (b"streamno", Value::U32(v)) => store(&mut self.streamno, "streamno", v),
            (b"pampgain", Value::I32(v)) => store(&mut self.preamp_gain, "pampgain", v),
            (b"p__flags", Value::I32(v)) => store(&mut self.flags, "p__flags", v),
            _ => Ok(()),
        }
    }

    /// Duration of one spectrum in ns: `framedur`, or derived like the reference implementation
    /// for older files that do not store it.
    fn spec_duration(&self, datamode: Datamode, compression_time: u32) -> Result<f64, ParseError> {
        let duration = match self.spec_duration {
            Some(duration) => duration,
            None => {
                let sample_frq = f64::from(required(self.sample_frq, "framedur")?);
                let compression = f64::from(compression_time);
                match datamode {
                    Datamode::Fft => {
                        let oversampling = required(self.fft_oversampling, "framedur")?;
                        let oversampling = 2f64.powi(oversampling);
                        1e9 / (sample_frq * oversampling / 1024.0) * compression
                    }
                    _ => 1e9 / sample_frq * compression,
                }
            }
        };
        if !duration.is_finite() || duration <= 0.0 {
            return Err(ParseError::invalid(
                "framedur",
                format!("{duration} is not a positive duration"),
            ));
        }
        Ok(duration)
    }

    fn build(self, buf: &[u8]) -> Result<Header, ParseError> {
        let header_size = positive(required(self.header_size, "qassdata")?, "qassdata")?;
        if header_size as usize != buf.len() {
            return Err(ParseError::invalid(
                "qassdata",
                format!("header size {header_size} != {} parsed bytes", buf.len()),
            ));
        }

        let datamode = Datamode::try_from(required(self.datamode, "datamode")?)?;
        let datatype = Datatype::try_from(required(self.datatype, "datatype")?)?;
        let datakind = match self.datakind {
            Some(kind) => Datakind::try_from(kind)?,
            None => Datakind::Undef,
        };

        let flags = self.flags.unwrap_or(0) as u32;
        let bytes_per_sample = required(self.bytes_per_sample, "b_p_samp")?;
        let sample_type = match (bytes_per_sample, flags & FLAG_FLOAT_SAMPLES != 0) {
            (2, false) => SampleType::U16,
            (4, false) => SampleType::U32,
            (4, true) => SampleType::F32,
            (bytes, float) => {
                return Err(ParseError::invalid(
                    "b_p_samp",
                    format!("unsupported sample size {bytes} (float flag: {float})"),
                ));
            }
        };

        let frq_bands = positive(required(self.frq_bands, "s_p_fram")?, "s_p_fram")?;
        if datamode == Datamode::Signal && frq_bands != 1 {
            return Err(ParseError::invalid(
                "s_p_fram",
                format!("signal files have 1 sample per frame, not {frq_bands}"),
            ));
        }
        let db_size = positive(required(self.db_size, "db__size")?, "db__size")?;
        let spec_bytes = sample_type.size() as u32 * frq_bands;
        if db_size % spec_bytes != 0 {
            return Err(ParseError::invalid(
                "db__size",
                format!("{db_size} is not a multiple of the spectrum size {spec_bytes}"),
            ));
        }
        let db_header_size = unsigned(required(self.db_header_size, "dbhdsize")?, "dbhdsize")?;

        let compression_frq = positive(required(self.compression_frq, "frqratio")?, "frqratio")?;
        let compression_time = positive(required(self.compression_time, "comratio")?, "comratio")?;
        let spec_duration = self.spec_duration(datamode, compression_time)?;

        let frq_per_band = match (datamode, self.frq_per_band) {
            (Datamode::Fft, None) => return Err(ParseError::MissingKey("frqpband")),
            (Datamode::Fft, Some(v)) if !v.is_finite() || v <= 0.0 => {
                return Err(ParseError::invalid(
                    "frqpband",
                    format!("{v} is not positive"),
                ));
            }
            (_, v) => v.unwrap_or(0.0),
        };
        let frq_start = f64::from(self.frq_start.unwrap_or(0));
        let frq_end = match self.frq_end {
            Some(end) => f64::from(end),
            None => frq_start + f64::from(frq_bands) * frq_per_band,
        };

        let process_time = required(self.process_time, "proctime")?;
        let process_time = DateTime::from_timestamp(i64::from(process_time), 0)
            .ok_or_else(|| ParseError::invalid("proctime", "timestamp out of range"))?;
        let measure_start = self
            .epoch_time
            .map(|ms| {
                DateTime::from_timestamp_millis(ms)
                    .ok_or_else(|| ParseError::invalid("epoctime", "timestamp out of range"))
            })
            .transpose()?;

        Ok(Header {
            header_size,
            project_id: required(self.project_id, "proj__id")?,
            process: unsigned(required(self.process, "proc_cnt")?, "proc_cnt")?,
            channel: unsigned(required(self.channel, "dumpchan")?, "dumpchan")?,
            datamode,
            datatype,
            datakind,
            process_time,
            measure_start,
            db_header_size,
            db_size,
            sample_type,
            frq_bands,
            compression_frq,
            compression_time,
            avg_time: self.avg_time.unwrap_or(1),
            avg_frq: self.avg_frq.unwrap_or(1),
            spec_duration,
            frq_start,
            frq_end,
            frq_per_band,
            adc_type: AdcType::try_from(required(self.adc_type, "adc_type")?)?,
            bit_resolution: unsigned(required(self.bit_resolution, "adbitres")?, "adbitres")?,
            fft_log_shift: required(self.fft_log_shift, "fftlogsh")?,
            max_amplitude_level: unsigned(
                required(self.max_amplitude_level, "max_ampl")?,
                "max_ampl",
            )?,
            zero_amplitude_level: self.zero_amplitude_level,
            streamno: self.streamno,
            preamp_gain: required(self.preamp_gain, "pampgain")?,
            flags,
            hash: digest(&SHA256, buf)
                .as_ref()
                .try_into()
                .expect("SHA-256 digests are 32 bytes"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPRESSED_HEADER: &[u8] = include_bytes!("../testdata/compressed-header.bin");

    #[test]
    fn parses_compressed_header() {
        let header = Header::parse(COMPRESSED_HEADER).expect("parsing header failed");
        assert_eq!(header.header_size, 2000);
        assert_eq!(header.datamode, Datamode::Fft);
        assert_eq!(header.datatype, Datatype::MovingAverageFrq);
        assert_eq!(header.datakind, Datakind::Undef);
        assert_eq!(header.sample_type, SampleType::U16);
        assert_eq!(header.frq_bands, 64);
        assert_eq!(header.db_size, 983040);
        assert_eq!(header.max_amplitude_level, 65536);
        assert_eq!(header.compression_time, 8);
        assert_eq!(header.compression_frq, 8);
        assert_eq!(header.spec_duration, 1_310_720.0);
        assert_eq!(header.frq_per_band, 6096.0);
        assert_eq!(header.frq_end, 64.0 * 6096.0);
        assert_eq!(header.process, 8971);
        assert_eq!(header.channel, 0);
        assert_eq!(header.process_time.timestamp(), 1_719_898_109);
        assert_eq!(
            header.measure_start.map(|t| t.timestamp_millis()),
            Some(1_719_898_109_335)
        );
        assert_eq!(header.db_spec_count(), 983040 / 128);
    }

    fn patched(key: &[u8; 8], value: &[u8]) -> Vec<u8> {
        let mut buf = COMPRESSED_HEADER.to_vec();
        let at = buf
            .windows(8)
            .position(|w| w == key)
            .expect("key present in fixture");
        buf[at + 8..at + 8 + value.len()].copy_from_slice(value);
        buf
    }

    #[test]
    fn rejects_unknown_enum_value() {
        let buf = patched(b"datamode", &99i32.to_le_bytes());
        assert!(matches!(
            Header::parse(&buf),
            Err(ParseError::InvalidValue {
                key: "datamode",
                ..
            })
        ));
    }

    #[test]
    fn rejects_negative_duration() {
        let buf = patched(b"framedur", &(-1.0f64).to_le_bytes());
        assert!(matches!(
            Header::parse(&buf),
            Err(ParseError::InvalidValue {
                key: "framedur",
                ..
            })
        ));
    }

    #[test]
    fn rejects_unsupported_sample_size() {
        let buf = patched(b"b_p_samp", &3i32.to_le_bytes());
        assert!(matches!(
            Header::parse(&buf),
            Err(ParseError::InvalidValue {
                key: "b_p_samp",
                ..
            })
        ));
    }

    #[test]
    fn rejects_wrong_header_size() {
        assert!(Header::parse(&COMPRESSED_HEADER[..1999]).is_err());
    }

    fn renamed(key: &[u8; 8], new_key: &[u8; 8]) -> Vec<u8> {
        let mut buf = COMPRESSED_HEADER.to_vec();
        let at = buf
            .windows(8)
            .position(|w| w == key)
            .expect("key present in fixture");
        buf[at..at + 8].copy_from_slice(new_key);
        buf
    }

    #[test]
    fn skips_unknown_key_with_4_byte_value() {
        let buf = renamed(b"poly_cnt", b"zzzzzzzz");
        let header = Header::parse(&buf).expect("unknown key is skipped");
        assert_eq!(header.process, 8971);
    }

    #[test]
    fn rejects_unknown_key_without_plausible_successor() {
        // "comments" is followed by a length-prefixed string; as an unknown key its payload
        // cannot be skipped reliably
        let buf = renamed(b"comments", b"zzzzzzzz");
        assert!(matches!(
            Header::parse(&buf),
            Err(ParseError::UnknownKey { .. })
        ));
    }
}
