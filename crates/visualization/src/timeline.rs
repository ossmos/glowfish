use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use glowfish_data::{Grid, MinMaxCell, ValueRange};

use crate::camera::BlocksViewInput;
use crate::colormap::{rgba8, viridis};
use crate::spectrogram::value_range;
use crate::{
    Dataset, DisplayedGrid, Layout, Loading, Selection, VisualizationConfig, VisualizationEntity,
    format,
};

/// Height of the timeline panel in logical pixels.
pub const TIMELINE_HEIGHT: f32 = 110.0;
/// Time cells of the overview image.
const OVERVIEW_CELLS: usize = 1024;
/// Most frequency rows of the overview image.
const OVERVIEW_BANDS: usize = 24;
const DOUBLE_CLICK_SECS: f64 = 0.35;
const ACCENT: Color = Color::srgb(0.16, 0.47, 0.84);
const PANEL: Color = Color::srgb(0.14, 0.14, 0.15);
const HATCH: [[u8; 4]; 2] = [[70, 70, 74, 255], [52, 52, 56, 255]];

#[derive(Component)]
struct TimelineStrip;

#[derive(Component)]
struct TimelineWindow;

#[derive(Component)]
struct TimelineProgress;

#[derive(Component)]
struct TimelineLabel;

#[derive(Resource)]
struct OverviewImage(Handle<Image>);

/// Pixels the cursor has to move before a press on the timeline becomes a selection; shorter
/// movements are clicks (and double clicks) that keep the current window.
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Resource, Default)]
struct TimelineDrag {
    /// Moving the window (left button): the grabbed row's offset from the window start.
    grab: Option<f64>,
    /// Creating a window (right button): press position as a fraction of the timeline width,
    /// and in pixels.
    anchor: Option<(f64, f32)>,
    last_press: Option<(f64, f64)>,
}

/// Where a window grabbed at `grab` rows after its start goes when the cursor is at `row`.
fn moved_window(selection: Selection, grab: f64, row: f64, rows: u64) -> Selection {
    let start = row - grab;
    Selection::new(start, start + selection.span()).clamped(rows)
}

/// The selection while dragging from `anchor` to `current` (fractions of the timeline width,
/// `x` in pixels for the threshold); `None` while the drag is still a click.
fn drag_selection(anchor: (f64, f32), current: (f64, f32), rows: u64) -> Option<Selection> {
    if (current.1 - anchor.1).abs() < DRAG_THRESHOLD {
        return None;
    }
    let (a, b) = (anchor.0.clamp(0.0, 1.0), current.0.clamp(0.0, 1.0));
    let total = rows as f64;
    Some(Selection::new(a.min(b) * total, a.max(b) * total).clamped(rows))
}

pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimelineDrag>().add_systems(
            Update,
            (
                spawn_timeline.run_if(resource_changed::<Dataset>),
                timeline_input.before(crate::DataSystems),
                (update_overview, update_window, update_progress).after(crate::DataSystems),
            )
                .chain()
                .run_if(resource_exists::<Dataset>),
        );
    }
}

fn spawn_timeline(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    config: Res<VisualizationConfig>,
    dataset: Res<Dataset>,
) {
    if config.layout != Layout::Interactive {
        return;
    }
    let height = dataset.bands().min(OVERVIEW_BANDS);
    let image = images.add(Image::new(
        Extent3d {
            width: OVERVIEW_CELLS as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        hatch(OVERVIEW_CELLS, height),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ));
    commands.insert_resource(OverviewImage(image.clone()));

    commands.spawn((
        VisualizationEntity,
        BlocksViewInput,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(0),
            height: px(TIMELINE_HEIGHT),
            padding: UiRect::axes(px(12), px(10)),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        },
        BackgroundColor(PANEL),
        children![
            (
                TimelineStrip,
                Node {
                    height: px(56),
                    border: px(2).all(),
                    border_radius: BorderRadius::all(px(4)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.3, 0.3, 0.32)),
                children![
                    (
                        ImageNode {
                            image_mode: NodeImageMode::Stretch,
                            ..ImageNode::new(image)
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            right: px(0),
                            top: px(0),
                            bottom: px(0),
                            ..default()
                        },
                    ),
                    (
                        TimelineWindow,
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(0),
                            bottom: px(0),
                            border: px(3).all(),
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        BorderColor::all(ACCENT),
                        BackgroundColor(ACCENT.with_alpha(0.15)),
                    ),
                    (
                        TimelineProgress,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: px(0),
                            height: px(3),
                            width: percent(0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.75, 0.2)),
                        Visibility::Hidden,
                    ),
                ],
            ),
            (
                TimelineLabel,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ),
        ],
    ));
}

