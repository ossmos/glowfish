use rayon::prelude::*;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

use crate::{error::StoreError, metadata::StreamFileMetadata};
use analyzer_files::StreamFile;

pub fn scan_dir(root: &Path, follow_links: bool) -> Result<Vec<StreamFileMetadata>, StoreError> {
    let files: Vec<DirEntry> = WalkDir::new(root)
        .min_depth(1)
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
