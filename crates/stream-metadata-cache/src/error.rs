use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("a row with that path already exists")]
    Duplicate,

    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("background task failed: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("database error: {0}")]
    Database(#[source] sqlx::Error),
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db) = e
            && db.is_unique_violation()
        {
            return StoreError::Duplicate;
        }
        StoreError::Database(e)
    }
}
