//use sqlx::postgres::Postgres;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

use crate::error::StoreError;
use crate::metadata::StreamFileMetadata;
use crate::scan::scan_dir;

pub struct SqliteStore {
    read_only_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
        let read_only_pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts.clone().read_only(true))
            .await?;
        let write_pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts.read_only(false))
            .await?;
        Ok(Self {
            read_only_pool,
            write_pool,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub inserted: u32,
    pub updated: u32,
    pub removed: u32,
    pub skipped: u32,
}

#[async_trait]
pub trait MetadataStore: Send + Sync {
    async fn migrate(&self) -> Result<(), StoreError>;
    //async fn find_raw(
    //    &self,
    //    sql: &str,
    //    binds: Vec<Value>,
    //) -> Result<Vec<StreamFileMetadata>, StoreError>;
    async fn insert_many(&self, files: &[StreamFileMetadata]) -> Result<(), StoreError>;
    async fn synchronize_directory(&self, path: PathBuf) -> Result<SyncStats, StoreError> {
        let scanned = tokio::task::spawn_blocking(move || scan_dir(&path, false))
            .await
            .map_err(StoreError::Join)??;

        self.insert_many(&scanned).await?;
        //let removed = self.delete_missing(&root, &paths).await?;

        Ok(SyncStats {
            inserted: scanned.len() as u32,
            updated: 0,
            removed: 0,
            skipped: 0,
        })
    }
}

//#[cfg(feature = "sqlite")]
#[async_trait]
impl MetadataStore for SqliteStore {
    async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("migrations/sqlite")
            .run(&self.write_pool)
            .await?;
        Ok(())
    }

    async fn insert_many(&self, files: &[StreamFileMetadata]) -> Result<(), StoreError> {
        let mut tx = self.write_pool.begin().await?;
        for f in files {
            // Maybe QueryBuilder is more performant here
            let _rec = sqlx::query(
                r#"
                INSERT INTO buffer_metadata
                (
                    filepath,
                    header_size,
                    project_id,
                    channel,
                    process_time,
                    process_datetime,
                    db_header_size,
                    bytes_per_sample,
                    db_size,
                    frq_bands,
                    compression_time,
                    compression_frq,
                    avg_time,
                    avg_frq,
                    spec_duration,
                    frq_start,
                    frq_end,
                    frq_per_band,
                    bit_resolution,
                    fft_log_shift,
                    streamno,
                    preamp_gain,
                    file_size,
                    db_count,
                    db_sample_count,
                    db_spec_count,
                    sample_count,
                    spec_count
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
            .bind(&f.filepath)
            .bind(f.header_size)
            .bind(f.project_id as i64)
            .bind(f.channel)
            .bind(f.process_time)
            .bind(f.process_datetime)
            .bind(f.db_header_size)
            .bind(f.bytes_per_sample)
            .bind(f.db_size)
            .bind(f.frq_bands)
            .bind(f.compression_time)
            .bind(f.compression_frq)
            .bind(f.avg_time)
            .bind(f.avg_frq)
            .bind(f.spec_duration as i64)
            .bind(f.frq_start)
            .bind(f.frq_end)
            .bind(f.frq_per_band)
            .bind(f.bit_resolution)
            .bind(f.fft_log_shift)
            .bind(f.streamno)
            .bind(f.preamp_gain)
            .bind(f.file_size as i64)
            .bind(f.db_count)
            .bind(f.db_sample_count)
            .bind(f.db_spec_count)
            .bind(f.sample_count)
            .bind(f.spec_count)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    //async fn find_raw(
    //    &self,
    //    sql: &str,
    //    binds: Vec<Bind>,
    //) -> Result<Vec<StreamFileMetadata>, StoreError> {
    //    let mut q = sqlx::query(sql.to_owned());
    //    for v in binds {
    //        q = match v {
    //            Bind::String(s) => q.bind(s),
    //            Bind::Int(n) => q.bind(n),
    //            Bind::Float(n) => q.bind(n),
    //            Bind::DateTime(d) => q.bind(d),
    //        }
    //    }
    //    Ok(vec![])
    //}
}

pub enum Bind {
    String(String),
    Int(i64),
    Float(f64),
    DateTime(DateTime<Utc>),
}

//#[cfg(feature = "postgres")]
//#[async_trait::async_trait]
//impl Store for PostgresStore {
//    async fn upsert_many(&self, files: &[NewMetadata]) -> Result<(), StoreError> {
//        // Postgres has array binding — one round trip instead of N
//        let paths: Vec<&str> = files
//            .iter()
//            .map(|f| path_to_db(&f.path))
//            .collect::<Result<_, _>>()?;
//        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
//        let sizes: Vec<i64> = files.iter().map(|f| f.size_bytes as i64).collect();
//        let mtimes: Vec<DateTime<Utc>> = files.iter().map(|f| f.modified_at).collect();
//        let dirs: Vec<bool> = files.iter().map(|f| f.is_dir).collect();
//
//        sqlx::query(
//            r#"
//            INSERT INTO file_meta (path, name, size_bytes, modified_at, is_dir)
//            SELECT * FROM UNNEST($1::text[], $2::text[], $3::bigint[], $4::timestamptz[], $5::bool[])
//            ON CONFLICT (path) DO UPDATE SET
//                name        = EXCLUDED.name,
//                size_bytes  = EXCLUDED.size_bytes,
//                modified_at = EXCLUDED.modified_at,
//                is_dir      = EXCLUDED.is_dir
//            "#,
//        )
//        .bind(&paths).bind(&names).bind(&sizes).bind(&mtimes).bind(&dirs)
//        .execute(&self.pool)
//        .await?;
//        Ok(())
//    }
//
//    async fn get_matching_metadata(&self, query: &str) -> Result<Vec<Metadata>, StoreError> {
//        let pattern = format!("%{}%", escape_like(query));
//        let rows = sqlx::query(
//            r#"
//            SELECT id, path, name, extension, size_bytes, modified_at, is_dir
//            FROM file_meta
//            WHERE name ILIKE $1 OR path ILIKE $1
//            ORDER BY path
//            LIMIT 1000
//            "#,
//        )
//        .bind(&pattern)
//        .fetch_all(&self.pool)
//        .await?;
//
//        rows.iter().map(pg_row_to_metadata).collect()
//    }
//}
