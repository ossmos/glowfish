use std::path::PathBuf;

use bevy::audio::AudioPlugin;
use bevy::prelude::*;
use glowfish_app::{Database, FolderPlugin};
use visualization::VisualizationPlugin;

use crate::config::Config;
use crate::options::{ViewOptions, open_dataset};

/// Opens the viewer for `path`, a file or a folder, or for a file picked in a dialog.
/// `recursive` overrides `metadata.recursive` for folders.
pub fn run(
    path: Option<PathBuf>,
    recursive: Option<bool>,
    options: &ViewOptions,
) -> anyhow::Result<()> {
    let mut config = options.config()?;
    if let Some(recursive) = recursive {
        config.metadata.recursive = recursive;
    }
    let path = match path {
        Some(path) => path,
        None => match rfd::FileDialog::new()
            .set_title(&config.labels.open_file)
            .pick_file()
        {
            Some(path) => path,
            None => return Ok(()),
        },
    };
    if path.is_dir() {
        return run_folder(path, &config);
    }
    let dataset = open_dataset(&path, &config)?;
    let selection = options.selection(&dataset)?;

    let mut app = App::new();
    app.add_plugins(default_plugins(format!("glowfish – {}", dataset.name)))
        .insert_resource(config.visualization()?)
        .add_plugins(VisualizationPlugin)
        .insert_resource(dataset);
    if let Some(selection) = selection {
        app.insert_resource(selection);
    }
    exit(app.run())
}

/// The folder panel next to the view; files are opened from the panel.
fn run_folder(root: PathBuf, config: &Config) -> anyhow::Result<()> {
    let mut app = App::new();
    app.add_plugins(default_plugins(format!("glowfish – {}", root.display())))
        .insert_resource(config.visualization()?)
        .add_plugins(VisualizationPlugin)
        .add_plugins(FolderPlugin {
            root,
            recursive: config.metadata.recursive,
            database: Database::parse(&config.metadata.database),
            cache_bytes: config.loading.cache_mb << 20,
            threads: config.loading.threads,
        });
    exit(app.run())
}

fn default_plugins(title: String) -> impl PluginGroup {
    DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window { title, ..default() }),
            ..default()
        })
        .disable::<AudioPlugin>()
}

fn exit(code: AppExit) -> anyhow::Result<()> {
    match code {
        AppExit::Success => Ok(()),
        AppExit::Error(code) => anyhow::bail!("the viewer exited with code {code}"),
    }
}
