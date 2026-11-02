use bevy::asset::RenderAssetUsages;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::ui_widgets::{Activate, Button, observe};
use glowfish_data::{Grid, MinMaxCell};

use crate::camera::{BlocksViewInput, ViewRig, ground_point};
use crate::colormap::{rgba8, viridis};
use crate::layout::{FREQ_EXTENT, TIME_EXTENT};
use crate::signal::{PLOT_MARGIN, plot_rect};
use crate::spectrogram::value_range;
use crate::timeline::TIMELINE_HEIGHT;
use crate::{
    DataKind, Dataset, DisplayMode, DisplayedGrid, Layout, Selection, VisualizationConfig,
    VisualizationEntity, format,
};

const BUTTON: Color = Color::srgb(0.2, 0.2, 0.22);
const BUTTON_HOVERED: Color = Color::srgb(0.28, 0.28, 0.31);
const BUTTON_SELECTED: Color = Color::srgb(0.88, 0.88, 0.88);
const TEXT: Color = Color::srgb(0.9, 0.9, 0.9);
const LEGEND_STEPS: u32 = 256;

#[derive(Component)]
struct ModeButton(DisplayMode);

#[derive(Component)]
struct LegendLabel {
    top: bool,
}

#[derive(Component)]
struct Readout;

#[derive(Component)]
struct Title;

/// The colour bar of the legend; only meaningful for spectrograms.
#[derive(Component)]
struct LegendBar;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_hud.run_if(resource_changed::<Dataset>),
                (style_buttons, update_legend, update_readout, update_title)
                    .after(crate::DataSystems),
            )
                .chain()
                .run_if(resource_exists::<Dataset>),
        );
    }
}

fn text(value: impl Into<String>, size: f32) -> impl Bundle {
    colored_text(value, size, TEXT)
}

fn colored_text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn button(label: &str) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        BlocksViewInput,
        Node {
            padding: UiRect::axes(px(10), px(3)),
            border: px(2).all(),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.7, 0.7, 0.7)),
        BackgroundColor(BUTTON),
        children![text(label, 15.0)],
    )
}

