//! Reading Analyzer4D stream files ("buffers").
//!
//! A port of the reference implementation `buffer_parser.py` with stricter validation: invalid
//! or inconsistent header values are errors instead of being silently converted.

mod data;
mod enums;
mod error;
mod header;
mod stream_file;

pub use data::{Sample, Samples, Signal, Spectrogram, StreamData};
pub use enums::{AdcType, Datakind, Datamode, Datatype};
pub use error::{ParseError, ReadError};
pub use header::{Header, SampleType};
pub use stream_file::StreamFile;
