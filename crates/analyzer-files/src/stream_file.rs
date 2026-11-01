use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::data::Sample;
use crate::{
    Datamode, Header, ParseError, ReadError, SampleType, Samples, Signal, Spectrogram, StreamData,
};

const MAGIC: &[u8; 8] = b"qassdata";
/// Headers are 2000 bytes in practice; anything far beyond that is not a stream file.
const MAX_HEADER_SIZE: u64 = 1 << 20;

/// An opened Analyzer4D stream file ("buffer").
///
/// The file is a header followed by data blocks. Each block is a block header of
/// `db_header_size` bytes followed by `db_size` bytes of samples; the last block may be
/// partially filled. Spectra never span two blocks.
#[derive(Debug)]
pub struct StreamFile {
    path: PathBuf,
    file: File,
    header: Header,
    file_size: u64,
    db_count: u64,
    spec_count: u64,
}

impl StreamFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ParseError> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len();

        let mut start = [0u8; 12];
        file.read_exact(&mut start).map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof => ParseError::NotAStreamFile,
            _ => e.into(),
        })?;
        if &start[..8] != MAGIC {
            return Err(ParseError::NotAStreamFile);
        }
        let header_size = i32::from_le_bytes(start[8..].try_into().expect("4 bytes"));
        let header_size = u64::try_from(header_size)
            .ok()
            .filter(|&size| size >= 12 && size <= file_size.min(MAX_HEADER_SIZE))
            .ok_or_else(|| ParseError::invalid("qassdata", format!("header size {header_size}")))?;

        let mut buf = vec![0u8; header_size as usize];
        buf[..12].copy_from_slice(&start);
        file.read_exact(&mut buf[12..])?;
        let header = Header::parse(&buf)?;

        let (db_count, spec_count) = layout(&header, file_size);
        Ok(Self {
            path: path.to_owned(),
            file,
            header,
            file_size,
            db_count,
            spec_count,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Number of data blocks that contain data.
    pub fn db_count(&self) -> u64 {
        self.db_count
    }

    /// Number of complete spectra in the file (samples for signal files).
    pub fn spec_count(&self) -> u64 {
        self.spec_count
    }

    /// Duration of the whole file in ns.
    pub fn duration(&self) -> f64 {
        self.spec_count as f64 * self.header.spec_duration
    }

    /// Reads the spectra (samples for signal files) in `range`.
    pub fn read(&self, range: Range<u64>) -> Result<StreamData, ReadError> {
        self.check_range(&range)?;
        let h = &self.header;
        let samples = match h.sample_type {
            SampleType::U16 => Samples::U16(self.read_samples(range.clone())?),
            SampleType::U32 => Samples::U32(self.read_samples(range.clone())?),
            SampleType::F32 => Samples::F32(self.read_samples(range.clone())?),
        };
        match h.datamode {
            Datamode::Fft => Ok(StreamData::Spectrogram(Spectrogram::new(
                samples,
                range.start,
                h.frq_bands as usize,
                h.spec_duration,
                h.frq_start,
                h.frq_per_band,
            ))),
            Datamode::Signal => Ok(StreamData::Signal(Signal::new(
                samples,
                range.start,
                h.spec_duration,
                h.zero_amplitude_level.map(f64::from),
            ))),
            mode => Err(ReadError::UnsupportedDatamode(mode)),
        }
    }

    /// Reads the whole file.
    pub fn read_all(&self) -> Result<StreamData, ReadError> {
        self.read(0..self.spec_count)
    }

    /// Appends the spectra (samples for signal files) in `range` to `out`, converted to `f32`.
    ///
    /// Unlike [`read`](Self::read) this allocates nothing when `out` has enough capacity, which
    /// matters when many threads read concurrently: large allocations are zero-filled by page
    /// faults that serialize in the kernel. `u32` values above 2^24 are rounded.
    pub fn read_f32(&self, range: Range<u64>, out: &mut Vec<f32>) -> Result<(), ReadError> {
        self.check_range(&range)?;
        out.reserve((range.end - range.start) as usize * self.header.frq_bands as usize);
        match self.header.sample_type {
            SampleType::U16 => self.for_each_chunk(range, |bytes| {
                let (samples, _) = bytes.as_chunks::<2>();
                out.extend(samples.iter().map(|&b| f32::from(u16::from_le_bytes(b))));
            }),
            SampleType::U32 => self.for_each_chunk(range, |bytes| {
                let (samples, _) = bytes.as_chunks::<4>();
                out.extend(samples.iter().map(|&b| u32::from_le_bytes(b) as f32));
            }),
            SampleType::F32 => self.for_each_chunk(range, |bytes| {
                let (samples, _) = bytes.as_chunks::<4>();
                out.extend(samples.iter().map(|&b| f32::from_le_bytes(b)));
            }),
        }
    }

    fn check_range(&self, range: &Range<u64>) -> Result<(), ReadError> {
        if range.start > range.end || range.end > self.spec_count {
            return Err(ReadError::OutOfRange {
                range: range.clone(),
                available: self.spec_count,
            });
        }
        Ok(())
    }

    fn read_samples<T: Sample>(&self, range: Range<u64>) -> Result<Vec<T>, ReadError> {
        debug_assert_eq!(T::SAMPLE_TYPE, self.header.sample_type);
        let samples = (range.end - range.start) as usize * self.header.frq_bands as usize;
        let mut out = Vec::with_capacity(samples);
        self.for_each_chunk(range, |bytes| {
            out.extend(
                bytes
                    .chunks_exact(T::SAMPLE_TYPE.size())
                    .map(T::from_le_slice),
            );
        })?;
        Ok(out)
    }

    /// Reads `range` block by block and passes the raw sample bytes of every block to `visit`,
    /// in order. Uses a reused per-thread buffer.
    fn for_each_chunk(
        &self,
        range: Range<u64>,
        mut visit: impl FnMut(&[u8]),
    ) -> Result<(), ReadError> {
        thread_local! {
            static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        }
        let h = &self.header;
        let spec_bytes = h.spec_bytes();
        let specs_per_block = h.db_spec_count();
        let block_len = u64::from(h.db_header_size) + u64::from(h.db_size);

        BUFFER.with_borrow_mut(|buf| {
            let mut spec = range.start;
            while spec < range.end {
                let block = spec / specs_per_block;
                let in_block = spec % specs_per_block;
                let count = (range.end - spec).min(specs_per_block - in_block);
                let offset = u64::from(h.header_size)
                    + block * block_len
                    + u64::from(h.db_header_size)
                    + in_block * spec_bytes;
                let len = (count * spec_bytes) as usize;
                if buf.len() < len {
                    buf.resize(len, 0);
                }
                read_exact_at(&self.file, &mut buf[..len], offset)?;
                visit(&buf[..len]);
                spec += count;
            }
            Ok(())
        })
    }
}

/// Returns `(db_count, spec_count)` for a file of `file_size` bytes. Trailing bytes that do not
/// form a complete spectrum (e.g. a file that is still being written) are ignored.
fn layout(header: &Header, file_size: u64) -> (u64, u64) {
    let db_header_size = u64::from(header.db_header_size);
    let block_len = db_header_size + u64::from(header.db_size);
    let data_len = file_size - u64::from(header.header_size);
    let full_blocks = data_len / block_len;
    let partial_bytes = (data_len % block_len).saturating_sub(db_header_size);
    let partial_specs = partial_bytes / header.spec_bytes();
    let db_count = full_blocks + u64::from(partial_specs > 0);
    (
        db_count,
        full_blocks * header.db_spec_count() + partial_specs,
    )
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, offset) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const HEADER: &[u8] = include_bytes!("../testdata/compressed-header.bin");
    const DB_HEADER_SIZE: usize = 16;

    fn patch(buf: &mut [u8], key: &[u8; 8], value: &[u8]) {
        let at = buf
            .windows(8)
            .position(|w| w == key)
            .expect("key present in fixture");
        buf[at + 8..at + 8 + value.len()].copy_from_slice(value);
    }

    /// Writes a stream file with `bands` bands, `bytes_per_sample` and `specs_per_block` spectra
    /// per block. `samples` are the raw sample bytes, split into blocks.
    fn write_file(
        datamode: Datamode,
        bands: i32,
        bytes_per_sample: i32,
        float: bool,
        specs_per_block: i32,
        samples: &[u8],
    ) -> tempfile::NamedTempFile {
        let db_size = bands * bytes_per_sample * specs_per_block;
        let mut header = HEADER.to_vec();
        patch(&mut header, b"datamode", &datamode.as_i32().to_le_bytes());
        patch(&mut header, b"s_p_fram", &bands.to_le_bytes());
        patch(&mut header, b"b_p_samp", &bytes_per_sample.to_le_bytes());
        patch(&mut header, b"db__size", &db_size.to_le_bytes());
        patch(
            &mut header,
            b"dbhdsize",
            &(DB_HEADER_SIZE as i32).to_le_bytes(),
        );
        patch(
            &mut header,
            b"p__flags",
            &(if float { 8i32 } else { 0 }).to_le_bytes(),
        );

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&header).unwrap();
        for block in samples.chunks(db_size as usize) {
            file.write_all(&[0xAB; DB_HEADER_SIZE]).unwrap();
            file.write_all(block).unwrap();
        }
        file.flush().unwrap();
        file
    }

    fn u16_bytes(values: impl IntoIterator<Item = u16>) -> Vec<u8> {
        values.into_iter().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn reads_across_block_boundaries() {
        // 3 bands, 4 spectra per block, 10 spectra -> blocks of 4, 4 and a partial one of 2
        let values: Vec<u16> = (0..30).collect();
        let file = write_file(Datamode::Fft, 3, 2, false, 4, &u16_bytes(values.clone()));
        let stream = StreamFile::open(file.path()).unwrap();
        assert_eq!(stream.spec_count(), 10);
        assert_eq!(stream.db_count(), 3);

        let StreamData::Spectrogram(all) = stream.read_all().unwrap() else {
            panic!("expected a spectrogram");
        };
        assert_eq!(all.shape(), (10, 3));
        assert_eq!(all.samples(), &Samples::U16(values));

        let StreamData::Spectrogram(part) = stream.read(3..9).unwrap() else {
            panic!("expected a spectrogram");
        };
        assert_eq!(part.first_spec(), 3);
        assert_eq!(part.shape(), (6, 3));
        assert_eq!(part.get(0, 0), Some(9.0));
        assert_eq!(part.get(5, 2), Some(26.0));
    }

    #[test]
    fn empty_and_out_of_range_reads() {
        let file = write_file(Datamode::Fft, 2, 2, false, 2, &u16_bytes(0..8));
        let stream = StreamFile::open(file.path()).unwrap();
        assert_eq!(stream.read(4..4).unwrap().samples().len(), 0);
        assert!(matches!(
            stream.read(3..5),
            Err(ReadError::OutOfRange { available: 4, .. })
        ));
    }

    #[test]
    fn ignores_incomplete_trailing_spectrum() {
        // 2 complete spectra of 2 bands plus one extra sample
        let file = write_file(Datamode::Fft, 2, 2, false, 4, &u16_bytes(0..5));
        let stream = StreamFile::open(file.path()).unwrap();
        assert_eq!(stream.spec_count(), 2);
    }

    #[test]
    fn read_f32_matches_read() {
        let file = write_file(Datamode::Fft, 3, 2, false, 4, &u16_bytes(0..30));
        let stream = StreamFile::open(file.path()).unwrap();
        let mut out = vec![-1.0];
        stream.read_f32(2..9, &mut out).unwrap();
        let expected: Vec<f32> = std::iter::once(-1.0)
            .chain((6..27).map(|v| v as f32))
            .collect();
        assert_eq!(out, expected);
        assert!(stream.read_f32(9..11, &mut out).is_err());
    }

    #[test]
    fn reads_float_samples() {
        let values = [1.5f32, -2.25, 3.0, 4.0];
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let file = write_file(Datamode::Fft, 2, 4, true, 1, &bytes);
        let stream = StreamFile::open(file.path()).unwrap();
        assert_eq!(
            stream.read_all().unwrap().samples(),
            &Samples::F32(values.to_vec())
        );
    }

    #[test]
    fn reads_signal() {
        let file = write_file(Datamode::Signal, 1, 2, false, 3, &u16_bytes(100..107));
        let stream = StreamFile::open(file.path()).unwrap();
        let StreamData::Signal(signal) = stream.read(2..6).unwrap() else {
            panic!("expected a signal");
        };
        assert_eq!(signal.first_sample(), 2);
        assert_eq!(signal.samples(), &Samples::U16(vec![102, 103, 104, 105]));
    }

    #[test]
    fn rejects_non_stream_files() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"definitely not a stream file").unwrap();
        assert!(matches!(
            StreamFile::open(file.path()),
            Err(ParseError::NotAStreamFile)
        ));
        let empty = tempfile::NamedTempFile::new().unwrap();
        assert!(matches!(
            StreamFile::open(empty.path()),
            Err(ParseError::NotAStreamFile)
        ));
    }
}