/// Hatched placeholder pixels for unloaded data.
fn hatch(width: usize, height: usize) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| HATCH[((x + y) / 3) % 2]))
        .flatten()
        .collect()
}

/// Rgba8 pixels of an overview grid; the highest frequency is the top row, unloaded cells are
/// hatched.
fn overview_pixels(
    grid: &Grid<MinMaxCell>,
    range: ValueRange,
    width: usize,
    height: usize,
) -> Vec<u8> {
    let mut pixels = hatch(width, height);
    for t in 0..grid.time_cells.min(width) {
        for f in 0..grid.freq_cells.min(height) {
            let cell = grid.cells[t * grid.freq_cells + f];
            if cell.is_empty() {
                continue;
            }
            let y = height - 1 - f;
            let at = (y * width + t) * 4;
            pixels[at..at + 4].copy_from_slice(&rgba8(viridis(range.normalize(cell.max))));
        }
    }
    pixels
}

fn update_overview(
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    displayed: Option<Res<DisplayedGrid>>,
    overview: Option<Res<OverviewImage>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(displayed), Some(overview)) = (displayed, overview) else {
        return;
    };
    if !displayed.is_changed() && !config.is_changed() {
        return;
    }
    let Some(mut image) = images.get_mut(&overview.0) else {
        return;
    };
    let (width, height) = (image.width() as usize, image.height() as usize);
    // the whole file at timeline resolution, only from what is cached
    let grid = dataset.pyramid.peek(0..dataset.rows(), width, height);
    let range = value_range(config.scaling, dataset.full_scale, &grid);
    let pixels = overview_pixels(&grid_to_width(&grid, width), range, width, height);
    image.data = Some(pixels);
}

/// Spreads the cells of an overview grid over `width` columns, so files with fewer rows than
/// pixels still fill the timeline.
fn grid_to_width(grid: &Grid<MinMaxCell>, width: usize) -> Grid<MinMaxCell> {
    let rows = grid.range.end - grid.range.start;
    if rows == 0 {
        return grid.clone();
    }
    let mut cells = Vec::with_capacity(width * grid.freq_cells);
    for x in 0..width {
        let row = grid.range.start + (x as u64 * rows) / width as u64;
        let t = ((row >> grid.level) - grid.first_cell) as usize;
        for f in 0..grid.freq_cells {
            cells.push(grid.get(t, f).unwrap_or(MinMaxCell {
                min: f32::INFINITY,
                max: f32::NEG_INFINITY,
            }));
        }
    }
    Grid {
        time_cells: width,
        cells,
        ..grid.clone()
    }
}

fn update_window(
    dataset: Res<Dataset>,
    selection: Option<Res<Selection>>,
    mut windows: Query<&mut Node, With<TimelineWindow>>,
    mut labels: Query<&mut Text, With<TimelineLabel>>,
) {
    let Some(selection) = selection else { return };
    if !selection.is_changed() {
        return;
    }
    let rows = dataset.rows().max(1) as f64;
    for mut node in &mut windows {
        node.left = percent(selection.start / rows * 100.0);
        node.width = percent((selection.span() / rows * 100.0).max(0.3));
    }
    let (start, end) = (
        dataset.row_time_ms(selection.start),
        dataset.row_time_ms(selection.end),
    );
    for mut text in &mut labels {
        text.0 = format!(
            "start {}    end {}    span {}    of {}",
            format::milliseconds(start),
            format::milliseconds(end),
            format::milliseconds(end - start),
            format::milliseconds(dataset.row_time_ms(rows)),
        );
    }
}

