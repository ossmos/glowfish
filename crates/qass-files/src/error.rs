use std::ops::Range;

use crate::Datamode;

/// Errors raised while opening a stream file and parsing its header.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the file is not a stream file (missing \"qassdata\" magic)")]
    NotAStreamFile,
    #[error("unexpected end of header at byte {0}")]
    UnexpectedEof(usize),
    #[error("header does not end with \"headsend\" within {0} bytes")]
    MissingHeaderEnd(usize),
    #[error("unknown header key {key:?} at byte {offset}")]
    UnknownKey { key: String, offset: usize },
    #[error("duplicate header key {0}")]
    DuplicateKey(&'static str),
    #[error("missing header key {0}")]
    MissingKey(&'static str),
    #[error("invalid value for {key}: {reason}")]
    InvalidValue { key: &'static str, reason: String },
}

impl ParseError {
    pub(crate) fn invalid(key: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidValue {
            key,
            reason: reason.into(),
        }
    }
}

/// Errors raised while reading sample data from an opened stream file.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("range {range:?} is outside of the available spectra 0..{available}")]
    OutOfRange { range: Range<u64>, available: u64 },
    #[error("reading data of datamode {0} is not supported")]
    UnsupportedDatamode(Datamode),
}
