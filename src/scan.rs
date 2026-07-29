use std::path::Path;
use walkdir::WalkDir;

use crate::{error::StoreError, metadata::StreamFileMetadata};
use analyzer_files::stream_file::StreamFile;

pub fn scan_dir(root: &Path, follow_links: bool) -> Result<Vec<StreamFileMetadata>, StoreError> {
    let mut out = vec![];

    for entry in WalkDir::new(root)
        .min_depth(1)
        .follow_links(follow_links)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        let Ok(stream_file) = StreamFile::new_dep(path.to_path_buf()) else {
            continue;
        };
        let metadata = StreamFileMetadata::from(stream_file);
        out.push(metadata);
    }
    Ok(out)
}
