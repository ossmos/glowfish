use std::path::PathBuf;

use bevy::audio::AudioPlugin;
use bevy::prelude::*;
use visualization::VisualizationPlugin;

use crate::options::{ViewOptions, open_dataset};

/// Opens the viewer for `path`, or for a file picked in a dialog.
pub fn run(path: Option<PathBuf>, options: &ViewOptions) -> anyhow::Result<()> {
    let config = options.config()?;
    let path = match path {
        Some(path) => path,
        None => match rfd::FileDialog::new()
            .set_title("Open a measurement file")
            .pick_file()
        {
            Some(path) => path,
            None => return Ok(()),
        },
    };
    let dataset = open_dataset(&path, &config)?;
    let selection = options.selection(&dataset)?;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("glowfish – {}", dataset.name),
                    ..default()
                }),
                ..default()
            })
            .disable::<AudioPlugin>(),
    )
    .insert_resource(config.visualization()?)
    .add_plugins(VisualizationPlugin)
    .insert_resource(dataset);
    if let Some(selection) = selection {
        app.insert_resource(selection);
    }
    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(code) => anyhow::bail!("the viewer exited with code {code}"),
    }
}
