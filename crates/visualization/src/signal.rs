use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use glowfish_data::{Grid, MinMaxCell, ValueRange};

use crate::camera::{BlocksViewInput, cursor_over_ui};
use crate::colormap::PLACEHOLDER;
use crate::spectrogram::value_range;
use crate::{
    DataKind, Dataset, DisplayedGrid, Selection, VisualizationConfig, VisualizationEntity,
};

/// Space around the plot in logical pixels: left, right, top, bottom (the timeline is below).
pub const PLOT_MARGIN: [f32; 4] = [70.0, 70.0, 60.0, 150.0];
const LINE_COLOR: Color = Color::srgb(0.35, 0.63, 0.9);
const BACKGROUND: Color = Color::srgb(0.12, 0.12, 0.13);
const MIN_THICKNESS: f32 = 1.0;

/// The entity drawing the signal.
#[derive(Component)]
pub struct SignalView;

pub struct SignalPlugin;

impl Plugin for SignalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_signal.run_if(resource_changed::<Dataset>),
                signal_input.before(crate::DataSystems),
                update_signal.after(crate::DataSystems),
            )
                .run_if(is_signal),
        );
    }
}

fn is_signal(dataset: Option<Res<Dataset>>) -> bool {
    dataset.is_some_and(|d| d.kind == DataKind::Signal)
}

/// The plot rectangle in world coordinates of the default 2D camera (origin at the window
/// centre, y up, one unit per logical pixel).
pub fn plot_rect(window_size: Vec2) -> Rect {
    let [left, right, top, bottom] = PLOT_MARGIN;
    let half = window_size / 2.0;
    let min = Vec2::new(-half.x + left, -half.y + bottom);
    let max = Vec2::new(half.x - right, half.y - top);
    Rect::from_corners(min, max.max(min + 1.0))
}

fn spawn_signal(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        SignalView,
        VisualizationEntity,
        Mesh2d(meshes.add(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        ))),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
    ));
}

fn update_signal(
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    selection: Option<Res<Selection>>,
    displayed: Option<Res<DisplayedGrid>>,
    window: Single<Ref<Window>>,
    views: Query<&Mesh2d, With<SignalView>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(selection), Some(displayed)) = (selection, displayed) else {
        return;
    };
    if !selection.is_changed() && !displayed.is_changed() && !window.is_changed() {
        return;
    }
    let range = value_range(config.scaling, dataset.full_scale, &displayed.grid);
    let rect = plot_rect(window.size());
    for mesh in &views {
        if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
            *mesh = envelope_mesh(&displayed.grid, &selection, range, rect);
        }
    }
}