fn spawn_hud(
    mut commands: Commands,
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    mut images: ResMut<Assets<Image>>,
) {
    let interactive = config.layout == Layout::Interactive;
    // the timeline only exists in the interactive layout
    let bottom = if interactive { TIMELINE_HEIGHT } else { 0.0 };
    let legend = images.add(Image::new(
        Extent3d {
            width: 1,
            height: LEGEND_STEPS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        (0..LEGEND_STEPS)
            .rev()
            .flat_map(|i| rgba8(viridis(i as f32 / (LEGEND_STEPS - 1) as f32)))
            .collect(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));

    // top left: display mode (spectrograms only), or the title of a report
    if !interactive {
        commands.spawn((
            VisualizationEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                top: px(12),
                ..default()
            },
            children![(Title, text("", 16.0))],
        ));
    } else if dataset.kind == DataKind::Spectrogram {
        commands.spawn((
            VisualizationEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(10),
                top: px(10),
                column_gap: px(4),
                ..default()
            },
            children![
                (
                    button("Surface"),
                    ModeButton(DisplayMode::Surface),
                    observe(set_mode(DisplayMode::Surface))
                ),
                (
                    button("Cubes"),
                    ModeButton(DisplayMode::Cubes),
                    observe(set_mode(DisplayMode::Cubes))
                ),
            ],
        ));
    }

    // top right: export
    if interactive {
        commands.spawn((
            VisualizationEntity,
            Node {
                position_type: PositionType::Absolute,
                right: px(10),
                top: px(10),
                column_gap: px(6),
                ..default()
            },
            children![(button("PNG"), observe(export_png))],
        ));
    }

    // right: colour legend
    commands.spawn((
        VisualizationEntity,
        Node {
            position_type: PositionType::Absolute,
            right: px(14),
            top: px(52),
            bottom: px(bottom + 40.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            row_gap: px(4),
            ..default()
        },
        children![
            (LegendLabel { top: true }, text("", 13.0)),
            (
                LegendBar,
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(legend)
                },
                Node {
                    width: px(14),
                    // fill the column instead of taking the image's 1 x 256 aspect ratio
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_height: px(0),
                    border: px(1).all(),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.7, 0.7, 0.7)),
                // signals are drawn in one colour; the labels remain as the value range
                if dataset.kind == DataKind::Signal {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
            ),
            (LegendLabel { top: false }, text("", 13.0)),
        ],
    ));

    // bottom left: readout and axes
    let axes = match dataset.kind {
        DataKind::Spectrogram => "time: right   frequency: depth   amplitude: up",
        DataKind::Signal => "time: right   amplitude: up",
    };
    let labels = commands
        .spawn((
            VisualizationEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(10),
                bottom: px(bottom + 8.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
        ))
        .id();
    // the hint is redundant next to real axes
    if !config.axes {
        commands
            .entity(labels)
            .with_child(colored_text(axes, 13.0, Color::srgb(0.65, 0.65, 0.65)));
    }
    if interactive {
        commands
            .entity(labels)
            .with_child((Readout, text("", 14.0)));
    }
}

/// Report title: file name, time range and frequency range.
fn update_title(
    dataset: Res<Dataset>,
    selection: Option<Res<Selection>>,
    mut titles: Query<&mut Text, With<Title>>,
) {
    let Some(selection) = selection else { return };
    if !selection.is_changed() && !dataset.is_changed() {
        return;
    }
    let mut parts = vec![
        dataset.name.clone(),
        format!(
            "{} - {}",
            format::milliseconds(dataset.row_time_ms(selection.start)),
            format::milliseconds(dataset.row_time_ms(selection.end)),
        ),
    ];
    if dataset.kind == DataKind::Spectrogram {
        parts.push(format!(
            "{} - {}",
            format::frequency(dataset.band_frequency(0.0)),
            format::frequency(dataset.band_frequency(dataset.bands() as f64)),
        ));
    }
    let title = parts.join("    ");
    for mut text in &mut titles {
        text.0.clone_from(&title);
    }
}

fn set_mode(mode: DisplayMode) -> impl FnMut(On<Activate>, ResMut<VisualizationConfig>) {
    move |_, mut config| {
        if config.mode != mode {
            config.mode = mode;
        }
    }
}

fn export_png(
    _: On<Activate>,
    mut commands: Commands,
    dataset: Res<Dataset>,
    selection: Option<Res<Selection>>,
) {
    let range = selection.map_or(0..dataset.rows(), |s| s.rows());
    let stem = dataset.name.replace(['.', ' '], "_");
    let path = format!("{stem}_{}-{}.png", range.start, range.end);
    info!("exporting {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

fn style_buttons(
    config: Res<VisualizationConfig>,
    mut buttons: Query<
        (
            &Hovered,
            Option<&ModeButton>,
            &mut BackgroundColor,
            &Children,
        ),
        With<Button>,
    >,
    mut texts: Query<&mut TextColor>,
) {
    for (hovered, mode, mut background, children) in &mut buttons {
        let selected = mode.is_some_and(|m| m.0 == config.mode);
        let color = match (selected, hovered.get()) {
            (true, _) => BUTTON_SELECTED,
            (false, true) => BUTTON_HOVERED,
            (false, false) => BUTTON,
        };
        background.set_if_neq(BackgroundColor(color));
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                let color = if selected { Color::BLACK } else { TEXT };
                text.set_if_neq(TextColor(color));
            }
        }
    }
}

fn update_legend(
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    displayed: Option<Res<DisplayedGrid>>,
    mut labels: Query<(&LegendLabel, &mut Text, &mut Visibility)>,
) {
    let Some(displayed) = displayed else { return };
    if !displayed.is_changed() && !config.is_changed() {
        return;
    }
    // a signal's amplitude axis shows the same range; spectrograms keep them for the colours
    let visible = !(config.axes && dataset.kind == DataKind::Signal);
    let range = value_range(config.scaling, dataset.full_scale, &displayed.grid);
    for (label, mut text, mut visibility) in &mut labels {
        text.0 = format::amplitude(if label.top { range.max } else { range.min });
        visibility.set_if_neq(if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// The cell holding `row` and `band`, if the grid has data there.
pub fn cell_at(grid: &Grid<MinMaxCell>, row: u64, band: usize) -> Option<MinMaxCell> {
    let t = (row >> grid.level).checked_sub(grid.first_cell)? as usize;
    let f = band / grid.bands_per_cell.max(1);
    grid.get(t, f).filter(|c| !c.is_empty())
}

fn update_readout(
    dataset: Res<Dataset>,
    selection: Option<Res<Selection>>,
    displayed: Option<Res<DisplayedGrid>>,
    window: Single<&Window>,
    ui_scale: Res<UiScale>,
    cameras: Query<(&Camera, &GlobalTransform), With<ViewRig>>,
    mut readouts: Query<&mut Text, With<Readout>>,
) {
    let (Some(selection), Some(displayed)) = (selection, displayed) else {
        return;
    };
    let cursor = window.cursor_position();
    // (row, band) under the cursor
    let target = cursor.and_then(|cursor| match dataset.kind {
        DataKind::Spectrogram => {
            let (camera, transform) = cameras.single().ok()?;
            let point = ground_point(camera, transform, cursor)?;
            let inside =
                (0.0..=TIME_EXTENT).contains(&point.x) && (0.0..=FREQ_EXTENT).contains(&point.z);
            inside.then(|| {
                let row = selection.start + point.x as f64 / TIME_EXTENT as f64 * selection.span();
                let band = point.z / FREQ_EXTENT * dataset.bands() as f32;
                (row, band.min(dataset.bands() as f32 - 1.0) as usize)
            })
        }
        DataKind::Signal => {
            let rect = plot_rect(window.size(), Layout::Interactive, ui_scale.0);
            let x = (cursor.x - PLOT_MARGIN[0] * ui_scale.0) / rect.width();
            (0.0..=1.0)
                .contains(&x)
                .then(|| (selection.start + x as f64 * selection.span(), 0))
        }
    });
    let content = match target {
        Some((row, band)) => {
            let mut parts = vec![format!(
                "t = {}",
                format::milliseconds(dataset.row_time_ms(row))
            )];
            if dataset.kind == DataKind::Spectrogram {
                parts.push(format!(
                    "f = {}",
                    format::frequency(dataset.band_frequency(band as f64))
                ));
            }
            match cell_at(&displayed.grid, row as u64, band) {
                Some(cell) if cell.min == cell.max => {
                    parts.push(format!("amplitude {}", format::amplitude(cell.max)));
                }
                Some(cell) => parts.push(format!(
                    "max {}   min {}",
                    format::amplitude(cell.max),
                    format::amplitude(cell.min)
                )),
                None => parts.push("not loaded".into()),
            }
            parts.join("   |   ")
        }
        None => String::new(),
    };
    for mut text in &mut readouts {
        if text.0 != content {
            text.0.clone_from(&content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_lookup() {
        let grid = Grid {
            range: 8..24,
            level: 2,
            first_cell: 2,
            time_cells: 4,
            freq_cells: 2,
            bands_per_cell: 3,
            bands: 5,
            cells: (0..8)
                .map(|i| MinMaxCell {
                    min: i as f32,
                    max: i as f32,
                })
                .collect(),
        };
        // row 13 -> cell 3 -> t = 1; band 4 -> f = 1
        assert_eq!(cell_at(&grid, 13, 4).unwrap().max, 3.0);
        assert_eq!(cell_at(&grid, 4, 0), None);
        assert_eq!(cell_at(&grid, 100, 0), None);
    }
}
