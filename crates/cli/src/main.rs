//! `glowfish`: open measurement files in the viewer or export them as images.

mod config;
mod export;
mod options;
mod viewer;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};

use crate::options::ViewOptions;

/// Visualize Analyzer4D measurement files.
///
/// Without a command, opens PATH in the viewer (or a file dialog if PATH is missing).
#[derive(Debug, Parser)]
#[command(name = "glowfish", version, args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Measurement file to open
    path: Option<PathBuf>,

    #[command(flatten)]
    options: ViewOptions,
}

// parsed once, the size difference between the variants does not matter
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
enum Command {
    /// Render a file or a time window of it to a PNG, without opening a window
    Export {
        /// Measurement file
        file: PathBuf,
        /// PNG file to write
        #[arg(short, long)]
        output: PathBuf,
        /// Image size, e.g. 3840x2160 (default: from the configuration, 1920x1080)
        #[arg(long, value_parser = export::parse_size, value_name = "WIDTHxHEIGHT")]
        size: Option<(u32, u32)>,
        #[command(flatten)]
        options: ViewOptions,
    },
    /// Show where the configuration file is, print or create it
    Config {
        /// Print the default configuration with comments
        #[arg(long, conflicts_with = "init")]
        default: bool,
        /// Write the default configuration to the configuration file if it does not exist
        #[arg(long)]
        init: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        None => viewer::run(cli.path, &cli.options),
        Some(Command::Export {
            file,
            output,
            size,
            options,
        }) => export::run(&file, output, size, &options),
        Some(Command::Config { default, init }) => config_command(default, init),
    }
}

fn config_command(default: bool, init: bool) -> anyhow::Result<()> {
    if default {
        print!("{}", config::DEFAULT_CONFIG);
        return Ok(());
    }
    let path = config::default_path().context("no configuration directory on this system")?;
    if init {
        if path.exists() {
            bail!("{} already exists", path.display());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        std::fs::write(&path, config::DEFAULT_CONFIG)
            .with_context(|| format!("cannot write {}", path.display()))?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    let state = if path.exists() {
        "exists"
    } else {
        "does not exist, defaults are used; create it with `glowfish config --init`"
    };
    println!("{} ({state})", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn command_line_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_viewer_and_export() {
        let cli = Cli::try_parse_from(["glowfish", "file.000", "--mode", "cubes", "--yaw", "-45"])
            .unwrap();
        assert_eq!(cli.path, Some(PathBuf::from("file.000")));
        assert_eq!(cli.options.yaw, Some(-45.0));

        let cli = Cli::try_parse_from([
            "glowfish", "export", "f", "-o", "a.png", "--size", "800x600", "--start", "10",
        ])
        .unwrap();
        let Some(Command::Export { size, options, .. }) = cli.command else {
            panic!("expected export");
        };
        assert_eq!(size, Some((800, 600)));
        assert_eq!(options.start, Some(10.0));
    }
}
