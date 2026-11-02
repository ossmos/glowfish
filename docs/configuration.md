# Configuration

glowfish reads its settings from a configuration file. Options on the
[command line](usage/command-line.md#view-options) override the file for one run.

## The configuration file

| System | Location |
| --- | --- |
| Linux | `~/.config/glowfish/config.toml` |
| Windows | `%APPDATA%\glowfish\config\config.toml` |
| macOS | `~/Library/Application Support/glowfish/config.toml` |

```sh
glowfish config            # shows the location and whether the file exists
glowfish config --init     # creates the file with the defaults below
```

Without a file, the defaults are used. A file only needs the settings you want to change;
everything else keeps its default. `--config <FILE>` uses another file for one run.

glowfish checks the file when it starts: unknown settings (e.g. a typo), invalid values and
contradictions are reported instead of being ignored:

```text
error: invalid configuration /home/me/.config/glowfish/config.toml: TOML parse error at line 2, column 1
  |
2 | mod = "cubes"
  | ^^^
unknown field `mod`, expected one of `mode`, `cells`, `max_cells`, `axes`, `yaw`, `pitch`, `distance`
```

## Defaults

This is the file `glowfish config --init` writes:

```toml
# glowfish configuration. Command line options override these values.

[view]
# "surface" or "cubes"
mode = "surface"
# "max": columns up to the maximum of each cell (keeps peaks)
# "range": boxes from the minimum to the maximum (keeps peaks and gaps)
cells = "max"
# Most cells (cubes or surface vertices) drawn; lower it for older hardware.
# 65536 gives 512 (time) x 128 (frequency) cells.
max_cells = 65536
# Axes with ticks and labels: time and frequency for spectrograms, time and amplitude for
# signals
axes = false
# Initial camera: rotation around the vertical axis and elevation in degrees, distance
yaw = -30.0
pitch = 35.0
distance = 22.0

[labels]
# Every text of the user interface, e.g. for translations. Values are filled into the
# placeholders in braces; their order can change. Units (ms, Hz, kHz) stay as they are.

# Axis titles (view.axes); an empty title is not shown. {unit} is Hz or kHz.
time = "time [ms]"
frequency = "frequency [{unit}]"
amplitude = "amplitude"
# Buttons: display mode and saving the window as PNG
surface = "Surface"
cubes = "Cubes"
save_png = "PNG"
# Shown instead of the axes
hint_spectrogram = "time: right   frequency: depth   amplitude: up"
hint_signal = "time: right   amplitude: up"
# Readout under the cursor; {value} for one value, {max} and {min} for several
readout_time = "t = {time}"
readout_frequency = "f = {frequency}"
readout_value = "amplitude {value}"
readout_range = "max {max}   min {min}"
readout_not_loaded = "not loaded"
# Below the timeline
timeline = "start {start}    end {end}    span {span}    of {length}"
# Title of the file dialog
open_file = "Open a measurement file"
# Folder panel: section titles, the title of the file list, the second line of a file
# ({time} and {frequency} compression, {bands} frequency bands), and status messages
project = "Project"
measurement = "Measurement"
channels = "Channels"
files = "Files on channel {channel}"
file_details = "t ×{time} · f ×{frequency} · {bands} bands"
signal_details = "signal · t ×{time}"
scanning = "Reading {folder} ..."
no_files = "No measurement files in {folder}"
# Font for all texts. The bundled Fira Mono covers Latin, Greek and Cyrillic; set a font file
# (TrueType / OpenType) for other scripts, e.g. "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"
# font = ""

[scaling]
# "linear": 0 to the full scale of the file
# "auto": minimum to maximum of the displayed data
# "fixed": from min to max below
mode = "auto"
# min = 0.0
# max = 65536.0

[controls]
# Mouse buttons: "left", "right" or "middle"
pan = "left"
rotate = "right"

[loading]
# Threads that read and reduce data; 0 picks up to 4. More rarely helps, loading is limited by
# memory and disk bandwidth.
threads = 0
# Memory for cached, reduced data per open file
cache_mb = 512

[export]
# Image size of `glowfish export` in pixels
width = 1920
height = 1080

[metadata]
# Database of the metadata cache used when opening folders: a file path, or ":memory:"
database = ":memory:"
# Include files in subfolders when opening a folder
recursive = true
```

## view

| Key | Default | |
| --- | --- | --- |
| `mode` | `"surface"` | `"surface"` or `"cubes"`, see [Surface and cubes](usage/viewer.md#surface-and-cubes) |
| `cells` | `"max"` | `"max"`: columns from the ground up to the maximum of each cell. `"range"`: boxes from the minimum to the maximum, which also shows gaps |
| `max_cells` | `65536` | most cells drawn, see [below](#older-hardware) |
| `axes` | `false` | axes with ticks and labels, see [Axes](usage/viewer.md#axes) |
| `yaw` | `-30.0` | initial rotation around the vertical axis in degrees; 0 looks along the frequency axis |
| `pitch` | `35.0` | initial elevation in degrees, from about 3 (flat) to 86 (from above) |
| `distance` | `22.0` | initial camera distance, from 3 to 80; the data is 16 units long |

Time signals use `max_cells` too: up to 4096 points along time.

## labels

Every text of the user interface, for example to translate it. Texts with values contain
placeholders in braces, which are replaced by the values; a translation can put them in any
order. A text set to `""` is not shown. Numbers and units (ms, Hz, kHz) are not translated.

| Key | Default | Placeholders | Where |
| --- | --- | --- | --- |
| `time` | `"time [ms]"` | | title of the time [axis](usage/viewer.md#axes); times are always in ms |
| `frequency` | `"frequency [{unit}]"` | `{unit}`: `Hz` or `kHz`, the unit of the ticks | title of the frequency axis |
| `amplitude` | `"amplitude"` | | title of the amplitude axis of signals |
| `surface` | `"Surface"` | | display mode button |
| `cubes` | `"Cubes"` | | display mode button |
| `save_png` | `"PNG"` | | button that saves the window |
| `hint_spectrogram` | `"time: right   frequency: depth   amplitude: up"` | | hint shown without axes |
| `hint_signal` | `"time: right   amplitude: up"` | | hint shown without axes |
| `readout_time` | `"t = {time}"` | `{time}` | [readout](usage/viewer.md#controls) under the cursor |
| `readout_frequency` | `"f = {frequency}"` | `{frequency}` | readout, spectrograms |
| `readout_value` | `"amplitude {value}"` | `{value}` | readout of a single value |
| `readout_range` | `"max {max}   min {min}"` | `{max}`, `{min}` | readout of a cell that combines several values |
| `readout_not_loaded` | `"not loaded"` | | readout where no data is loaded yet |
| `timeline` | `"start {start}    end {end}    span {span}    of {length}"` | `{start}`, `{end}`, `{span}`, `{length}` | below the [timeline](usage/viewer.md#the-timeline) |
| `open_file` | `"Open a measurement file"` | | title of the file dialog |
| `project`, `measurement`, `channels` | `"Project"`, ... | | section titles of the [folder panel](usage/folders.md) |
| `files` | `"Files on channel {channel}"` | `{channel}` | title of the file list |
| `file_details` | `"t ×{time} · f ×{frequency} · {bands} bands"` | `{time}`, `{frequency}`: compression; `{bands}` | second line of a file in the list |
| `signal_details` | `"signal · t ×{time}"` | `{time}` | second line of a time signal in the list |
| `scanning` | `"Reading {folder} ..."` | `{folder}` | while a folder is read |
| `no_files` | `"No measurement files in {folder}"` | `{folder}` | for a folder without measurement files |
| `font` | not set | | font file for all texts, see below |

### Data types

The file list names the data type of each file, e.g. *Raw* or *Maximum*. Rename them in a
`[labels.datatypes]` table, by the code of the data type:

```toml
[labels.datatypes]
RAW = "Rohdaten"
MAXIMUM = "Maximum"
AVERAGE = "Mittelwert"
MOV_AVERAGE_FRQ = "Gleitender Mittelwert (Zeit und Frequenz)"
```

The codes are those of the Analyzer4D software without their `COMP_` prefix: `RAW`,
`DOWNSAMPLE`, `MAXIMUM`, `AVERAGE`, `STD_DEVIATION`, `ENVELOPE`, `MOV_AVERAGE`, `EXTERN_DATA`,
`ANALYZE_OVERVIEW`, `MOV_AVERAGE_OPT`, `COLLECTION`, `IMPORT_SIG`, `SCOPE_RAW`,
`MOV_AVERAGE_FRQ`, `SLOPECHANGE`, `OTHER`, `IO_SIGNAL`, `ENERGY`, `AUDIO_RAW`, `XY_PLOT`,
`SECOND_FFT`, `AUDIO_COMMENT`, `CP_ENERGY_SIG`, `VID_MEASURE`, `VID_SCREEN`, `VID_EXT_LINK`,
`ENVELOPE_UPPER`, `ENVELOPE_LOWER`, `PATTERN_REFOBJ`, `SIGNIFICANCE`, `SIGNIFICANCE_32`,
`PATTERN_MASK`, `GRADIENT_FRQ`, `CSV_EXPORT`.

### Fonts

glowfish ships with the font Fira Mono, which covers Latin, Greek and Cyrillic letters, so
translations into most European languages work without changes. For other scripts, such as
Chinese, Japanese or Arabic, set `font` to a TrueType or OpenType file that contains them:

```toml
[labels]
font = "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"
```

The font is used for every text. Fira Mono is licensed under the SIL Open Font License 1.1.

### Example

Only the texts you change need to be in the file. A German example:

```toml
[labels]
time = "Zeit [ms]"
frequency = "Frequenz [{unit}]"
amplitude = "Amplitude"
surface = "Fläche"
cubes = "Quader"
hint_spectrogram = "Zeit: rechts   Frequenz: Tiefe   Amplitude: oben"
hint_signal = "Zeit: rechts   Amplitude: oben"
readout_time = "t = {time}"
readout_value = "Amplitude {value}"
readout_range = "Max. {max}   Min. {min}"
readout_not_loaded = "nicht geladen"
timeline = "{start} bis {end}    Länge {span}    Datei {length}"
open_file = "Messdatei öffnen"
```

## scaling

Which values fill the height and the colours.

| Key | Default | |
| --- | --- | --- |
| `mode` | `"auto"` | `"auto"`: smallest to largest displayed value. `"linear"`: 0 to the largest value the file can hold. `"fixed"`: from `min` to `max` |
| `min` | | lower value for `"fixed"` |
| `max` | | upper value for `"fixed"`; must be larger than `min` |

Use `"fixed"` to compare files or time windows: the same value always gets the same height and
colour.

```toml
[scaling]
mode = "fixed"
min = 0.0
max = 60000.0
```

## controls

Mouse buttons of the [view controls](usage/viewer.md#controls): `"left"`, `"right"` or
`"middle"`. The two buttons must differ.

| Key | Default | |
| --- | --- | --- |
| `pan` | `"left"` | move along time |
| `rotate` | `"right"` | rotate the view |

For example, rotating with the left button and panning with the middle button, as in many 3D
programs:

```toml
[controls]
pan = "middle"
rotate = "left"
```

The buttons on the [timeline](usage/viewer.md#the-timeline) do not change.

## loading

| Key | Default | |
| --- | --- | --- |
| `threads` | `0` | threads that read and reduce data; `0` uses up to 4 |
| `cache_mb` | `512` | memory in MiB for loaded data of one file |

Loading is limited by the speed of the disk and the memory rather than by the processor, so
more than four threads rarely helps. A larger cache keeps more of a file ready when you move
back to places you looked at before.

## export

| Key | Default | |
| --- | --- | --- |
| `width` | `1920` | width of [exported images](usage/export.md) in pixels |
| `height` | `1080` | height in pixels |

`--size` overrides both.

## metadata

Settings for [opening folders](usage/folders.md).

| Key | Default | |
| --- | --- | --- |
| `database` | `":memory:"` | where the metadata of the files is kept: `":memory:"` while glowfish runs, or a file path to keep it between runs |
| `recursive` | `true` | include files in subfolders; `--recursive` / `--no-recursive` override it |

With a database file, opening the same folder again updates the stored metadata: new files
are added and files that are gone are removed.

## Older hardware

The amount of work for the graphics card is set by `view.max_cells`:

| `max_cells` | Cells (time × frequency) | Cubes: triangles |
| --- | --- | --- |
| 16384 | 256 × 64 | 0.3 million |
| 65536 (default) | 512 × 128 | 1.3 million |
| 262144 | 1024 × 256 | 5.2 million |

Surfaces need about a twentieth of that. On older or integrated graphics, start with a lower
value:

```toml
[view]
max_cells = 16384
```

Fewer cells do not hide peaks: every cell still keeps the minimum and maximum of the data it
covers.
