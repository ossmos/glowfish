//use sqlx::postgres::Postgres;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{QueryBuilder, Sqlite};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::error::StoreError;
use crate::filter::{COLUMNS, MetadataFilter};
use crate::metadata::StreamFileMetadata;
use crate::scan::scan_dir;

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
        // every connection to an in-memory database opens its own, empty database
        let connections = if url.contains(":memory:") { 1 } else { 5 };
        let pool = SqlitePoolOptions::new()
            .max_connections(connections)
            .connect_with(opts)
            .await?;
        Ok(Self { pool })
    }

    pub async fn from_pool(pool: SqlitePool) -> Result<Self, StoreError> {
        Ok(Self { pool })
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
    async fn find(&self, filter: MetadataFilter) -> Result<Vec<StreamFileMetadata>, StoreError>;
    async fn insert_many(&self, files: &[StreamFileMetadata]) -> Result<(), StoreError>;
    /// Deletes the rows of these files.
    async fn delete_many(&self, filepaths: &[String]) -> Result<(), StoreError>;

    /// Scans `root` (and its subfolders if `recursive`), stores the metadata of every stream
    /// file and removes the rows of files under `root` that no longer exist. Returns the
    /// metadata of all stream files found.
    async fn synchronize_directory(
        &self,
        root: PathBuf,
        recursive: bool,
    ) -> Result<(Vec<StreamFileMetadata>, SyncStats), StoreError> {
        let scan_root = root.clone();
        let scanned = tokio::task::spawn_blocking(move || scan_dir(&scan_root, recursive, false))
            .await
            .map_err(StoreError::Join)??;

        let prefix = root.to_string_lossy().into_owned();
        let known = self
            .find(MetadataFilter::new().and(COLUMNS.filepath.like(&format!("{prefix}%"))))
            .await?;
        let found: HashSet<&str> = scanned.iter().map(|f| f.filepath.as_str()).collect();
        let removed: Vec<String> = known
            .iter()
            // LIKE treats `_` and `%` in the prefix as wildcards: compare real paths
            .filter(|f| Path::new(&f.filepath).starts_with(&root))
            .filter(|f| recursive || Path::new(&f.filepath).parent() == Some(root.as_path()))
            .filter(|f| !found.contains(f.filepath.as_str()))
            .map(|f| f.filepath.clone())
            .collect();
        self.delete_many(&removed).await?;
        self.insert_many(&scanned).await?;

        let stats = SyncStats {
            inserted: scanned.len() as u32,
            updated: 0,
            removed: removed.len() as u32,
            skipped: 0,
        };
        Ok((scanned, stats))
    }
}

