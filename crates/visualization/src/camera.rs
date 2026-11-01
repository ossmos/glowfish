use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use crate::layout::{FREQ_EXTENT, TIME_EXTENT};
use crate::{DataKind, Dataset, Perspective, Selection, VisualizationConfig, VisualizationEntity};

const MIN_PITCH: f32 = 0.05;
const MAX_PITCH: f32 = 1.5;
const MIN_DISTANCE: f32 = 3.0;
const MAX_DISTANCE: f32 = 80.0;
const ROTATE_SPEED: f32 = 0.006;
/// Factor of the time window per wheel line when zooming in time.
pub const ZOOM_STEP: f64 = 0.85;

/// Orbit camera around a focus point on the ground plane. The camera itself never moves along
/// the time axis: panning in time moves the data (the selection) instead.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ViewRig {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub focus: Vec3,
}

impl ViewRig {
    pub fn new(perspective: Perspective) -> Self {
        Self {
            yaw: perspective.yaw,
            pitch: perspective.pitch.clamp(MIN_PITCH, MAX_PITCH),
            distance: perspective.distance.clamp(MIN_DISTANCE, MAX_DISTANCE),
            focus: Vec3::new(TIME_EXTENT / 2.0, 0.0, FREQ_EXTENT / 2.0),
        }
    }

    pub fn transform(&self) -> Transform {
        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let position = self.focus + rotation * Vec3::new(0.0, 0.0, self.distance);
        Transform::from_translation(position).looking_at(self.focus, Vec3::Y)
    }
}

/// UI nodes with this component capture the mouse: drags starting on them do not move the view.
#[derive(Component, Default)]
pub struct BlocksViewInput;

/// The mouse button of the drag in progress, if it started in the view.
#[derive(Resource, Default)]
struct ViewDrag(Option<MouseButton>);

pub struct ViewCameraPlugin;

impl Plugin for ViewCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewDrag>().add_systems(
            Update,
            (
                spawn_camera.run_if(resource_changed::<Dataset>),
                (track_drag, view_input, apply_rig).chain(),
            )
                .chain()
                .before(crate::DataSystems)
                .run_if(resource_exists::<Dataset>),
        );
    }
}

