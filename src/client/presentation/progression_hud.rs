use bevy::prelude::*;

use crate::{
    client::state::ControlledPlayer,
    shared::{gameplay::progression::BaseProgression, states::ClientState},
};

const HUD_WIDTH: f32 = 340.0;
const HUD_HEIGHT: f32 = 20.0;
const HUD_LEFT_MARGIN: f32 = 12.0;
const HUD_BOTTOM_MARGIN: f32 = 2.0;

#[derive(Component)]
struct ProgressionHudRoot;

#[derive(Component)]
struct BaseLevelText;

#[derive(Component)]
struct ExperienceBarFill;

#[derive(Component)]
struct ExperiencePercentageText;

pub(crate) struct ProgressionHudPlugin;

impl Plugin for ProgressionHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ClientState::InGame), spawn_progression_hud)
            .add_systems(
                Update,
                update_progression_hud.run_if(in_state(ClientState::InGame)),
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
                column_gap: Val::Px(5.0),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.04, 0.055, 0.94)),
            BorderColor::all(Color::srgba(0.58, 0.61, 0.68, 0.95)),
            GlobalZIndex(290),
            Pickable::IGNORE,
            ProgressionHudRoot,
            Name::new("Base Level and Experience HUD"),
        ))
        .with_children(|hud| {
            hud.spawn((
                Text::new("Base Lv. 1"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.94, 0.97)),
                TextShadow {
                    offset: Vec2::new(1.0, 1.0),
                    color: Color::BLACK,
                },
                Node {
                    width: Val::Px(68.0),
                    ..default()
                },
                Pickable::IGNORE,
                BaseLevelText,
            ));

            hud.spawn((
                Node {
                    height: Val::Px(12.0),
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.07, 0.075, 0.09)),
                BorderColor::all(Color::srgb(0.34, 0.36, 0.42)),
                Pickable::IGNORE,
                Name::new("Base Experience Track"),
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
                    BackgroundColor(Color::srgb(0.92, 0.67, 0.12)),
                    Pickable::IGNORE,
                    ExperienceBarFill,
                    Name::new("Base Experience Fill"),
                ));

                track.spawn((
                    Text::new("0.0%"),
                    TextFont {
                        font_size: FontSize::Px(9.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextShadow {
                        offset: Vec2::new(1.0, 1.0),
                        color: Color::BLACK,
                    },
                    Pickable::IGNORE,
                    ExperiencePercentageText,
                ));
            });
        });
}

fn update_progression_hud(
    progression: Query<&BaseProgression, (With<ControlledPlayer>, Changed<BaseProgression>)>,
    mut level_text: Query<&mut Text, (With<BaseLevelText>, Without<ExperiencePercentageText>)>,
    mut percentage_text: Query<&mut Text, (With<ExperiencePercentageText>, Without<BaseLevelText>)>,
    mut fill: Query<&mut Node, With<ExperienceBarFill>>,
) {
    let Ok(progression) = progression.single() else {
        return;
    };
    let percentage = experience_percentage(progression);

    if let Ok(mut text) = level_text.single_mut() {
        text.0 = format!("Base Lv. {}", progression.level);
    }
    if let Ok(mut text) = percentage_text.single_mut() {
        text.0 = format!("{percentage:.1}%");
    }
    if let Ok(mut fill) = fill.single_mut() {
        fill.width = Val::Percent(percentage);
    }
}

fn despawn_progression_hud(mut commands: Commands, roots: Query<Entity, With<ProgressionHudRoot>>) {
    for entity in &roots {
        commands.entity(entity).try_despawn();
    }
}

fn experience_percentage(progression: &BaseProgression) -> f32 {
    match progression.experience_to_next_level() {
        Some(required) if required > 0 => {
            (progression.experience as f64 / required as f64 * 100.0).clamp(0.0, 100.0) as f32
        }
        _ => 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::gameplay::progression::MAX_BASE_LEVEL;

    #[test]
    fn percentage_uses_experience_toward_the_current_level_threshold() {
        assert_eq!(
            experience_percentage(&BaseProgression {
                level: 1,
                experience: 50,
            }),
            50.0
        );
        assert!(
            (experience_percentage(&BaseProgression {
                level: 3,
                experience: 25,
            }) - 8.333_333)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn max_level_displays_a_complete_bar() {
        assert_eq!(
            experience_percentage(&BaseProgression {
                level: MAX_BASE_LEVEL,
                experience: 0,
            }),
            100.0
        );
    }

    #[test]
    fn invalid_excess_experience_never_overfills_the_bar() {
        assert_eq!(
            experience_percentage(&BaseProgression {
                level: 1,
                experience: 1_000,
            }),
            100.0
        );
    }
}
