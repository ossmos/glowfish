//! Runs the pyramid on the test buffers in the workspace root; skipped when they are missing.

use std::path::PathBuf;

use glowfish_data::{Downsampler, LoadPool, MinMax, Pyramid};
use qass_files::StreamFile;

fn open(name: &str) -> Option<StreamFile> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name);
    if !path.exists() {
        eprintln!("skipping: {} not found", path.display());
        return None;
    }
    Some(StreamFile::open(&path).expect("opening test buffer failed"))
}

#[test]
fn aggregation_overview_matches_brute_force() {
    let Some(file) = open("test-buffer-1") else {
        return;
    };
    let all = file.read_all().unwrap().samples().to_f32_vec();
    let bands = file.header().frq_bands as usize;
    let rows = file.spec_count();
    let pyramid: Pyramid<_> = Pyramid::new(file, 256 << 20, LoadPool::default());

    let grid = pyramid.query(0..rows, 300, 5).unwrap();
    assert!(grid.time_cells <= 301);
    for t in 0..grid.time_cells {
        let rows = grid.cell_rows(t);
        for f in 0..grid.freq_cells {
            let mut expected = MinMax::EMPTY;
            for row in rows.start..rows.end.min(pyramid.rows()) {
                for band in grid.cell_bands(f) {
                    expected =
                        MinMax::combine(expected, MinMax::leaf(all[row as usize * bands + band]));
                }
            }
            assert_eq!(grid.get(t, f).unwrap(), expected, "cell ({t}, {f})");
        }
    }
}

#[test]
fn signal_slice() {
    let Some(file) = open("test-buffer-3-signal.000") else {
        return;
    };
    let pyramid: Pyramid<_> = Pyramid::new(file, 64 << 20, LoadPool::default());
    // the reference values around the block boundary at sample 491520 are 33232 and 33226
    let grid = pyramid.query(491_519..491_521, 10, 1).unwrap();
    assert_eq!(grid.level, 0);
    assert_eq!(grid.get(0, 0).unwrap().max, 33232.0);
    assert_eq!(grid.get(1, 0).unwrap().max, 33226.0);
}