fn spawn_camera(mut commands: Commands, dataset: Res<Dataset>, config: Res<VisualizationConfig>) {
    match dataset.kind {
        DataKind::Spectrogram => {
            let rig = ViewRig::new(config.perspective);
            commands.spawn((
                Camera3d::default(),
                rig.transform(),
                rig,
                VisualizationEntity,
            ));
            commands.spawn((
                DirectionalLight {
                    illuminance: 4_000.0,
                    ..default()
                },
                Transform::from_xyz(4.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
                VisualizationEntity,
            ));
        }
        DataKind::Signal => {
            commands.spawn((Camera2d, VisualizationEntity));
        }
    }
}

/// True if `cursor` (logical pixels) is over a node that blocks view input.
pub fn cursor_over_ui(
    cursor: Vec2,
    window: &Window,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<BlocksViewInput>>,
) -> bool {
    let physical = cursor * window.scale_factor();
    nodes.iter().any(|(node, transform, visible)| {
        visible.get() && node.contains_point(*transform, physical)
    })
}

fn track_drag(
    mut drag: ResMut<ViewDrag>,
    buttons: Res<ButtonInput<MouseButton>>,
    config: Res<VisualizationConfig>,
    window: Single<&Window>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<BlocksViewInput>>,
) {
    let controls = config.controls;
    if let Some(button) = drag.0
        && !buttons.pressed(button)
    {
        drag.0 = None;
    }
    for button in [controls.pan, controls.rotate] {
        if drag.0.is_none() && buttons.just_pressed(button) {
            let over_ui = window
                .cursor_position()
                .is_some_and(|cursor| cursor_over_ui(cursor, &window, &nodes));
            if !over_ui {
                drag.0 = Some(button);
            }
        }
    }
}

/// Rotate with the rotate button, pan along time with the pan button (the data under the cursor
/// follows it), wheel zooms the camera, Ctrl + wheel zooms the time window around the cursor.
#[allow(clippy::too_many_arguments)]
fn view_input(
    drag: Res<ViewDrag>,
    config: Res<VisualizationConfig>,
    dataset: Res<Dataset>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<BlocksViewInput>>,
    mut rigs: Query<(&mut ViewRig, &Projection, &Camera, &GlobalTransform)>,
    selection: Option<ResMut<Selection>>,
) {
    let Some(mut selection) = selection else {
        return;
    };
    let delta = motion.delta;
    let cursor = window.cursor_position();
    let over_ui = cursor.is_some_and(|c| cursor_over_ui(c, &window, &nodes));
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    let zoom_time = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let mut moved = *selection;
    for (mut rig, projection, camera, camera_transform) in &mut rigs {
        if drag.0 == Some(config.controls.rotate) && delta != Vec2::ZERO {
            rig.yaw -= delta.x * ROTATE_SPEED;
            rig.pitch = (rig.pitch + delta.y * ROTATE_SPEED).clamp(MIN_PITCH, MAX_PITCH);
        }
        if drag.0 == Some(config.controls.pan) && delta != Vec2::ZERO {
            let world = pan_on_ground(&rig, projection, window.height(), delta);
            // only along time: dragging right shows earlier rows
            let rows = -world.x as f64 / TIME_EXTENT as f64 * moved.span();
            moved = moved.shifted(rows, dataset.rows());
        }
        if lines != 0.0 && !over_ui {
            if zoom_time {
                // keep the time under the cursor in place; the view centre if it is off the data
                let at = cursor
                    .and_then(|c| ground_point(camera, camera_transform, c))
                    .map_or(0.5, |p| (p.x / TIME_EXTENT) as f64);
                moved = moved.zoomed(at, ZOOM_STEP.powf(lines as f64), dataset.rows());
            } else {
                rig.distance =
                    (rig.distance * 0.9f32.powf(lines)).clamp(MIN_DISTANCE, MAX_DISTANCE);
            }
        }
    }
    if moved != *selection {
        *selection = moved;
    }
}

/// The point on the ground plane (y = 0) under `cursor`.
pub fn ground_point(camera: &Camera, transform: &GlobalTransform, cursor: Vec2) -> Option<Vec3> {
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let distance = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    Some(ray.get_point(distance))
}

/// A screen drag of `delta` pixels as a movement on the ground plane (x along time, y along
/// frequency), scaled so the data under the cursor follows it.
pub fn pan_on_ground(
    rig: &ViewRig,
    projection: &Projection,
    viewport_height: f32,
    delta: Vec2,
) -> Vec2 {
    let fov = match projection {
        Projection::Perspective(p) => p.fov,
        _ => std::f32::consts::FRAC_PI_4,
    };
    let units_per_pixel = 2.0 * rig.distance * (fov / 2.0).tan() / viewport_height.max(1.0);
    let transform = rig.transform();
    let right = flatten(transform.right().as_vec3());
    let forward = flatten(transform.forward().as_vec3());
    let moved = (right * delta.x - forward * delta.y) * units_per_pixel;
    Vec2::new(moved.x, moved.z)
}

fn flatten(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z).normalize_or_zero()
}

fn apply_rig(mut rigs: Query<(&ViewRig, &mut Transform), Changed<ViewRig>>) {
    for (rig, mut transform) in &mut rigs {
        *transform = rig.transform();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rig_looks_at_focus_from_above() {
        let rig = ViewRig::new(Perspective {
            yaw: 0.0,
            pitch: 0.5,
            distance: 10.0,
        });
        let t = rig.transform();
        assert!(t.translation.y > 0.0);
        let to_focus = (rig.focus - t.translation).normalize();
        assert!(t.forward().dot(to_focus) > 0.999);
        assert!((t.translation.distance(rig.focus) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn pitch_and_distance_are_clamped() {
        let rig = ViewRig::new(Perspective {
            yaw: 0.0,
            pitch: 3.0,
            distance: 1000.0,
        });
        assert_eq!(rig.pitch, MAX_PITCH);
        assert_eq!(rig.distance, MAX_DISTANCE);
    }

    #[test]
    fn panning_right_moves_along_time_when_looking_along_frequency() {
        // yaw 0 looks along -z, so screen right is +x (time)
        let rig = ViewRig::new(Perspective {
            yaw: 0.0,
            pitch: 0.6,
            distance: 10.0,
        });
        let moved = pan_on_ground(&rig, &Projection::default(), 600.0, Vec2::new(100.0, 0.0));
        assert!(moved.x > 0.0);
        assert!(moved.y.abs() < 1e-4);
        // dragging down (positive y) moves towards the camera (+z)
        let moved = pan_on_ground(&rig, &Projection::default(), 600.0, Vec2::new(0.0, 100.0));
        assert!(moved.y > 0.0);
    }
}
