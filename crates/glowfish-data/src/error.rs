#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error(transparent)]
    Read(#[from] qass_files::ReadError),
    #[error("loading was cancelled")]
    Cancelled,
    #[error("source error: {0}")]
    Source(#[from] Box<dyn std::error::Error + Send + Sync>),
}
