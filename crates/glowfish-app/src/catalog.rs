//! The files of a folder, grouped as project -> measurement (process number) -> channel -> file,
//! and which of them is selected.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use analyzer_files::{Datamode, Datatype};
use bevy::prelude::*;
use chrono::{DateTime, Utc};
use stream_metadata_cache::StreamFileMetadata;

/// One measurement file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub channel: u32,
    pub signal: bool,
    pub datatype: Option<Datatype>,
    pub compression_time: u32,
    pub compression_frq: u32,
    pub frq_bands: u32,
}

/// All files with the same process number.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    pub process: u32,
    pub time: DateTime<Utc>,
    pub files: Vec<FileEntry>,
}

impl Measurement {
    /// Files on `channel`: raw data first, then by data type and compression.
    pub fn files_on(&self, channel: u32) -> Vec<&FileEntry> {
        let mut files: Vec<&FileEntry> =
            self.files.iter().filter(|f| f.channel == channel).collect();
        files.sort_by_key(|f| {
            (
                f.datatype != Some(Datatype::Raw),
                f.datatype.map(|d| d.as_i32()),
                f.signal,
                f.compression_time,
                f.compression_frq,
                f.path.clone(),
            )
        });
        files
    }

    pub fn count_on(&self, channel: u32) -> usize {
        self.files.iter().filter(|f| f.channel == channel).count()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub id: u64,
    /// Sorted by process number.
    pub measurements: Vec<Measurement>,
}

impl Project {
    /// Every channel used by any measurement of the project, sorted.
    pub fn channels(&self) -> Vec<u32> {
        let mut channels: Vec<u32> = self
            .measurements
            .iter()
            .flat_map(|m| m.files.iter().map(|f| f.channel))
            .collect();
        channels.sort_unstable();
        channels.dedup();
        channels
    }
}

/// The measurement files of a folder and the current selection.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Catalog {
    pub root: PathBuf,
    /// Sorted by id.
    pub projects: Vec<Project>,
    pub project: usize,
    pub measurement: usize,
    pub channel: Option<u32>,
    pub file: Option<PathBuf>,
}

/// A change of the selection, e.g. from a button of the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum Select {
    Project(isize),
    Measurement(isize),
    Channel(u32),
    File(PathBuf),
}

impl Catalog {
    /// Groups `files` found in `root`. Selects the first project, its latest measurement and
    /// the first file of its first channel.
    pub fn new(root: &Path, files: &[StreamFileMetadata]) -> Self {
        let mut projects: BTreeMap<u64, BTreeMap<u32, Measurement>> = BTreeMap::new();
        for f in files {
            let measurement = projects
                .entry(f.project_id)
                .or_default()
                .entry(f.process)
                .or_insert_with(|| Measurement {
                    process: f.process,
                    time: f.measure_start.unwrap_or(f.process_datetime),
                    files: Vec::new(),
                });
            measurement.files.push(FileEntry {
                path: PathBuf::from(&f.filepath),
                channel: f.channel,
                signal: f.datamode == Datamode::Signal.as_i32(),
                datatype: Datatype::try_from(f.datatype).ok(),
                compression_time: f.compression_time,
                compression_frq: f.compression_frq,
                frq_bands: f.frq_bands,
            });
        }
        let projects = projects
            .into_iter()
            .map(|(id, measurements)| Project {
                id,
                measurements: measurements.into_values().collect(),
            })
            .collect();
        let mut catalog = Self {
            root: root.to_owned(),
            projects,
            project: 0,
            measurement: 0,
            channel: None,
            file: None,
        };
        catalog.select_measurement(usize::MAX);
        catalog
    }

    pub fn current_project(&self) -> Option<&Project> {
        self.projects.get(self.project)
    }

    pub fn current_measurement(&self) -> Option<&Measurement> {
        self.current_project()?.measurements.get(self.measurement)
    }

    /// Applies `select`; returns whether the selection changed.
    pub fn apply(&mut self, select: Select) -> bool {
        let before = (
            self.project,
            self.measurement,
            self.channel,
            self.file.clone(),
        );
        match select {
            Select::Project(step) => {
                let project = step_index(self.project, step, self.projects.len());
                if project != self.project {
                    self.project = project;
                    self.select_measurement(usize::MAX);
                }
            }
            Select::Measurement(step) => {
                let count = self.current_project().map_or(0, |p| p.measurements.len());
                let measurement = step_index(self.measurement, step, count);
                if measurement != self.measurement {
                    self.select_measurement(measurement);
                }
            }
            Select::Channel(channel) => {
                if self
                    .current_measurement()
                    .is_some_and(|m| m.count_on(channel) > 0)
                {
                    self.select_channel(channel);
                }
            }
            Select::File(path) => {
                let known = self
                    .current_measurement()
                    .is_some_and(|m| m.files.iter().any(|f| f.path == path));
                if known {
                    self.file = Some(path);
                }
            }
        }
        before
            != (
                self.project,
                self.measurement,
                self.channel,
                self.file.clone(),
            )
    }

