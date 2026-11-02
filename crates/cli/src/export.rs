//! Renders a file to a PNG offscreen: no window, the camera renders into an image.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::bail;
use bevy::app::ScheduleRunnerPlugin;
use bevy::audio::AudioPlugin;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use visualization::{
    Dataset, DisplayedGrid, Layout, RenderToImage, Selection, VisualizationPlugin,
};

use crate::options::{ViewOptions, open_dataset};

/// Frames and seconds rendered after the data is loaded before capturing: render pipelines
/// compile in the background and are not drawn until they are ready.
const SETTLE_FRAMES: u32 = 10;
const SETTLE_SECS: f32 = 1.0;
/// Give up if loading and rendering take longer than this.
const TIMEOUT_SECS: f32 = 300.0;

pub fn run(
    file: &Path,
    output: PathBuf,
    size: Option<(u32, u32)>,
    options: &ViewOptions,
) -> anyhow::Result<()> {
    if !output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        bail!("the output file must end with .png");
    }
    if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty())
        && !dir.is_dir()
    {
        bail!("the folder {} does not exist", dir.display());
    }
    let config = options.config()?;
    let (width, height) = size.unwrap_or((config.export.width, config.export.height));
    let dataset = open_dataset(file, &config)?;
    let selection = options.selection(&dataset)?;
    let visualization = visualization::VisualizationConfig {
        layout: Layout::Report,
        ..config.visualization()?
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<AudioPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        1.0 / 60.0,
    )))
    .insert_resource(visualization)
    // labels keep their size relative to the image (1.0 at 1080 pixels height)
    .insert_resource(UiScale(height as f32 / 1080.0))
    .add_plugins(VisualizationPlugin)
    .insert_resource(Export {
        output: output.clone(),
        width,
        height,
        dataset: Some(dataset),
        selection,
        image: None,
        frames: 0,
        settled: 0.0,
        elapsed: 0.0,
        requested: false,
        failed: false,
    })
    .add_systems(Startup, setup)
    .add_systems(Last, capture);
    let started = std::time::SystemTime::now();
    match app.run() {
        // saving only logs its errors; check that the file was written by this run
        AppExit::Success if written_since(&output, started) => {
            println!("wrote {}", output.display());
            Ok(())
        }
        AppExit::Success => bail!("could not write {}", output.display()),
        AppExit::Error(_) => bail!("export timed out after {TIMEOUT_SECS} s"),
    }
}

#[derive(Resource)]
struct Export {
    output: PathBuf,
    width: u32,
    height: u32,
    dataset: Option<Dataset>,
    selection: Option<Selection>,
    image: Option<Handle<Image>>,
    frames: u32,
    settled: f32,
    elapsed: f32,
    requested: bool,
    failed: bool,
}

/// Creates the target image, then hands the dataset to the visualization.
fn setup(mut commands: Commands, mut export: ResMut<Export>, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(
        export.width,
        export.height,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    // the screenshot copies from the target
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = images.add(image);
    commands.insert_resource(RenderToImage(image.clone()));
    export.image = Some(image);
    if let Some(selection) = export.selection {
        commands.insert_resource(selection);
    }
    if let Some(dataset) = export.dataset.take() {
        commands.insert_resource(dataset);
    }
}

fn capture(
    mut commands: Commands,
    time: Res<Time>,
    mut export: ResMut<Export>,
    displayed: Option<Res<DisplayedGrid>>,
    mut exit: MessageWriter<AppExit>,
) {
    export.elapsed += time.delta_secs();
    if export.elapsed > TIMEOUT_SECS && !export.failed {
        export.failed = true;
        exit.write(AppExit::error());
        return;
    }
    if export.requested || !displayed.is_some_and(|d| d.complete) {
        return;
    }
    export.frames += 1;
    export.settled += time.delta_secs();
    if export.frames < SETTLE_FRAMES || export.settled < SETTLE_SECS {
        return;
    }
    let Some(image) = export.image.clone() else {
        return;
    };
    export.requested = true;
    commands
        .spawn(Screenshot::image(image))
        .observe(save_to_disk(export.output.clone()))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}

fn written_since(path: &Path, since: std::time::SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|modified| modified >= since)
}

/// Parses `WIDTHxHEIGHT`.
pub fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("expected WIDTHxHEIGHT, got {text:?}"))?;
    let parse = |v: &str| {
        v.trim()
            .parse::<u32>()
            .ok()
            .filter(|&v| (1..=16384).contains(&v))
            .ok_or_else(|| format!("{v:?} is not a size between 1 and 16384"))
    };
    Ok((parse(w)?, parse(h)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("1920x1080"), Ok((1920, 1080)));
        assert_eq!(parse_size("640X480"), Ok((640, 480)));
        assert!(parse_size("1920").is_err());
        assert!(parse_size("0x10").is_err());
        assert!(parse_size("axb").is_err());
    }
}
