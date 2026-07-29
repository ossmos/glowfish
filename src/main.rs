use analyzer_files::stream_file::StreamFile;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;

mod error;
mod filter;
mod metadata;
mod scan;
mod store;
use error::StoreError;
use metadata::StreamFileMetadata;
use store::{MetadataStore, SqliteStore};

#[tokio::main]
async fn main() -> Result<(), StoreError> {
    let store = SqliteStore::connect("sqlite://test.db").await?;
    store.migrate().await?;
    let _ = store
        .synchronize_directory(PathBuf::from("/home/ossmos/projects/rust/analyzer-files"))
        .await?;
    Ok(())
}

//let opts = SqliteConnectOptions::from_str("sqlite://test.db")?.create_if_missing(true);
//let pool = SqlitePool::connect_with(opts).await?;
//let file = StreamFile::new_dep(PathBuf::from(
//    "/home/ossmos/projects/rust/analyzer-files/compressed_buffer",
//))
//.unwrap();
//println!("{:?}", file);
//
//// TODO: add header hash
//sqlx::query(
//    r#"
//    CREATE TABLE IF NOT EXISTS buffer_metadata (
//        id INTEGER PRIMARY KEY AUTOINCREMENT,
//        filepath TEXT NOT NULL UNIQUE,
//        header_size INTEGER NOT NULL,
//        project_id INTEGER NOT NULL,
//        channel INTEGER NOT NULL,
//        process_time INTEGER NOT NULL,
//        process_datetime TIMESTAMP NOT NULL,
//        db_header_size INTEGER NOT NULL,
//        bytes_per_sample INTEGER NOT NULL,
//        db_size INTEGER NOT NULL,
//        frq_bands INTEGER NOT NULL,
//        compression_time INTEGER NOT NULL,
//        compression_frq INTEGER NOT NULL,
//        avg_time INTEGER NOT NULL,
//        avg_frq INTEGER NOT NULL,
//        spec_duration INTEGER NOT NULL,
//        frq_start INTEGER NOT NULL,
//        frq_end INTEGER NOT NULL,
//        frq_per_band INTEGER NOT NULL,
//        bit_resolution INTEGER NOT NULL,
//        fft_log_shift INTEGER NOT NULL,
//        streamno INTEGER NOT NULL,
//        preamp_gain INTEGER NOT NULL,
//        file_size INTEGER NOT NULL,
//        db_count INTEGER NOT NULL,
//        db_sample_count INTEGER NOT NULL,
//        db_spec_count INTEGER NOT NULL,
//        sample_count INTEGER NOT NULL,
//        spec_count INTEGER NOT NULL
//    );
//    "#,
//)
//.execute(&pool)
//.await?;
//
//let metadata = StreamFileMetadata::from(file);
//let _rec = sqlx::query(
//    r#"
//    INSERT INTO buffer_metadata
//    (
//        filepath,
//        header_size,
//        project_id,
//        channel,
//        process_time,
//        process_datetime,
//        db_header_size,
//        bytes_per_sample,
//        db_size,
//        frq_bands,
//        compression_time,
//        compression_frq,
//        avg_time,
//        avg_frq,
//        spec_duration,
//        frq_start,
//        frq_end,
//        frq_per_band,
//        bit_resolution,
//        fft_log_shift,
//        streamno,
//        preamp_gain,
//        file_size,
//        db_count,
//        db_sample_count,
//        db_spec_count,
//        sample_count,
//        spec_count
//    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
//    "#,
//)
//.bind(&metadata.filepath)
//.bind(metadata.header_size)
//.bind(metadata.project_id as i64)
//.bind(metadata.channel)
//.bind(metadata.process_time)
//.bind(metadata.process_datetime)
//.bind(metadata.db_header_size)
//.bind(metadata.bytes_per_sample)
//.bind(metadata.db_size)
//.bind(metadata.frq_bands)
//.bind(metadata.compression_time)
//.bind(metadata.compression_frq)
//.bind(metadata.avg_time)
//.bind(metadata.avg_frq)
//.bind(metadata.spec_duration as i64)
//.bind(metadata.frq_start)
//.bind(metadata.frq_end)
//.bind(metadata.frq_per_band)
//.bind(metadata.bit_resolution)
//.bind(metadata.fft_log_shift)
//.bind(metadata.streamno)
//.bind(metadata.preamp_gain)
//.bind(metadata.file_size as i64)
//.bind(metadata.db_count)
//.bind(metadata.db_sample_count)
//.bind(metadata.db_spec_count)
//.bind(metadata.sample_count)
//.bind(metadata.spec_count)
//.execute(&pool)
//.await?;
//
//Ok(())
