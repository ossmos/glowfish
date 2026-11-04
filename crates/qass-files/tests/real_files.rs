//! Compares against values produced by the reference implementation `buffer_parser.py` for the
//! test buffers in the workspace root. The files are large and not committed, so tests skip
//! when a file is missing.

use std::path::PathBuf;

use qass_files::{AdcType, Datamode, Datatype, SampleType, Samples, StreamData, StreamFile};

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

fn values(data: &StreamData) -> Vec<f64> {
    let samples = data.samples();
    (0..samples.len())
        .map(|i| samples.get(i).unwrap())
        .collect()
}

fn sum(file: &StreamFile, specs: u64) -> f64 {
    values(&file.read(0..specs).unwrap()).iter().sum()
}

#[test]
fn aggregation_buffer() {
    let Some(file) = open("test-buffer-1") else {
        return;
    };
    let h = file.header();
    assert_eq!(h.datamode, Datamode::Fft);
    assert_eq!(h.datatype, Datatype::MovingAverageFrq);
    assert_eq!(h.sample_type, SampleType::U32);
    assert_eq!(h.adc_type, AdcType::Adc24Bit);
    assert_eq!(h.frq_bands, 16);
    assert_eq!(h.compression_frq, 32);
    assert_eq!(h.compression_time, 4);
    assert_eq!(h.spec_duration, 256_000.0);
    assert_eq!(h.frq_per_band, 124_992.0);
    assert_eq!(h.process, 386_297);
    assert_eq!(h.project_id, 14_510_982_826);
    assert_eq!(h.process_time.timestamp(), 1_752_872_555);
    assert_eq!(
        h.measure_start.map(|t| t.timestamp_millis()),
        Some(1_752_872_555_747)
    );
    assert_eq!(file.db_count(), 31);
    assert_eq!(file.spec_count(), 468_751);

    let first = values(&file.read(0..1).unwrap());
    assert_eq!(first[..3], [3_390_949.0, 3_555_304.0, 3_172_917.0]);

    // the last spectrum of block 0 and the first of block 1
    let boundary = h.db_spec_count();
    let cross = values(&file.read(boundary - 1..boundary + 1).unwrap());
    assert_eq!(cross[..3], [1_892_391.0, 2_188_582.0, 1_856_563.0]);
    assert_eq!(cross[16..19], [1_204_136.0, 1_397_238.0, 1_294_143.0]);

    let last = values(&file.read(file.spec_count() - 1..file.spec_count()).unwrap());
    assert_eq!(last[..3], [3_222_532.0, 3_357_953.0, 3_011_772.0]);
    assert_eq!(last[15], 7_900.0);

    assert_eq!(sum(&file, 2000), 41_487_695_064.0);
}

#[test]
fn raw_buffer() {
    let Some(file) = open("test-buffer-2-raw.000") else {
        return;
    };
    let h = file.header();
    assert_eq!(h.datamode, Datamode::Fft);
    assert_eq!(h.datatype, Datatype::Raw);
    assert_eq!(h.sample_type, SampleType::U16);
    assert_eq!(h.frq_bands, 512);
    assert_eq!(h.fft_log_shift, 14);
    assert_eq!(h.spec_duration, 81_920.0);
    assert_eq!(file.db_count(), 6746);
    assert_eq!(file.spec_count(), 6_475_950);

    let StreamData::Spectrogram(first) = file.read(0..4).unwrap() else {
        panic!("expected a spectrogram");
    };
    assert_eq!(first.shape(), (4, 512));
    let Samples::U16(samples) = first.samples() else {
        panic!("expected u16 samples");
    };
    assert_eq!(samples[..4], [189, 336, 702, 282]);

    let boundary = h.db_spec_count();
    let cross = values(&file.read(boundary - 1..boundary + 1).unwrap());
    assert_eq!(cross[..3], [2.0, 214.0, 453.0]);
    assert_eq!(cross[512..515], [221.0, 228.0, 534.0]);

    let last = values(&file.read(file.spec_count() - 1..file.spec_count()).unwrap());
    assert_eq!(last[..3], [96.0, 144.0, 518.0]);
    assert_eq!(last[511], 117.0);

    assert_eq!(sum(&file, 2000), 935_803_980.0);
}

#[test]
fn signal_buffer() {
    let Some(file) = open("test-buffer-3-signal.000") else {
        return;
    };
    let h = file.header();
    assert_eq!(h.datamode, Datamode::Signal);
    assert_eq!(h.datatype, Datatype::Raw);
    assert_eq!(h.frq_bands, 1);
    assert_eq!(h.zero_amplitude_level, Some(32768));
    assert_eq!(h.spec_duration, 10_240.026_214_467_109);
    assert_eq!(h.channel, 3);
    assert_eq!(h.measure_start, None);
    assert_eq!(file.db_count(), 2);
    assert_eq!(file.spec_count(), 898_560);

    let StreamData::Signal(signal) = file.read(0..4).unwrap() else {
        panic!("expected a signal");
    };
    assert_eq!(
        signal.samples(),
        &Samples::U16(vec![32927, 32927, 32928, 32928])
    );
    assert_eq!(signal.zero_level(), Some(32768.0));

    let boundary = h.db_spec_count();
    let cross = values(&file.read(boundary - 1..boundary + 1).unwrap());
    assert_eq!(cross, [33232.0, 33226.0]);

    let last = values(&file.read(file.spec_count() - 1..file.spec_count()).unwrap());
    assert_eq!(last, [32933.0]);

    assert_eq!(sum(&file, 2000), 65_859_938.0);
}
