//! Optional axes with ticks and labels: time and frequency on the ground plane of spectrograms,
//! time and amplitude around signal plots.
//!
//! Lines are gizmos; labels are UI text placed at the projected tick positions, so they stay
//! readable from every angle and scale with exported images.

use bevy::prelude::*;

use crate::camera::ViewRig;
use crate::layout::{FREQ_EXTENT, TIME_EXTENT};
use crate::signal::plot_rect;
use crate::spectrogram::value_range;
use crate::{
    AxisLabels, DataKind, Dataset, DisplayedGrid, Selection, VisualizationConfig,
    VisualizationEntity, format,
};

const COLOR: Color = Color::srgb(0.75, 0.75, 0.78);
/// Ticks to aim for along an axis.
const TICKS: usize = 6;
/// Distance of the spectrogram axes from the data, and length of their ticks, in world units.
const OFFSET: f32 = 0.3;
const TICK: f32 = 0.25;
/// Tick length and label gap of the signal axes in logical pixels.
const TICK_PX: f32 = 6.0;
const GAP_PX: f32 = 4.0;
const FONT_SIZE: f32 = 13.0;

pub struct AxesPlugin;

impl Plugin for AxesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (scale_lines, update_axes)
                .chain()
                .after(crate::DataSystems)
                .run_if(resource_exists::<Dataset>),
        );
    }
}

/// A container placed at a label's anchor point; its text is aligned to that point.
#[derive(Component)]
struct AxisLabel;

/// Where a label sits relative to its anchor point.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Anchor {
    /// Centred below the point.
    Below,
    /// Right-aligned, vertically centred left of the point.
    Left,
    /// Left-aligned above the point.
    Above,
}

#[derive(Debug, Clone, PartialEq)]
struct Label {
    /// Logical viewport position.
    at: Vec2,
    text: String,
    anchor: Anchor,
}

/// Evenly spaced "nice" values (1, 2 or 5 times a power of ten) within `min..=max`, about
/// `target` of them, and the step between them.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> (Vec<f64>, f64) {
    let span = max - min;
    if !span.is_finite() || span <= 0.0 {
        return (Vec::new(), 0.0);
    }
    let target = target.max(1) as f64;
    let magnitude = 10f64.powf((span / target).log10().floor());
    // the nice step whose number of ticks is closest to the target
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .min_by(|a, b| {
            let off = |step: f64| (span / step - target).abs();
            off(*a).total_cmp(&off(*b))
        })
        .expect("candidates are not empty");
    let first = (min / step).ceil();
    let ticks = (0..)
        .map(|i| (first + i as f64) * step)
        .take_while(|&v| v <= max + step * 1e-9)
        .collect();
    (ticks, step)
}

/// A tick value with as many decimals as the step needs.
pub fn tick_label(value: f64, step: f64) -> String {
    let decimals = if step > 0.0 {
        (-step.log10().floor()).clamp(0.0, 6.0) as usize
    } else {
        0
    };
    format::grouped(value, decimals)
}

/// Frequencies in Hz or kHz: (divisor, unit).
fn frequency_unit(max_hz: f64) -> (f64, &'static str) {
    if max_hz >= 1e3 {
        (1e3, "kHz")
    } else {
        (1.0, "Hz")
    }
}

/// Lines follow the label size, so exported images keep their proportions.
fn scale_lines(ui_scale: Res<UiScale>, mut store: ResMut<GizmoConfigStore>, mut done: Local<bool>) {
    if *done && !ui_scale.is_changed() {
        return;
    }
    *done = true;
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 1.5 * ui_scale.0;
}