//#[cfg(feature = "sqlite")]
#[async_trait]
impl MetadataStore for SqliteStore {
    async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("migrations/sqlite").run(&self.pool).await?;
        Ok(())
    }

    async fn insert_many(&self, files: &[StreamFileMetadata]) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        for f in files {
            // Maybe QueryBuilder is more performant here
            let _rec = sqlx::query(
                r#"
                INSERT OR REPLACE INTO buffer_metadata
                (
                    filepath,
                    header_size,
                    project_id,
                    channel,
                    process,
                    datamode,
                    datatype,
                    datakind,
                    process_datetime,
                    measure_start,
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
                    adc_type,
                    bit_resolution,
                    fft_log_shift,
                    streamno,
                    preamp_gain,
                    file_size,
                    db_count,
                    spec_count
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
            .bind(&f.filepath)
            .bind(f.header_size)
            .bind(f.project_id as i64)
            .bind(f.channel)
            .bind(f.process)
            .bind(f.datamode)
            .bind(f.datatype)
            .bind(f.datakind)
            .bind(f.process_datetime)
            .bind(f.measure_start)
            .bind(f.db_header_size)
            .bind(f.bytes_per_sample)
            .bind(f.db_size)
            .bind(f.frq_bands)
            .bind(f.compression_time)
            .bind(f.compression_frq)
            .bind(f.avg_time)
            .bind(f.avg_frq)
            .bind(f.spec_duration)
            .bind(f.frq_start)
            .bind(f.frq_end)
            .bind(f.frq_per_band)
            .bind(f.adc_type)
            .bind(f.bit_resolution)
            .bind(f.fft_log_shift)
            .bind(f.streamno)
            .bind(f.preamp_gain)
            .bind(f.file_size as i64)
            .bind(f.db_count as i64)
            .bind(f.spec_count as i64)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn delete_many(&self, filepaths: &[String]) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        for path in filepaths {
            sqlx::query("DELETE FROM buffer_metadata WHERE filepath = ?")
                .bind(path)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn find(&self, filter: MetadataFilter) -> Result<Vec<StreamFileMetadata>, StoreError> {
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM buffer_metadata");
        filter.apply(&mut builder);
        builder.push(" ORDER BY process DESC");
        let rows = builder
            .build_query_as::<StreamFileMetadata>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
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

#[cfg(test)]
mod tests {
    use crate::filter::COLUMNS;

    use super::*;
    use chrono::Utc;

    async fn setup() -> SqliteStore {
        let store = SqliteStore::connect("sqlite://:memory:")
            .await
            .expect("Database connection failed");
        store.migrate().await.expect("migration failed");
        store
    }

    async fn insert_test_data(store: &impl MetadataStore) {
        let stream_files: Vec<StreamFileMetadata> = (0..10)
            .map(|i| StreamFileMetadata {
                id: None,
                filepath: format!("foo-{}", i),
                header_size: 2000,
                project_id: 10000000,
                channel: 1,
                process: i,
                datamode: 2,
                datatype: 0,
                datakind: -1,
                process_datetime: Utc::now(),
                measure_start: None,
                db_header_size: 1,
                bytes_per_sample: 2,
                db_size: 1,
                frq_bands: 512,
                compression_time: 1,
                compression_frq: 1,
                avg_time: 1,
                avg_frq: 1,
                spec_duration: 81920.0,
                frq_start: 0.0,
                frq_end: 780800.0,
                frq_per_band: 1525.0,
                adc_type: 1,
                bit_resolution: 16,
                fft_log_shift: 14,
                streamno: None,
                preamp_gain: 1,
                file_size: 0,
                db_count: 1,
                spec_count: 1,
            })
            .collect();
        store
            .insert_many(&stream_files)
            .await
            .expect("Insertion of test files failed");
    }

    #[tokio::test]
    async fn test_sqlite_store() {
        let store = setup().await;
        insert_test_data(&store).await;
        let cases = [
            (MetadataFilter::new().and(COLUMNS.process.eq(4)), 1),
            (MetadataFilter::new().and(COLUMNS.filepath.like("foo%")), 10),
            (MetadataFilter::new().and(COLUMNS.process.gt(4)), 5),
        ];

        for (filter, expected_results) in cases {
            let results = store
                .find(filter)
                .await
                .expect("Fetching filtered metadata failed");
            assert_eq!(results.len(), expected_results);
        }
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    const HEADER: &[u8] = include_bytes!("../../qass-files/testdata/compressed-header.bin");

    /// `root/a.000`, `root/b.000`, `root/sub/c.000` and a file that is no stream file.
    fn folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        for name in ["a.000", "b.000", "sub/c.000"] {
            std::fs::write(dir.path().join(name), HEADER).unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "not a stream file").unwrap();
        dir
    }

    async fn store() -> SqliteStore {
        let store = SqliteStore::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    fn names(files: &[StreamFileMetadata]) -> Vec<String> {
        let mut names: Vec<String> = files
            .iter()
            .map(|f| {
                Path::new(&f.filepath)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn recursive_and_flat_scans() {
        let dir = folder();
        let store = store().await;
        let (files, _) = store
            .synchronize_directory(dir.path().to_owned(), true)
            .await
            .unwrap();
        assert_eq!(names(&files), ["a.000", "b.000", "c.000"]);
        let (files, _) = store
            .synchronize_directory(dir.path().to_owned(), false)
            .await
            .unwrap();
        assert_eq!(names(&files), ["a.000", "b.000"]);
    }

    #[tokio::test]
    async fn synchronizing_again_updates_and_removes() {
        let dir = folder();
        let store = store().await;
        store
            .synchronize_directory(dir.path().to_owned(), true)
            .await
            .unwrap();
        std::fs::remove_file(dir.path().join("b.000")).unwrap();
        // a second sync must not fail on the rows that already exist
        let (_, stats) = store
            .synchronize_directory(dir.path().to_owned(), true)
            .await
            .unwrap();
        assert_eq!(stats.removed, 1);
        let all = store.find(MetadataFilter::new()).await.unwrap();
        assert_eq!(names(&all), ["a.000", "c.000"]);
    }
}
