//! Times pyramid queries on a stream file:
//! `cargo run --release -p glowfish-data --example overview -- <file> [time_cells] [freq_cells]`

use std::time::Instant;

use glowfish_data::{LoadPool, Pyramid};
use qass_files::StreamFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: overview <file> [time_cells] [freq_cells]")?;
    let time_cells: usize = args.next().map_or(Ok(512), |a| a.parse())?;
    let freq_cells: usize = args.next().map_or(Ok(128), |a| a.parse())?;

    let file = StreamFile::open(&path)?;
    let rows = file.spec_count();
    println!("{path}: {rows} rows × {} bands", file.header().frq_bands);
    let pyramid: Pyramid<_> = Pyramid::new(file, 512 << 20, LoadPool::default());

    let time =
        |label: &str, range: std::ops::Range<u64>| -> Result<(), Box<dyn std::error::Error>> {
            let start = Instant::now();
            let grid = pyramid.query(range.clone(), time_cells, freq_cells)?;
            println!(
                "{label:>14}: {range:?} -> {}×{} cells at level {} in {:.3?} (cache {} MiB)",
                grid.time_cells,
                grid.freq_cells,
                grid.level,
                start.elapsed(),
                pyramid.cached_bytes() >> 20
            );
            Ok(())
        };
    time("slice", rows / 2..rows / 2 + rows / 100)?;
    time("slice again", rows / 2..rows / 2 + rows / 100)?;
    time("whole file", 0..rows)?;
    time("whole again", 0..rows)?;
    time("zoom in", rows / 4..rows / 4 + rows / 50)?;
    Ok(())
}