#[allow(clippy::too_many_arguments)]
fn update_axes(
    mut commands: Commands,
    mut gizmos: Gizmos,
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    selection: Option<Res<Selection>>,
    displayed: Option<Res<DisplayedGrid>>,
    ui_scale: Res<UiScale>,
    rigs: Query<(&Camera, &ViewRig)>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    labels: Query<Entity, With<AxisLabel>>,
    mut previous: Local<Vec<Label>>,
) {
    let wanted = match (config.axes, selection) {
        (true, Some(selection)) => match dataset.kind {
            DataKind::Spectrogram => rigs
                .iter()
                .next()
                .map(|(camera, rig)| {
                    spectrogram_axes(
                        &mut gizmos,
                        &dataset,
                        &selection,
                        &config.axis_labels,
                        camera,
                        rig,
                    )
                })
                .unwrap_or_default(),
            DataKind::Signal => match (cameras_2d.iter().next(), displayed) {
                (Some((camera, transform)), Some(displayed)) => signal_axes(
                    &mut gizmos,
                    &dataset,
                    &config,
                    &selection,
                    &displayed,
                    ui_scale.0,
                    camera,
                    transform,
                ),
                _ => Vec::new(),
            },
        },
        _ => Vec::new(),
    };
    // labels only change when the view moves; rebuild them only then
    if *previous == wanted {
        return;
    }
    for entity in &labels {
        commands.entity(entity).despawn();
    }
    for label in &wanted {
        spawn_label(&mut commands, label, ui_scale.0);
    }
    *previous = wanted;
}

fn spawn_label(commands: &mut Commands, label: &Label, ui_scale: f32) {
    // a zero sized container at the anchor point; the text overflows it as aligned
    let (justify_content, align_items) = match label.anchor {
        Anchor::Below => (JustifyContent::Center, AlignItems::FlexStart),
        Anchor::Left => (JustifyContent::FlexEnd, AlignItems::Center),
        Anchor::Above => (JustifyContent::FlexStart, AlignItems::FlexEnd),
    };
    let at = label.at / ui_scale;
    commands.spawn((
        AxisLabel,
        VisualizationEntity,
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(at.x),
            top: px(at.y),
            width: px(0),
            height: px(0),
            justify_content,
            align_items,
            ..default()
        },
        children![(
            Text::new(label.text.clone()),
            TextFont {
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(COLOR),
            TextLayout::no_wrap(),
            Pickable::IGNORE,
        )],
    ));
}

/// Time along the front edge, frequency along the left edge of the data, on the ground.
fn spectrogram_axes(
    gizmos: &mut Gizmos,
    dataset: &Dataset,
    selection: &Selection,
    titles: &AxisLabels,
    camera: &Camera,
    rig: &ViewRig,
) -> Vec<Label> {
    let transform = GlobalTransform::from(rig.transform());
    let project = |p: Vec3| camera.world_to_viewport(&transform, p).ok();
    let mut labels = Vec::new();
    let mut push = |at: Option<Vec2>, text: String, anchor| {
        if let Some(at) = at
            && !text.is_empty()
        {
            labels.push(Label { at, text, anchor });
        }
    };

    // time
    let z = FREQ_EXTENT + OFFSET;
    gizmos.line(
        Vec3::new(0.0, 0.0, z),
        Vec3::new(TIME_EXTENT, 0.0, z),
        COLOR,
    );
    let (start, end) = (
        dataset.row_time_ms(selection.start),
        dataset.row_time_ms(selection.end),
    );
    let (ticks, step) = nice_ticks(start, end, TICKS);
    for ms in ticks {
        let x = ((ms - start) / (end - start)) as f32 * TIME_EXTENT;
        gizmos.line(Vec3::new(x, 0.0, z), Vec3::new(x, 0.0, z + TICK), COLOR);
        push(
            project(Vec3::new(x, 0.0, z + TICK * 1.5)),
            tick_label(ms, step),
            Anchor::Below,
        );
    }
    push(
        project(Vec3::new(TIME_EXTENT / 2.0, 0.0, z + TICK * 5.0)),
        titles.time.clone(),
        Anchor::Below,
    );

    // frequency
    let x = -OFFSET;
    gizmos.line(
        Vec3::new(x, 0.0, 0.0),
        Vec3::new(x, 0.0, FREQ_EXTENT),
        COLOR,
    );
    let (low, high) = (
        dataset.band_frequency(0.0),
        dataset.band_frequency(dataset.bands() as f64),
    );
    let (divisor, unit) = frequency_unit(high);
    let (ticks, step) = nice_ticks(low / divisor, high / divisor, TICKS);
    for value in ticks {
        let z = ((value * divisor - low) / (high - low)) as f32 * FREQ_EXTENT;
        gizmos.line(Vec3::new(x, 0.0, z), Vec3::new(x - TICK, 0.0, z), COLOR);
        push(
            project(Vec3::new(x - TICK * 1.5, 0.0, z)),
            tick_label(value, step),
            Anchor::Left,
        );
    }
    push(
        project(Vec3::new(x, 0.0, -TICK)),
        titles.frequency_in(unit),
        Anchor::Above,
    );
    labels
}

