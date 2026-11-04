//! The folder panel left of the view: project, measurement, channels and the files of a
//! channel. Clicking a file opens it.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::{Activate, Button, observe};
use glowfish_viz::{BlocksViewInput, Labels, VisualizationConfig, fill};

use crate::catalog::{Catalog, FileEntry, Select, display_name};
use crate::scan::{Scan, ScanFailed};

/// Width of the panel in UI pixels.
pub const PANEL_WIDTH: f32 = 280.0;

const PANEL: Color = Color::srgb(0.14, 0.14, 0.15);
const BORDER: Color = Color::srgb(0.3, 0.3, 0.32);
const TEXT: Color = Color::srgb(0.9, 0.9, 0.9);
const DIM: Color = Color::srgb(0.6, 0.6, 0.62);
const DISABLED: Color = Color::srgb(0.38, 0.38, 0.4);
const ACCENT: Color = Color::srgb(0.16, 0.47, 0.84);
const BUTTON: Color = Color::srgb(0.2, 0.2, 0.22);
const BUTTON_HOVERED: Color = Color::srgb(0.28, 0.28, 0.31);
/// Pixels scrolled per wheel line.
const SCROLL_LINE: f32 = 32.0;

#[derive(Component)]
pub struct Panel;

/// The scrollable file list.
#[derive(Component)]
pub(crate) struct FileList;

/// A button or list row that changes the selection.
#[derive(Component, Clone)]
pub(crate) struct Action(Select);

/// Background of a button when not hovered and when hovered.
#[derive(Component, Clone, Copy)]
pub(crate) struct Hover {
    normal: Color,
    hovered: Color,
}

impl Hover {
    fn new(normal: Color, hovered: Color) -> (Self, BackgroundColor) {
        (Self { normal, hovered }, BackgroundColor(normal))
    }
}

pub(crate) fn spawn_panel(mut commands: Commands) {
    commands.spawn((
        Panel,
        BlocksViewInput,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: px(PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(14),
            padding: UiRect::all(px(12)),
            border: UiRect::right(px(2)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    ));
}

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn title(value: impl Into<String>) -> impl Bundle {
    text(value, 13.0, DIM)
}

fn button(label: &str, action: Select) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Action(action),
        Node {
            padding: UiRect::axes(px(8), px(2)),
            border: px(2).all(),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BorderColor::all(BORDER),
        Hover::new(BUTTON, BUTTON_HOVERED),
        children![text(label, 15.0, TEXT)],
        observe(apply_action),
    )
}

fn apply_action(event: On<Activate>, actions: Query<&Action>, catalog: Option<ResMut<Catalog>>) {
    let (Ok(action), Some(mut catalog)) = (actions.get(event.entity), catalog) else {
        return;
    };
    // only mark the catalog changed if the selection really changed
    let mut next = catalog.clone();
    if next.apply(action.0.clone()) {
        *catalog = next;
    }
}

/// `← value (i/n) →` for stepping through projects or measurements.
fn stepper(
    parent: &mut ChildSpawnerCommands,
    value: String,
    index: usize,
    count: usize,
    make: fn(isize) -> Select,
) {
    parent
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            ..default()
        })
        .with_children(|row| {
            row.spawn(button("\u{2190}", make(-1)));
            row.spawn(Node {
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                column_gap: px(6),
                ..default()
            })
            .with_children(|value_box| {
                value_box.spawn(text(value, 15.0, TEXT));
                value_box.spawn(text(format!("{}/{count}", index + 1), 12.0, DIM));
            });
            row.spawn(button("\u{2192}", make(1)));
        });
}

/// Rebuilds the panel when the folder or the selection changes; keeps the file list's scroll
/// position.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_panel(
    mut commands: Commands,
    config: Res<VisualizationConfig>,
    catalog: Option<Res<Catalog>>,
    scan: Option<Res<Scan>>,
    failed: Option<Res<ScanFailed>>,
    panels: Query<Entity, With<Panel>>,
    lists: Query<&ScrollPosition, With<FileList>>,
    mut last_state: Local<Option<(bool, bool, bool)>>,
) {
    let state = (catalog.is_some(), scan.is_some(), failed.is_some());
    let catalog_changed = catalog.as_ref().is_some_and(|c| c.is_changed());
    if *last_state == Some(state) && !catalog_changed {
        return;
    }
    *last_state = Some(state);
    let Ok(panel) = panels.single() else { return };
    let scroll = lists.iter().next().map(|s| s.0).unwrap_or_default();
    let labels = &config.labels;
    commands.entity(panel).despawn_children();
    commands.entity(panel).with_children(|panel| {
        if let Some(scan) = &scan {
            let folder = scan.root.display().to_string();
            panel.spawn(text(
                fill(&labels.scanning, &[("folder", &folder)]),
                14.0,
                DIM,
            ));
            return;
        }
        if let Some(failed) = &failed {
            panel.spawn(text(failed.0.clone(), 14.0, Color::srgb(0.9, 0.4, 0.4)));
            return;
        }
        let Some(catalog) = &catalog else { return };
        build(panel, catalog, labels, scroll);
    });
}