/// One quad per cell from its minimum to its maximum. Each cell is extended to reach the
/// neighbouring cell's values, so single samples are connected into a line.
pub fn envelope_mesh(
    grid: &Grid<MinMaxCell>,
    selection: &Selection,
    range: ValueRange,
    rect: Rect,
) -> Mesh {
    let mut positions: Vec<[f32; 3]> = vec![
        [rect.min.x, rect.min.y, -1.0],
        [rect.max.x, rect.min.y, -1.0],
        [rect.max.x, rect.max.y, -1.0],
        [rect.min.x, rect.max.y, -1.0],
    ];
    let mut colors = vec![BACKGROUND.to_linear().to_f32_array(); 4];
    let mut indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3];

    let span = selection.span().max(f64::MIN_POSITIVE);
    let x_of = |row: f64| {
        let x = rect.min.x as f64 + (row - selection.start) / span * rect.width() as f64;
        (x as f32).clamp(rect.min.x, rect.max.x)
    };
    let y_of = |value: f32| rect.min.y + range.normalize(value) * rect.height();
    let cells: Vec<MinMaxCell> = (0..grid.time_cells)
        .map(|t| grid.get(t, 0).expect("cell in grid"))
        .collect();
    for (t, cell) in cells.iter().enumerate() {
        let rows = grid.cell_rows(t);
        let (x0, x1) = (x_of(rows.start as f64), x_of(rows.end as f64));
        if x1 <= x0 {
            continue;
        }
        let (lo, hi, color) = if cell.is_empty() {
            let mid = rect.center().y;
            (mid - 2.0, mid + 2.0, PLACEHOLDER)
        } else {
            let mut lo = cell.min;
            let mut hi = cell.max;
            if let Some(next) = cells.get(t + 1).filter(|c| !c.is_empty()) {
                lo = lo.min(next.max);
                hi = hi.max(next.min);
            }
            let (lo, hi) = (y_of(lo), y_of(hi));
            let grow = ((MIN_THICKNESS - (hi - lo)) / 2.0).max(0.0);
            (lo - grow, hi + grow, LINE_COLOR)
        };
        let base = positions.len() as u32;
        positions.extend([[x0, lo, 0.0], [x1, lo, 0.0], [x1, hi, 0.0], [x0, hi, 0.0]]);
        colors.extend([color.to_linear().to_f32_array(); 4]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Pan with the pan button, zoom the time window around the cursor with the wheel.
#[allow(clippy::too_many_arguments)]
fn signal_input(
    config: Res<VisualizationConfig>,
    dataset: Res<Dataset>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<BlocksViewInput>>,
    mut dragging: Local<bool>,
    selection: Option<ResMut<Selection>>,
) {
    let Some(mut selection) = selection else {
        return;
    };
    let rect = plot_rect(window.size());
    let cursor = window.cursor_position();
    let over_ui = cursor.is_some_and(|c| cursor_over_ui(c, &window, &nodes));
    if buttons.just_pressed(config.controls.pan) {
        *dragging = !over_ui;
    }
    if !buttons.pressed(config.controls.pan) {
        *dragging = false;
    }

    let mut moved = *selection;
    if *dragging && motion.delta.x != 0.0 {
        let rows = -motion.delta.x as f64 / rect.width() as f64 * selection.span();
        moved = moved.shifted(rows, dataset.rows());
    }
    if scroll.delta.y != 0.0
        && !over_ui
        && let Some(cursor) = cursor
    {
        let lines = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        // cursor x in plot space: logical pixels from the left window edge
        let at = ((cursor.x - PLOT_MARGIN[0]) / rect.width()).clamp(0.0, 1.0) as f64;
        moved = zoom(moved, at, 0.85f64.powf(lines as f64), dataset.rows());
    }
    if moved != *selection {
        *selection = moved;
    }
}

/// Scales the selection by `factor` keeping the row at fraction `at` in place.
pub fn zoom(selection: Selection, at: f64, factor: f64, rows: u64) -> Selection {
    let pivot = selection.start + selection.span() * at;
    let span = selection.span() * factor;
    Selection::new(pivot - span * at, pivot - span * at + span).clamped(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(cells: Vec<MinMaxCell>) -> Grid<MinMaxCell> {
        Grid {
            range: 0..cells.len() as u64,
            level: 0,
            first_cell: 0,
            time_cells: cells.len(),
            freq_cells: 1,
            bands_per_cell: 1,
            bands: 1,
            cells,
        }
    }

    fn sample(v: f32) -> MinMaxCell {
        MinMaxCell { min: v, max: v }
    }

    #[test]
    fn envelope_connects_samples() {
        let g = grid(vec![
            sample(0.0),
            sample(10.0),
            MinMaxCell {
                min: 1.0,
                max: -1.0,
            },
        ]);
        let rect = Rect::new(0.0, 0.0, 30.0, 10.0);
        let mesh = envelope_mesh(
            &g,
            &Selection::new(0.0, 3.0),
            ValueRange::new(0.0, 10.0),
            rect,
        );
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions missing");
        };
        // background + 3 cells
        assert_eq!(positions.len(), 16);
        // the first cell reaches up to the next sample
        assert_eq!(positions[4][1], 0.0);
        assert_eq!(positions[6][1], 10.0);
        // the empty cell is a placeholder in the middle
        assert_eq!(positions[12][1], 3.0);
        assert_eq!(positions[14][1], 7.0);
    }

    #[test]
    fn zoom_keeps_the_pivot() {
        let s = zoom(Selection::new(100.0, 200.0), 0.25, 0.5, 1000);
        assert_eq!(s, Selection::new(112.5, 162.5));
        let s = zoom(Selection::new(0.0, 100.0), 0.5, 100.0, 1000);
        assert_eq!(s, Selection::all(1000));
    }

    #[test]
    fn plot_rect_respects_margins() {
        let rect = plot_rect(Vec2::new(1000.0, 800.0));
        assert_eq!(rect.min, Vec2::new(-430.0, -250.0));
        assert_eq!(rect.max, Vec2::new(430.0, 340.0));
    }
}