/// Time below, amplitude left of the plot.
#[allow(clippy::too_many_arguments)]
fn signal_axes(
    gizmos: &mut Gizmos,
    dataset: &Dataset,
    config: &VisualizationConfig,
    selection: &Selection,
    displayed: &DisplayedGrid,
    ui_scale: f32,
    camera: &Camera,
    transform: &GlobalTransform,
) -> Vec<Label> {
    let Some(size) = camera.logical_viewport_size() else {
        return Vec::new();
    };
    let rect = plot_rect(size, config.layout, ui_scale);
    let (tick, gap) = (TICK_PX * ui_scale, GAP_PX * ui_scale);
    let project = |p: Vec2| camera.world_to_viewport(transform, p.extend(0.0)).ok();
    let mut labels = Vec::new();
    let mut push = |at: Option<Vec2>, text: String, anchor| {
        if let Some(at) = at
            && !text.is_empty()
        {
            labels.push(Label { at, text, anchor });
        }
    };

    // time
    gizmos.line_2d(rect.min, Vec2::new(rect.max.x, rect.min.y), COLOR);
    let (start, end) = (
        dataset.row_time_ms(selection.start),
        dataset.row_time_ms(selection.end),
    );
    let (ticks, step) = nice_ticks(start, end, TICKS);
    for ms in ticks {
        let x = rect.min.x + ((ms - start) / (end - start)) as f32 * rect.width();
        gizmos.line_2d(
            Vec2::new(x, rect.min.y),
            Vec2::new(x, rect.min.y - tick),
            COLOR,
        );
        push(
            project(Vec2::new(x, rect.min.y - tick - gap)),
            tick_label(ms, step),
            Anchor::Below,
        );
    }
    push(
        project(Vec2::new(rect.max.x, rect.min.y - tick - gap)),
        config.axis_labels.time.clone(),
        Anchor::Left,
    );

    // amplitude
    gizmos.line_2d(rect.min, Vec2::new(rect.min.x, rect.max.y), COLOR);
    let range = value_range(config.scaling, dataset.full_scale, &displayed.grid);
    let (ticks, step) = nice_ticks(range.min as f64, range.max as f64, TICKS);
    for value in ticks {
        let y = rect.min.y + range.normalize(value as f32) * rect.height();
        gizmos.line_2d(
            Vec2::new(rect.min.x, y),
            Vec2::new(rect.min.x - tick, y),
            COLOR,
        );
        push(
            project(Vec2::new(rect.min.x - tick - gap, y)),
            tick_label(value, step),
            Anchor::Left,
        );
    }
    push(
        project(Vec2::new(rect.min.x, rect.max.y + gap)),
        config.axis_labels.amplitude.clone(),
        Anchor::Above,
    );
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_tick_steps() {
        let (ticks, step) = nice_ticks(0.0, 100.0, 5);
        assert_eq!(step, 20.0);
        assert_eq!(ticks, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);

        let (ticks, step) = nice_ticks(298_409.7, 298_745.2, 6);
        assert_eq!(step, 50.0);
        assert_eq!(ticks.first(), Some(&298_450.0));
        assert_eq!(ticks.last(), Some(&298_700.0));

        let (ticks, step) = nice_ticks(32_700.0, 34_100.0, 6);
        assert_eq!(step, 200.0);
        assert_eq!(ticks.first(), Some(&32_800.0));
        assert_eq!(ticks.last(), Some(&34_000.0));

        assert_eq!(nice_ticks(5.0, 5.0, 6).0, Vec::<f64>::new());
        assert_eq!(nice_ticks(0.0, f64::INFINITY, 6).0, Vec::<f64>::new());
    }

    #[test]
    fn tick_labels_use_the_step_precision() {
        assert_eq!(tick_label(298_450.0, 50.0), "298\u{2009}450");
        assert_eq!(tick_label(0.25, 0.05), "0.25");
        assert_eq!(tick_label(1.5, 0.5), "1.5");
    }

    #[test]
    fn frequency_units() {
        assert_eq!(frequency_unit(780_800.0), (1e3, "kHz"));
        assert_eq!(frequency_unit(500.0), (1.0, "Hz"));
    }
}
