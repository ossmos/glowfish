//! While a newly opened dataset loads, only the timeline and a loading indicator are shown: the
//! plot, its axes and the HUD (display mode, export, legend, readout) are hidden.

use bevy::prelude::*;

use crate::signal::SignalView;
use crate::spectrogram::SpectrogramView;
use crate::timeline::TIMELINE_HEIGHT;
use crate::{Dataset, DisplayedGrid, Layout, Loading, VisualizationConfig, VisualizationEntity};

/// Loads that finish faster than this do not show the indicator, so quick switches between
/// files do not flicker.
const SHOW_AFTER_SECS: f32 = 0.15;
/// Full turns of the spinner per second.
const TURNS_PER_SEC: f32 = 0.8;
const SPINNER_SIZE: f32 = 44.0;
const ACCENT: Color = Color::srgb(0.16, 0.47, 0.84);
const TRACK: Color = Color::srgb(0.25, 0.25, 0.28);

/// UI hidden while a newly opened dataset loads; starts hidden. Put it on the root of a UI
/// hierarchy: the children inherit it and keep their own visibility otherwise.
#[derive(Component)]
#[require(Visibility::Hidden)]
pub(crate) struct HideWhileLoading;

/// True from opening a dataset until its first view is completely loaded. Later loads (zooming,
/// panning) keep the plot visible: it shows coarser cached data until the new data is there.
pub(crate) fn is_loading(displayed: Option<&DisplayedGrid>, loading: &Loading) -> bool {
    displayed.is_none() || loading.initial_progress().is_some()
}

/// The overlay in the middle of the view.
#[derive(Component)]
struct LoadingIndicator;

#[derive(Component)]
struct Spinner;

#[derive(Component)]
struct LoadingText;

pub struct IndicatorPlugin;

impl Plugin for IndicatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_indicator.run_if(resource_exists::<crate::Rebuild>),
                (show_content, update_indicator).after(crate::DataSystems),
            )
                .chain()
                .run_if(resource_exists::<Dataset>),
        );
    }
}

fn spawn_indicator(mut commands: Commands, config: Res<VisualizationConfig>) {
    // exports wait for the data and never show it
    if config.layout != Layout::Interactive {
        return;
    }
    commands.spawn((
        VisualizationEntity,
        LoadingIndicator,
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(config.view_left),
            right: px(0),
            top: px(0),
            bottom: px(TIMELINE_HEIGHT),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(14),
            ..default()
        },
        Visibility::Hidden,
        children![
            (
                Spinner,
                Pickable::IGNORE,
                Node {
                    width: px(SPINNER_SIZE),
                    height: px(SPINNER_SIZE),
                    border: px(4).all(),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                // one coloured quarter on a dim ring
                BorderColor {
                    top: ACCENT,
                    right: TRACK,
                    bottom: TRACK,
                    left: TRACK,
                },
                UiTransform::IDENTITY,
            ),
            (
                LoadingText,
                Pickable::IGNORE,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ),
        ],
    ));
}

