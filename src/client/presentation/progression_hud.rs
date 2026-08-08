use bevy::prelude::*;

use crate::{
    client::{presentation::ui_drag::DraggableUi, state::ControlledPlayer},
    shared::{
        gameplay::progression::{BaseProgression, JobProgression},
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

const HUD_WIDTH: f32 = 390.0;
const HUD_HEIGHT: f32 = 42.0;
const HUD_LEFT_MARGIN: f32 = 12.0;
const HUD_BOTTOM_MARGIN: f32 = 2.0;

#[derive(Component)]
struct ProgressionHudRoot;

#[derive(Component)]
struct BaseLevelText;

#[derive(Component)]
struct BaseExperienceBarFill;

#[derive(Component)]
struct BaseExperiencePercentageText;

#[derive(Component)]
struct JobLevelText;

#[derive(Component)]
struct JobExperienceBarFill;

#[derive(Component)]
struct JobExperiencePercentageText;

pub(crate) struct ProgressionHudPlugin;

impl Plugin for ProgressionHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ClientState::InGame), spawn_progression_hud)
            .add_systems(
                Update,
                (update_progression_hud, request_placeholder_class_cycle)
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_progression_hud);
    }
}

fn spawn_progression_hud(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(HUD_LEFT_MARGIN),
                bottom: Val::Px(HUD_BOTTOM_MARGIN),
                width: Val::Px(HUD_WIDTH),
                height: Val::Px(HUD_HEIGHT),
                padding: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.04, 0.055, 0.94)),
            BorderColor::all(Color::srgba(0.58, 0.61, 0.68, 0.95)),
            GlobalZIndex(290),
            Pickable::IGNORE,
            DraggableUi::entire_panel(),
            ProgressionHudRoot,
            Name::new("Base and Job Progression HUD"),
        ))
        .with_children(|hud| {
            spawn_progression_row(
                hud,
                "Base Lv. 1",
                BaseLevelText,
                BaseExperienceBarFill,
                BaseExperiencePercentageText,
                Color::srgb(0.92, 0.67, 0.12),
                "Base Experience",
            );
            spawn_progression_row(
                hud,
                "Novice Job Lv. 1 [J]",
                JobLevelText,
                JobExperienceBarFill,
                JobExperiencePercentageText,
                Color::srgb(0.28, 0.68, 0.96),
                "Job Experience",
            );
        });
}

fn spawn_progression_row<
    LevelMarker: Component,
    FillMarker: Component,
    PercentageMarker: Component,
>(
    hud: &mut ChildSpawnerCommands,
    initial_label: &str,
    level_marker: LevelMarker,
    fill_marker: FillMarker,
    percentage_marker: PercentageMarker,
    fill_color: Color,
    name: &'static str,
) {
    hud.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(16.0),
            column_gap: Val::Px(5.0),
            align_items: AlignItems::Center,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|row| {
        row.spawn((
            Text::new(initial_label),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.94, 0.94, 0.97)),
            TextShadow {
                offset: Vec2::new(1.0, 1.0),
                color: Color::BLACK,
            },
            Node {
                width: Val::Px(120.0),
                ..default()
            },
            Pickable::IGNORE,
            level_marker,
        ));

        row.spawn((
            Node {
                height: Val::Px(11.0),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.07, 0.075, 0.09)),
            BorderColor::all(Color::srgb(0.34, 0.36, 0.42)),
            Pickable::IGNORE,
            Name::new(format!("{name} Track")),
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(fill_color),
                Pickable::IGNORE,
                fill_marker,
                Name::new(format!("{name} Fill")),
            ));
            track.spawn((
                Text::new("0.0%"),
                TextFont {
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::new(1.0, 1.0),
                    color: Color::BLACK,
                },
                Pickable::IGNORE,
                percentage_marker,
            ));
        });
    });
}

fn update_progression_hud(
    progression: Query<
        (&BaseProgression, &JobProgression),
        (
            With<ControlledPlayer>,
            Or<(Changed<BaseProgression>, Changed<JobProgression>)>,
        ),
    >,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<BaseLevelText>>,
        Query<&mut Text, With<BaseExperiencePercentageText>>,
        Query<&mut Text, With<JobLevelText>>,
        Query<&mut Text, With<JobExperiencePercentageText>>,
    )>,
    mut base_fill: Query<&mut Node, (With<BaseExperienceBarFill>, Without<JobExperienceBarFill>)>,
    mut job_fill: Query<&mut Node, (With<JobExperienceBarFill>, Without<BaseExperienceBarFill>)>,
) {
    let Ok((base, job)) = progression.single() else {
        return;
    };
    let base_percentage = base_experience_percentage(base);
    let job_percentage = job_experience_percentage(job);

    {
        let mut base_level_text = text_queries.p0();
        if let Ok(mut text) = base_level_text.single_mut() {
            text.0 = format!("Base Lv. {}", base.level);
        }
    }
    {
        let mut base_percentage_text = text_queries.p1();
        if let Ok(mut text) = base_percentage_text.single_mut() {
            text.0 = format!("{base_percentage:.1}%");
        }
    }
    if let Ok(mut fill) = base_fill.single_mut() {
        fill.width = Val::Percent(base_percentage);
    }

    {
        let mut job_level_text = text_queries.p2();
        if let Ok(mut text) = job_level_text.single_mut() {
            text.0 = format!("{} Job Lv. {} [J]", job.class.name(), job.level);
        }
    }
    {
        let mut job_percentage_text = text_queries.p3();
        if let Ok(mut text) = job_percentage_text.single_mut() {
            text.0 = format!("{job_percentage:.1}%");
        }
    }
    if let Ok(mut fill) = job_fill.single_mut() {
        fill.width = Val::Percent(job_percentage);
    }
}