fn build(panel: &mut ChildSpawnerCommands, catalog: &Catalog, labels: &Labels, scroll: Vec2) {
    let folder = catalog.root.display().to_string();
    let (Some(project), Some(measurement)) =
        (catalog.current_project(), catalog.current_measurement())
    else {
        panel.spawn(text(
            fill(&labels.no_files, &[("folder", &folder)]),
            14.0,
            DIM,
        ));
        return;
    };

    // project, only if the folder has several
    if catalog.projects.len() > 1 {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            })
            .with_children(|section| {
                section.spawn(title(labels.project.clone()));
                stepper(
                    section,
                    project.id.to_string(),
                    catalog.project,
                    catalog.projects.len(),
                    Select::Project,
                );
            });
    }

    // measurement
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        })
        .with_children(|section| {
            section.spawn(title(labels.measurement.clone()));
            stepper(
                section,
                format!("#{}", measurement.process),
                catalog.measurement,
                project.measurements.len(),
                Select::Measurement,
            );
            section.spawn(text(
                measurement.time.format("%Y-%m-%d %H:%M:%S").to_string(),
                12.0,
                DIM,
            ));
        });

    // channels of the project; channels without files in this measurement are greyed out
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        })
        .with_children(|section| {
            section.spawn(title(labels.channels.clone()));
            section
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(6),
                    row_gap: px(6),
                    ..default()
                })
                .with_children(|chips| {
                    for channel in project.channels() {
                        let count = measurement.count_on(channel);
                        let selected = catalog.channel == Some(channel);
                        let color = if count == 0 { DISABLED } else { TEXT };
                        let mut chip = chips.spawn((
                            Button,
                            Hovered::default(),
                            Action(Select::Channel(channel)),
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: px(5),
                                padding: UiRect::new(px(9), px(4), px(2), px(2)),
                                border: px(2).all(),
                                border_radius: BorderRadius::all(px(11)),
                                ..default()
                            },
                            BorderColor::all(if count == 0 { DISABLED } else { BORDER }),
                            if selected {
                                Hover::new(ACCENT, ACCENT)
                            } else if count == 0 {
                                Hover::new(PANEL, PANEL)
                            } else {
                                Hover::new(BUTTON, BUTTON_HOVERED)
                            },
                            observe(apply_action),
                        ));
                        chip.with_children(|chip| {
                            chip.spawn(text(channel.to_string(), 15.0, color));
                            chip.spawn((
                                Node {
                                    padding: UiRect::axes(px(5), px(0)),
                                    border_radius: BorderRadius::all(px(9)),
                                    ..default()
                                },
                                BackgroundColor(if count == 0 { PANEL } else { BORDER }),
                                children![text(count.to_string(), 11.0, color)],
                            ));
                        });
                        if count == 0 {
                            chip.insert(InteractionDisabled);
                        }
                    }
                });
        });

    // files on the selected channel
    let Some(channel) = catalog.channel else {
        return;
    };
    let files = measurement.files_on(channel);
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            flex_grow: 1.0,
            min_height: px(0),
            ..default()
        })
        .with_children(|section| {
            section
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn(title(fill(
                        &labels.files,
                        &[("channel", &channel.to_string())],
                    )));
                    header.spawn(text(files.len().to_string(), 12.0, DIM));
                });
            section
                .spawn((
                    FileList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        min_height: px(0),
                        overflow: Overflow::scroll_y(),
                        border: px(2).all(),
                        border_radius: BorderRadius::all(px(5)),
                        ..default()
                    },
                    ScrollPosition(scroll),
                    BorderColor::all(BORDER),
                ))
                .with_children(|list| {
                    for file in files {
                        file_row(
                            list,
                            file,
                            catalog.file.as_ref() == Some(&file.path),
                            labels,
                        );
                    }
                });
        });
}

fn file_row(list: &mut ChildSpawnerCommands, file: &FileEntry, selected: bool, labels: &Labels) {
    let name = display_name(labels, file.datatype);
    let details = if file.signal {
        fill(
            &labels.signal_details,
            &[("time", &file.compression_time.to_string())],
        )
    } else {
        fill(
            &labels.file_details,
            &[
                ("time", &file.compression_time.to_string()),
                ("frequency", &file.compression_frq.to_string()),
                ("bands", &file.frq_bands.to_string()),
            ],
        )
    };
    list.spawn((
        Button,
        Hovered::default(),
        Action(Select::File(file.path.clone())),
        Node {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            row_gap: px(2),
            padding: UiRect::new(px(8), px(8), px(5), px(5)),
            border: UiRect::left(px(4)),
            ..default()
        },
        BorderColor::all(if selected { ACCENT } else { Color::NONE }),
        if selected {
            Hover::new(ACCENT.with_alpha(0.18), ACCENT.with_alpha(0.25))
        } else {
            Hover::new(Color::NONE, BUTTON_HOVERED)
        },
        observe(apply_action),
        children![text(name, 14.0, TEXT), text(details, 12.0, DIM)],
    ));
}

/// Hover feedback.
pub(crate) fn style_buttons(
    mut buttons: Query<(&Hover, &Hovered, &mut BackgroundColor), Changed<Hovered>>,
) {
    for (hover, hovered, mut background) in &mut buttons {
        let color = if hovered.get() {
            hover.hovered
        } else {
            hover.normal
        };
        background.set_if_neq(BackgroundColor(color));
    }
}

/// Scrolls the file list with the mouse wheel.
pub(crate) fn scroll_list(
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    mut lists: Query<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<FileList>>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let physical = cursor * window.scale_factor();
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / SCROLL_LINE,
    };
    for (node, transform, mut position) in &mut lists {
        if node.contains_point(*transform, physical) {
            let max = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor();
            position.0.y = (position.0.y - lines * SCROLL_LINE).clamp(0.0, max);
        }
    }
}
