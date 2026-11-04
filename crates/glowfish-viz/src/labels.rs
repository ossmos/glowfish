use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Every text shown to users, e.g. for translations. Texts with values use named placeholders
/// in braces (`{time}`), so translations can reorder them. Unknown placeholders stay as they are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Labels {
    /// Title of the time axis; an empty title is not shown.
    pub time: String,
    /// Title of the frequency axis; `{unit}` is the unit of the ticks (Hz or kHz).
    pub frequency: String,
    /// Title of the amplitude axis of signals.
    pub amplitude: String,

    /// Display mode buttons.
    pub surface: String,
    pub cubes: String,
    /// Button that saves the window as PNG.
    pub save_png: String,

    /// Hints shown instead of the axes.
    pub hint_spectrogram: String,
    pub hint_signal: String,

    /// Readout under the cursor: `{time}`, `{frequency}`; `{value}` for a single value,
    /// `{max}` and `{min}` for a cell that combines several values.
    pub readout_time: String,
    pub readout_frequency: String,
    pub readout_value: String,
    pub readout_range: String,
    pub readout_not_loaded: String,

    /// Below the timeline: `{start}`, `{end}`, `{span}` and `{length}` of the file.
    pub timeline: String,

    /// In the middle of the view while a file loads: `{progress}` in percent.
    pub loading: String,

    /// Title of the file dialog.
    pub open_file: String,

    /// Folder panel: section titles.
    pub project: String,
    pub measurement: String,
    pub channels: String,
    /// Title of the file list: `{channel}`.
    pub files: String,
    /// Second line of a file in the list: `{time}` and `{frequency}` compression factors,
    /// `{bands}` frequency bands.
    pub file_details: String,
    /// Second line of a time signal in the list: `{time}` compression factor.
    pub signal_details: String,
    /// While a folder is read: `{folder}`.
    pub scanning: String,
    /// When a folder has no measurement files: `{folder}`.
    pub no_files: String,
    /// Names of the data types in the file list, by code (e.g. `RAW`, `MAXIMUM`,
    /// `MOV_AVERAGE_FRQ`); data types without an entry keep their English name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub datatypes: BTreeMap<String, String>,

    /// Font file (TrueType / OpenType) for all texts, for scripts the bundled Fira Mono does not
    /// cover (it covers Latin, Greek and Cyrillic). `None` uses Fira Mono.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<PathBuf>,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            time: "time [ms]".into(),
            frequency: "frequency [{unit}]".into(),
            amplitude: "amplitude".into(),
            surface: "Surface".into(),
            cubes: "Cubes".into(),
            save_png: "PNG".into(),
            hint_spectrogram: "time: right   frequency: depth   amplitude: up".into(),
            hint_signal: "time: right   amplitude: up".into(),
            readout_time: "t = {time}".into(),
            readout_frequency: "f = {frequency}".into(),
            readout_value: "amplitude {value}".into(),
            readout_range: "max {max}   min {min}".into(),
            readout_not_loaded: "not loaded".into(),
            timeline: "start {start}    end {end}    span {span}    of {length}".into(),
            loading: "Loading {progress}".into(),
            open_file: "Open a measurement file".into(),
            project: "Project".into(),
            measurement: "Measurement".into(),
            channels: "Channels".into(),
            files: "Files on channel {channel}".into(),
            file_details: "t \u{d7}{time} \u{b7} f \u{d7}{frequency} \u{b7} {bands} bands".into(),
            signal_details: "signal \u{b7} t \u{d7}{time}".into(),
            scanning: "Reading {folder} ...".into(),
            no_files: "No measurement files in {folder}".into(),
            datatypes: BTreeMap::new(),
            font: None,
        }
    }
}

impl Labels {
    /// The frequency axis title for ticks in `unit`.
    pub fn frequency_in(&self, unit: &str) -> String {
        fill(&self.frequency, &[("unit", unit)])
    }
}

/// Replaces `{name}` placeholders in `template` by their values.
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders() {
        assert_eq!(fill("t = {time}", &[("time", "5 ms")]), "t = 5 ms");
        // reordered, repeated and unknown placeholders
        assert_eq!(
            fill("{b}/{a}/{b} {c}", &[("a", "1"), ("b", "2")]),
            "2/1/2 {c}"
        );
        assert_eq!(fill("no values", &[("a", "1")]), "no values");
    }
}