    /// Selects measurement `index` (clamped, `usize::MAX` for the latest), keeping the channel
    /// if the measurement has files on it.
    fn select_measurement(&mut self, index: usize) {
        let Some(project) = self.current_project() else {
            return;
        };
        self.measurement = index.min(project.measurements.len().saturating_sub(1));
        let Some(measurement) = self.current_measurement() else {
            return;
        };
        let channel = self
            .channel
            .filter(|&c| measurement.count_on(c) > 0)
            .or_else(|| measurement.files.iter().map(|f| f.channel).min());
        if let Some(channel) = channel {
            self.select_channel(channel);
        }
    }

    /// Selects `channel` and on it the file most like the current one: the same data type and
    /// compression if there is one, the first file otherwise.
    fn select_channel(&mut self, channel: u32) {
        let Some(measurement) = self.current_measurement() else {
            return;
        };
        let current = self
            .projects
            .iter()
            .flat_map(|p| &p.measurements)
            .flat_map(|m| &m.files)
            .find(|f| Some(&f.path) == self.file.as_ref())
            .map(|f| (f.signal, f.datatype, f.compression_time, f.compression_frq));
        let files = measurement.files_on(channel);
        let file = files
            .iter()
            .find(|f| {
                Some((f.signal, f.datatype, f.compression_time, f.compression_frq)) == current
            })
            .or(files.first())
            .map(|f| f.path.clone());
        self.channel = Some(channel);
        self.file = file;
    }
}

fn step_index(index: usize, step: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    index.saturating_add_signed(step).min(count - 1)
}

/// The name shown for a data type: from `labels.datatypes` by its code, else the English name.
pub fn display_name(labels: &visualization::Labels, datatype: Option<Datatype>) -> String {
    datatype
        .and_then(|d| labels.datatypes.get(&d.to_string()))
        .cloned()
        .unwrap_or_else(|| datatype_name(datatype).to_owned())
}