/// The plot and the HUD of a new dataset are only drawn once its first view is completely
/// loaded; before that the plot would be (partly) a flat field of placeholders. The timeline
/// shows the data that is loaded so far. Zooming and panning later do not hide them.
#[allow(clippy::type_complexity)]
fn show_content(
    displayed: Option<Res<DisplayedGrid>>,
    loading: Res<Loading>,
    mut content: Query<
        &mut Visibility,
        Or<(
            With<SpectrogramView>,
            With<SignalView>,
            With<HideWhileLoading>,
        )>,
    >,
) {
    let visible = !is_loading(displayed.as_deref(), &loading);
    for mut visibility in &mut content {
        visibility.set_if_neq(if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

fn update_indicator(
    time: Res<Time>,
    loading: Res<Loading>,
    config: Res<VisualizationConfig>,
    mut loading_for: Local<f32>,
    mut indicators: Query<&mut Visibility, With<LoadingIndicator>>,
    mut spinners: Query<&mut UiTransform, With<Spinner>>,
    mut texts: Query<&mut Text, With<LoadingText>>,
) {
    let progress = loading.initial_progress();
    *loading_for = match progress {
        Some(_) => *loading_for + time.delta_secs(),
        None => 0.0,
    };
    let shown = progress.is_some() && *loading_for >= SHOW_AFTER_SECS;
    for mut visibility in &mut indicators {
        visibility.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    let Some(progress) = progress.filter(|_| shown) else {
        return;
    };
    // loading requests redraws every frame, which keeps the spinner turning
    let angle = (*loading_for * TURNS_PER_SEC).fract() * std::f32::consts::TAU;
    for mut transform in &mut spinners {
        transform.rotation = Rot2::radians(angle);
    }
    let percent = format!("{:.0} %", progress * 100.0);
    let content = crate::fill(&config.labels.loading, &[("progress", &percent)]);
    for mut text in &mut texts {
        if text.0 != content {
            text.0 = content.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataPlugin;
    use crate::loading::tests::slow_dataset;

    #[test]
    fn plot_and_hud_are_hidden_until_data_arrives() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, DataPlugin))
            .init_resource::<VisualizationConfig>()
            .add_systems(Update, show_content.after(crate::DataSystems))
            .insert_resource(slow_dataset(20_000, 4));
        let view = app
            .world_mut()
            .spawn((SpectrogramView, Visibility::Inherited))
            .id();
        let hud = app.world_mut().spawn(HideWhileLoading).id();
        // the plot and the HUD are shown and hidden together
        let visibility = |app: &App| {
            let view = *app.world().get::<Visibility>(view).unwrap();
            assert_eq!(Some(&view), app.world().get::<Visibility>(hud));
            view
        };

        // nothing is cached when a dataset is opened
        app.update();
        assert_eq!(visibility(&app), Visibility::Hidden);
        assert!(
            app.world()
                .resource::<Loading>()
                .initial_progress()
                .is_some()
        );

        for _ in 0..2000 {
            app.update();
            if app.world().resource::<DisplayedGrid>().complete {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.world().resource::<DisplayedGrid>().complete);
        assert_eq!(visibility(&app), Visibility::Inherited);
        assert!(
            app.world()
                .resource::<Loading>()
                .initial_progress()
                .is_none()
        );

        // another dataset hides the plot again until its data is there
        app.insert_resource(slow_dataset(30_000, 4));
        app.update();
        assert_eq!(visibility(&app), Visibility::Hidden);
        assert!(
            app.world()
                .resource::<Loading>()
                .initial_progress()
                .is_some()
        );
    }

    #[test]
    fn zooming_keeps_the_plot_visible_with_coarse_data() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, DataPlugin))
            .init_resource::<VisualizationConfig>()
            .add_systems(Update, show_content.after(crate::DataSystems))
            .insert_resource(slow_dataset(20_000, 4));
        let view = app
            .world_mut()
            .spawn((SpectrogramView, Visibility::Inherited))
            .id();
        let wait_for_complete = |app: &mut App| {
            for _ in 0..2000 {
                app.update();
                if app.world().resource::<DisplayedGrid>().complete {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            panic!("loading did not complete");
        };
        wait_for_complete(&mut app);

        // far enough in that nothing is cached at the new level
        app.insert_resource(crate::Selection::new(10_000.0, 10_500.0));
        app.update();
        let displayed = app.world().resource::<DisplayedGrid>();
        assert!(!displayed.complete);
        assert_eq!(displayed.exact_cells, 0);
        assert!(displayed.grid.cells.iter().all(|c| !c.is_empty()));
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Inherited)
        );

        wait_for_complete(&mut app);
        let displayed = app.world().resource::<DisplayedGrid>();
        assert_eq!(displayed.exact_cells, displayed.grid.cells.len());
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Inherited)
        );
    }
}
