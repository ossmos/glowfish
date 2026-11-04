use rayon::prelude::*;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

use crate::{error::StoreError, metadata::StreamFileMetadata};
use qass_files::StreamFile;

/// Reads the metadata of all stream files in `root`, and in its subfolders if `recursive`.
/// Files that are not stream files are skipped.
pub fn scan_dir(
    root: &Path,
    recursive: bool,
    follow_links: bool,
) -> Result<Vec<StreamFileMetadata>, StoreError> {
    let files: Vec<DirEntry> = WalkDir::new(root)
        .min_depth(1)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .follow_links(follow_links)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();
    let results: Vec<StreamFileMetadata> = files
        .par_iter()
        .filter_map(|f| StreamFile::open(f.path()).ok())
        .filter_map(|f| StreamFileMetadata::try_from(&f).ok())
        .collect();
    Ok(results)
}