/// The readable English name of a data type; `labels.datatypes` can replace it.
pub fn datatype_name(datatype: Option<Datatype>) -> &'static str {
    use Datatype::*;
    match datatype {
        None | Some(Invalid) | Some(Undef) | Some(Count) => "Unknown",
        Some(Raw) => "Raw",
        Some(Downsample) => "Downsampled",
        Some(Maximum) => "Maximum",
        Some(Average) => "Average",
        Some(StdDeviation) => "Standard deviation",
        Some(Envelope) => "Envelope",
        Some(MovingAverage) => "Moving average",
        Some(ExternalData) => "External data",
        Some(AnalyzeOverview) => "Analyze overview",
        Some(MovingAverageOpt) => "Moving average (optimized)",
        Some(Collection) => "Collection",
        Some(ImportSig) => "Imported signal",
        Some(ScopeRaw) => "Oscilloscope raw",
        Some(MovingAverageFrq) => "Moving average (time and frequency)",
        Some(SlopeChange) => "Slope change",
        Some(Other) => "Other",
        Some(IoSignal) => "IO signal",
        Some(Energy) => "Energy",
        Some(AudioRaw) => "Audio raw",
        Some(XyPlot) => "XY plot",
        Some(SecondFft) => "Second FFT",
        Some(AudioComment) => "Audio comment",
        Some(CpEnergySig) => "Energy signature",
        Some(VideoMeasure) => "Video",
        Some(VideoScreen) => "Screen video",
        Some(VideoExtLink) => "Linked video",
        Some(EnvelopeUpper) => "Upper envelope",
        Some(EnvelopeLower) => "Lower envelope",
        Some(PatternRefObj) => "Pattern reference",
        Some(Significance) => "Significance",
        Some(Significance32) => "Significance (32 bit)",
        Some(PatternMask) => "Pattern mask",
        Some(GradientFrq) => "Frequency gradient",
        Some(CsvExport) => "CSV export",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn metadata(
        project: u64,
        process: u32,
        channel: u32,
        datatype: Datatype,
        compression: u32,
    ) -> StreamFileMetadata {
        StreamFileMetadata {
            id: None,
            filepath: format!(
                "/data/p{process}c{channel}-{}-{compression}",
                datatype.as_i32()
            ),
            header_size: 2000,
            project_id: project,
            channel,
            process,
            datamode: Datamode::Fft.as_i32(),
            datatype: datatype.as_i32(),
            datakind: -1,
            process_datetime: DateTime::from_timestamp(1_700_000_000 + process as i64, 0).unwrap(),
            measure_start: None,
            db_header_size: 0,
            bytes_per_sample: 2,
            db_size: 1024,
            frq_bands: 512 / compression,
            compression_time: compression,
            compression_frq: compression,
            avg_time: 1,
            avg_frq: 1,
            spec_duration: 1.0,
            frq_start: 0.0,
            frq_end: 1.0,
            frq_per_band: 1.0,
            adc_type: 1,
            bit_resolution: 16,
            fft_log_shift: 0,
            streamno: None,
            preamp_gain: 0,
            file_size: 0,
            db_count: 0,
            spec_count: 0,
        }
    }

    fn catalog() -> Catalog {
        let files = vec![
            metadata(7, 10, 1, Datatype::Maximum, 8),
            metadata(7, 10, 1, Datatype::Raw, 1),
            metadata(7, 10, 2, Datatype::Raw, 1),
            metadata(7, 11, 2, Datatype::Raw, 1),
            metadata(7, 11, 2, Datatype::Maximum, 8),
            metadata(3, 5, 0, Datatype::Raw, 1),
        ];
        Catalog::new(Path::new("/data"), &files)
    }

    fn selected(c: &Catalog) -> (u64, u32, Option<u32>, Option<String>) {
        (
            c.current_project().unwrap().id,
            c.current_measurement().unwrap().process,
            c.channel,
            c.file.as_ref().map(|p| p.display().to_string()),
        )
    }

    #[test]
    fn groups_and_selects_the_latest_measurement() {
        let c = catalog();
        assert_eq!(c.projects.iter().map(|p| p.id).collect::<Vec<_>>(), [3, 7]);
        assert_eq!(selected(&c), (3, 5, Some(0), Some("/data/p5c0-0-1".into())));
        let mut c = c;
        assert!(c.apply(Select::Project(1)));
        // latest measurement of project 7 is 11; it has files on channel 2 only
        assert_eq!(
            selected(&c),
            (7, 11, Some(2), Some("/data/p11c2-0-1".into()))
        );
        assert_eq!(c.current_project().unwrap().channels(), [1, 2]);
    }

    #[test]
    fn files_are_sorted_raw_first() {
        let c = catalog();
        let m = &c.projects[1].measurements[0];
        let files: Vec<_> = m.files_on(1).iter().map(|f| f.datatype).collect();
        assert_eq!(files, [Some(Datatype::Raw), Some(Datatype::Maximum)]);
        assert_eq!(m.count_on(1), 2);
        assert_eq!(m.count_on(3), 0);
    }

    #[test]
    fn switching_keeps_the_kind_of_file() {
        let mut c = catalog();
        c.apply(Select::Project(1));
        c.apply(Select::File("/data/p11c2-2-8".into()));
        // measurement 10 has a maximum file on channel 2? no: only raw, so the first file
        assert!(c.apply(Select::Measurement(-1)));
        assert_eq!(
            selected(&c),
            (7, 10, Some(2), Some("/data/p10c2-0-1".into()))
        );
        // channel 1 has the same kind as before (raw) first
        c.apply(Select::File("/data/p10c2-0-1".into()));
        c.apply(Select::Channel(1));
        assert_eq!(c.file, Some("/data/p10c1-0-1".into()));
        // back to 11: channel 1 has no files there, the channel falls back to 2
        c.apply(Select::Measurement(1));
        assert_eq!(selected(&c).2, Some(2));
    }

    #[test]
    fn invalid_selections_change_nothing() {
        let mut c = catalog();
        assert!(!c.apply(Select::Project(-1)));
        assert!(!c.apply(Select::Measurement(5)));
        assert!(!c.apply(Select::Channel(9)));
        assert!(!c.apply(Select::File("/elsewhere".into())));
    }

    #[test]
    fn data_type_names_can_be_replaced() {
        let mut labels = visualization::Labels::default();
        assert_eq!(display_name(&labels, Some(Datatype::Raw)), "Raw");
        labels.datatypes.insert("RAW".into(), "Rohdaten".into());
        assert_eq!(display_name(&labels, Some(Datatype::Raw)), "Rohdaten");
        assert_eq!(display_name(&labels, Some(Datatype::Maximum)), "Maximum");
        assert_eq!(display_name(&labels, None), "Unknown");
    }

    #[test]
    fn empty_folders() {
        let c = Catalog::new(Path::new("/empty"), &[]);
        assert!(c.projects.is_empty());
        assert!(c.current_measurement().is_none());
        assert_eq!(c.file, None);
    }
}
