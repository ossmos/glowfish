//! Opens a stream file in the visualization:
//! `cargo run --release -p glowfish-viz --example view -- <file> [options]`
//!
//! Options:
//!   --cubes                  show cubes instead of a surface
//!   --range                  draw min..max boxes instead of columns up to the maximum
//!   --auto                   scale to the displayed data instead of the file's full scale
//!   --rows <start>..<end>    initial selection in rows
//!   --screenshot <png>       save a screenshot once the data is loaded, then exit

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::winit::WinitSettings;
use glowfish_data::{LoadPool, Scaling};
use glowfish_viz::{
    CellValue, Dataset, DisplayMode, DisplayedGrid, Selection, VisualizationConfig,
    VisualizationPlugin,
};

/// Frames and seconds rendered after loading finished before the screenshot is taken; render
/// pipelines compile in the background and are not drawn until they are ready.
const SETTLE_FRAMES: u32 = 10;
const SETTLE_SECS: f32 = 1.5;

#[derive(Resource)]
struct ScreenshotPlan {
    path: String,
    frames: u32,
    secs: f32,
    requested: bool,
}

fn main() -> AppExit {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut config = VisualizationConfig::default();
    let mut selection = None;
    let mut screenshot = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cubes" => config.mode = DisplayMode::Cubes,
            "--range" => config.cell_value = CellValue::Range,
            "--auto" => config.scaling = Scaling::Auto,
            "--rows" => {
                let rows = args.next().unwrap_or_default();
                let Some((start, end)) = rows
                    .split_once("..")
                    .and_then(|(s, e)| Some((s.parse::<f64>().ok()?, e.parse::<f64>().ok()?)))
                else {
                    eprintln!("--rows expects <start>..<end>, got {rows:?}");
                    return AppExit::error();
                };
                selection = Some(Selection::new(start, end));
            }
            "--screenshot" => screenshot = args.next(),
            _ if path.is_none() => path = Some(arg),
            _ => {
                eprintln!("unexpected argument {arg:?}");
                return AppExit::error();
            }
        }
    }
    let Some(path) = path else {
        eprintln!(
            "usage: view <file> [--cubes] [--range] [--auto] [--rows a..b] [--screenshot png]"
        );
        return AppExit::error();
    };
    let dataset = match Dataset::open(&path, 512 << 20, LoadPool::default()) {
        Ok(dataset) => dataset,
        Err(error) => {
            eprintln!("cannot open {path}: {error}");
            return AppExit::error();
        }
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("glowfish – {}", dataset.name),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(config)
    .add_plugins(VisualizationPlugin)
    .insert_resource(dataset);
    if let Some(selection) = selection {
        app.insert_resource(selection);
    }
    if let Some(path) = screenshot {
        // keep rendering without input until the screenshot is saved
        app.insert_resource(WinitSettings::continuous())
            .insert_resource(ScreenshotPlan {
                path,
                frames: 0,
                secs: 0.0,
                requested: false,
            })
            .add_systems(Last, take_screenshot);
    }
    app.run()
}

fn take_screenshot(
    mut commands: Commands,
    time: Res<Time>,
    mut plan: ResMut<ScreenshotPlan>,
    displayed: Option<Res<DisplayedGrid>>,
) {
    if plan.requested || !displayed.is_some_and(|d| d.complete) {
        return;
    }
    plan.frames += 1;
    plan.secs += time.delta_secs();
    if plan.frames < SETTLE_FRAMES || plan.secs < SETTLE_SECS {
        return;
    }
    plan.requested = true;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(plan.path.clone()))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}