fn update_progress(
    loading: Res<Loading>,
    mut bars: Query<(&mut Node, &mut Visibility), With<TimelineProgress>>,
) {
    if !loading.is_changed() && loading.progress().is_none() {
        return;
    }
    for (mut node, mut visibility) in &mut bars {
        match loading.progress() {
            Some(progress) => {
                node.width = percent(progress * 100.0);
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

/// Left drag moves the window (a press outside of it centres it there first), right drag
/// creates a new window, a double left click selects the whole file.
fn timeline_input(
    time: Res<Time>,
    dataset: Res<Dataset>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    strips: Query<(&ComputedNode, &UiGlobalTransform), With<TimelineStrip>>,
    mut drag: ResMut<TimelineDrag>,
    selection: Option<ResMut<Selection>>,
) {
    let Some(mut selection) = selection else {
        return;
    };
    let Ok((node, transform)) = strips.single() else {
        return;
    };
    let rows = dataset.rows();
    let total = rows as f64;
    // the cursor as (fraction of the strip width, logical x); presses must be on the strip
    let cursor = window.cursor_position().and_then(|c| {
        let p = node.normalize_point(*transform, c * window.scale_factor())?;
        Some((p, (p.x as f64 + 0.5, c.x)))
    });
    let pressed_on_strip = |button| {
        buttons.just_pressed(button) && cursor.is_some_and(|(p, _)| p.abs().max_element() <= 0.5)
    };
    let mut moved = *selection;

    if pressed_on_strip(MouseButton::Left)
        && let Some((_, at)) = cursor
    {
        let now = time.elapsed_secs_f64();
        let double = drag.last_press.is_some_and(|(last, last_at)| {
            now - last < DOUBLE_CLICK_SECS && (at.0 - last_at).abs() < 0.01
        });
        drag.last_press = Some((now, at.0));
        let row = at.0 * total;
        if double {
            moved = Selection::all(rows);
            drag.grab = None;
            drag.last_press = None;
        } else if row >= moved.start && row <= moved.end {
            drag.grab = Some(row - moved.start);
        } else {
            let grab = moved.span() / 2.0;
            moved = moved_window(moved, grab, row, rows);
            drag.grab = Some(grab);
        }
    }
    if pressed_on_strip(MouseButton::Right)
        && let Some((_, at)) = cursor
    {
        drag.anchor = Some(at);
    }
    if !buttons.pressed(MouseButton::Left) {
        drag.grab = None;
    }
    if !buttons.pressed(MouseButton::Right) {
        drag.anchor = None;
    }

    if let (Some(grab), Some((_, at))) = (drag.grab, cursor) {
        moved = moved_window(moved, grab, at.0.clamp(0.0, 1.0) * total, rows);
    }
    if let (Some(anchor), Some((_, at))) = (drag.anchor, cursor)
        && let Some(created) = drag_selection(anchor, at, rows)
    {
        moved = created;
    }
    if moved != *selection {
        *selection = moved;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_selects_and_clicks_keep_the_window() {
        assert_eq!(drag_selection((0.2, 100.0), (0.21, 102.0), 1000), None);
        assert_eq!(
            drag_selection((0.5, 500.0), (0.25, 250.0), 1000),
            Some(Selection::new(250.0, 500.0))
        );
        // dragging beyond the strip clamps to the file
        assert_eq!(
            drag_selection((0.9, 900.0), (1.4, 1400.0), 1000),
            Some(Selection::new(900.0, 1000.0))
        );
    }

    #[test]
    fn moving_keeps_the_grabbed_row_under_the_cursor() {
        let window = Selection::new(100.0, 300.0);
        // grabbed 50 rows after the start, cursor now at row 600
        assert_eq!(
            moved_window(window, 50.0, 600.0, 1000),
            Selection::new(550.0, 750.0)
        );
        // stops at the end of the file
        assert_eq!(
            moved_window(window, 50.0, 990.0, 1000),
            Selection::new(800.0, 1000.0)
        );
    }

    #[test]
    fn overview_marks_unloaded_cells() {
        let grid = Grid {
            range: 0..2,
            level: 0,
            first_cell: 0,
            time_cells: 2,
            freq_cells: 1,
            bands_per_cell: 1,
            bands: 1,
            cells: vec![
                MinMaxCell { min: 0.0, max: 1.0 },
                MinMaxCell {
                    min: f32::INFINITY,
                    max: f32::NEG_INFINITY,
                },
            ],
        };
        let pixels = overview_pixels(&grid, ValueRange::new(0.0, 1.0), 2, 1);
        assert_eq!(pixels[..4], rgba8(viridis(1.0)));
        assert_eq!(pixels[4..8], HATCH[0]);
    }

    #[test]
    fn small_grids_are_spread_over_the_width() {
        let grid = Grid {
            range: 0..2,
            level: 0,
            first_cell: 0,
            time_cells: 2,
            freq_cells: 1,
            bands_per_cell: 1,
            bands: 1,
            cells: vec![
                MinMaxCell { min: 1.0, max: 1.0 },
                MinMaxCell { min: 2.0, max: 2.0 },
            ],
        };
        let wide = grid_to_width(&grid, 4);
        let maxima: Vec<f32> = wide.cells.iter().map(|c| c.max).collect();
        assert_eq!(maxima, [1.0, 1.0, 2.0, 2.0]);
    }
}
