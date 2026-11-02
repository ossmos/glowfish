# Command line reference

```text
glowfish [OPTIONS] [PATH]                       open PATH in the viewer
glowfish export <FILE> -o <PNG> [OPTIONS]       render FILE to a PNG
glowfish config [--default | --init]            show, print or create the configuration
```

`glowfish --help` and `glowfish <command> --help` list the options as well.

## Opening a file

```sh
glowfish [OPTIONS] [PATH]
```

`PATH` is a measurement file. Without it, a file dialog opens. Folders cannot be opened yet;
that comes with the metadata selection.

## View options

These options work for the viewer and for `export`. They override the
[configuration file](../configuration.md).

| Option | Values | Configuration key |
| --- | --- | --- |
| `--config <FILE>` | configuration file to use instead of the default location | |
| `--mode <MODE>` | `surface`, `cubes` | `view.mode` |
| `--cells <CELLS>` | `max`, `range` | `view.cells` |
| `--max-cells <N>` | most cells drawn | `view.max_cells` |
| `--axes`, `--no-axes` | show or hide the axes; the last one given wins | `view.axes` |
| `--scaling <MODE>` | `linear`, `auto`, `fixed` | `scaling.mode` |
| `--min <VALUE>` | lower value of fixed scaling | `scaling.min` |
| `--max <VALUE>` | upper value of fixed scaling | `scaling.max` |
| `--start <MS>` | start of the time window in ms (default: start of the file) | |
| `--end <MS>` | end of the time window in ms (default: end of the file) | |
| `--yaw <DEGREES>` | camera rotation around the vertical axis | `view.yaw` |
| `--pitch <DEGREES>` | camera elevation | `view.pitch` |
| `--distance <D>` | camera distance | `view.distance` |
| `--threads <N>` | threads that load data, 0 for automatic | `loading.threads` |
| `--cache-mb <MIB>` | memory for loaded data | `loading.cache_mb` |

`--min` or `--max` without `--scaling` switch to fixed scaling; fixed scaling needs both.

Times are given in milliseconds from the start of the file, as shown in the viewer's timeline.
A window that reaches past the end of the file ends at the end of the file.

## export

```sh
glowfish export <FILE> -o <PNG> [--size WIDTHxHEIGHT] [VIEW OPTIONS]
```

See [Exporting images](export.md).

## config

```sh
glowfish config            # where the configuration file is and whether it exists
glowfish config --default  # print the default configuration with comments
glowfish config --init     # create the configuration file with the defaults
```

`--init` does not overwrite an existing file.

## Exit status

glowfish exits with status 0 on success and 1 on errors, e.g. a file that cannot be read, an
invalid configuration or an export that could not be written. The reason is printed to the
error output:

```text
error: --start 500000 ms is after the end of the file (120000.3 ms)
```