fn request_placeholder_class_cycle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_commands: MessageWriter<PlayerCommand>,
) {
    if keyboard.just_pressed(KeyCode::KeyJ) {
        player_commands.write(PlayerCommand::CyclePlaceholderClass);
    }
}

fn despawn_progression_hud(mut commands: Commands, roots: Query<Entity, With<ProgressionHudRoot>>) {
    for entity in &roots {
        commands.entity(entity).try_despawn();
    }
}

fn base_experience_percentage(progression: &BaseProgression) -> f32 {
    experience_percentage(
        progression.experience,
        progression.experience_to_next_level(),
    )
}

fn job_experience_percentage(progression: &JobProgression) -> f32 {
    experience_percentage(
        progression.experience,
        progression.experience_to_next_level(),
    )
}

fn experience_percentage(experience: u64, required: Option<u64>) -> f32 {
    match required {
        Some(required) if required > 0 => {
            (experience as f64 / required as f64 * 100.0).clamp(0.0, 100.0) as f32
        }
        _ => 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::gameplay::progression::{CharacterClass, MAX_BASE_LEVEL};

    #[test]
    fn base_percentage_uses_the_current_level_threshold() {
        assert_eq!(
            base_experience_percentage(&BaseProgression {
                level: 1,
                experience: 50,
            }),
            50.0
        );
    }

    #[test]
    fn job_percentage_uses_the_current_class_curve() {
        assert_eq!(
            job_experience_percentage(&JobProgression {
                class: CharacterClass::Novice,
                level: 1,
                experience: 20,
            }),
            50.0
        );
    }

    #[test]
    fn maximum_levels_display_complete_bars() {
        assert_eq!(
            base_experience_percentage(&BaseProgression {
                level: MAX_BASE_LEVEL,
                experience: 0,
            }),
            100.0
        );
        assert_eq!(
            job_experience_percentage(&JobProgression {
                class: CharacterClass::Novice,
                level: CharacterClass::Novice.max_job_level(),
                experience: 0,
            }),
            100.0
        );
    }

    #[test]
    fn progression_hud_system_initializes_and_updates_all_rows() {
        let mut app = App::new();
        app.add_systems(Update, update_progression_hud);
        app.world_mut().spawn((
            ControlledPlayer,
            BaseProgression {
                level: 2,
                experience: 100,
            },
            JobProgression {
                class: CharacterClass::Novice,
                level: 2,
                experience: 40,
            },
        ));

        let base_level = app.world_mut().spawn((Text::new(""), BaseLevelText)).id();
        let base_percentage = app
            .world_mut()
            .spawn((Text::new(""), BaseExperiencePercentageText))
            .id();
        let base_fill = app
            .world_mut()
            .spawn((Node::default(), BaseExperienceBarFill))
            .id();
        let job_level = app.world_mut().spawn((Text::new(""), JobLevelText)).id();
        let job_percentage = app
            .world_mut()
            .spawn((Text::new(""), JobExperiencePercentageText))
            .id();
        let job_fill = app
            .world_mut()
            .spawn((Node::default(), JobExperienceBarFill))
            .id();

        app.update();

        assert_eq!(app.world().get::<Text>(base_level).unwrap().0, "Base Lv. 2");
        assert_eq!(app.world().get::<Text>(base_percentage).unwrap().0, "50.0%");
        assert_eq!(
            app.world().get::<Node>(base_fill).unwrap().width,
            Val::Percent(50.0)
        );
        assert_eq!(
            app.world().get::<Text>(job_level).unwrap().0,
            format!("{} Job Lv. 2 [J]", CharacterClass::Novice.name())
        );
        assert_eq!(app.world().get::<Text>(job_percentage).unwrap().0, "50.0%");
        assert_eq!(
            app.world().get::<Node>(job_fill).unwrap().width,
            Val::Percent(50.0)
        );
    }
}
