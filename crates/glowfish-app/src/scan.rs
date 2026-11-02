//! Reads the metadata of a folder in the background.

use std::path::PathBuf;
use std::sync::Mutex;
use std::thread::JoinHandle;

use bevy::prelude::*;
use bevy::window::RequestRedraw;
use stream_metadata_cache::{MetadataStore, SqliteStore, StreamFileMetadata};

use crate::catalog::Catalog;

/// Where folder metadata is cached.
#[derive(Debug, Clone, PartialEq)]
pub enum Database {
    Memory,
    File(PathBuf),
}

impl Database {
    /// `":memory:"` or a file path, as in the configuration.
    pub fn parse(value: &str) -> Self {
        if value == ":memory:" {
            Self::Memory
        } else {
            Self::File(PathBuf::from(value))
        }
    }

    fn url(&self) -> String {
        match self {
            Database::Memory => "sqlite::memory:".into(),
            Database::File(path) => format!("sqlite://{}", path.display()),
        }
    }
}

type ScanResult = Result<Vec<StreamFileMetadata>, String>;

/// A running scan.
#[derive(Resource)]
pub struct Scan {
    pub root: PathBuf,
    started: std::time::Instant,
    handle: Mutex<Option<JoinHandle<ScanResult>>>,
}

/// Why a folder could not be read.
#[derive(Resource, Debug, Clone)]
pub struct ScanFailed(pub String);

impl Scan {
    /// Starts reading `root` (and its subfolders if `recursive`) into `database`.
    pub fn start(root: PathBuf, recursive: bool, database: Database) -> Self {
        let scan_root = root.clone();
        let handle = std::thread::spawn(move || -> ScanResult {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = SqliteStore::connect(&database.url())
                    .await
                    .map_err(|e| e.to_string())?;
                store.migrate().await.map_err(|e| e.to_string())?;
                let (files, _) = store
                    .synchronize_directory(scan_root, recursive)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(files)
            })
        });
        Self {
            root,
            started: std::time::Instant::now(),
            handle: Mutex::new(Some(handle)),
        }
    }

    /// The result once the scan finished.
    fn try_finish(&self) -> Option<ScanResult> {
        let mut handle = self.handle.lock().expect("scan lock poisoned");
        if !handle.as_ref()?.is_finished() {
            return None;
        }
        let result = handle
            .take()?
            .join()
            .unwrap_or_else(|_| Err("reading the folder failed".into()));
        Some(result)
    }
}

/// Turns a finished scan into the [`Catalog`].
pub(crate) fn poll_scan(
    mut commands: Commands,
    scan: Option<Res<Scan>>,
    mut redraw: MessageWriter<RequestRedraw>,
) {
    let Some(scan) = scan else { return };
    let Some(result) = scan.try_finish() else {
        // reactive rendering only wakes on input
        redraw.write(RequestRedraw);
        return;
    };
    commands.remove_resource::<Scan>();
    match result {
        Ok(files) => {
            info!(
                "found {} measurement files in {} in {:.2?}",
                files.len(),
                scan.root.display(),
                scan.started.elapsed()
            );
            commands.insert_resource(Catalog::new(&scan.root, &files));
        }
        Err(error) => {
            error!("cannot read {}: {error}", scan.root.display());
            commands.insert_resource(ScanFailed(error));
        }
    }
    redraw.write(RequestRedraw);
}
