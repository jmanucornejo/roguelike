use bevy::prelude::*;

use crate::{
    client::{presentation::ui_drag::DraggableUi, state::ControlledPlayer},
    shared::{
        gameplay::{
            components::{
                CharacterAttribute, CharacterStats, Equipment, Health, Mana, MAX_ATTRIBUTE_VALUE,
            },
            items::{equipment_bonuses, equipment_derived_stats},
            progression::BaseProgression,
        },
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

const PANEL_WIDTH: f32 = 320.0;
const PANEL_HEIGHT: f32 = 510.0;
const PANEL_RIGHT: f32 = 10.0;
const PANEL_TOP: f32 = 12.0;

#[derive(Resource, Debug, Default)]
pub(crate) struct CharacterStatusUiState {
    open: bool,
}

#[derive(Component)]
struct CharacterStatusRoot;

#[derive(Component)]
struct AvailableAttributePointsText;

#[derive(Component)]
struct AttributeValueText(CharacterAttribute);

#[derive(Component)]
struct AttributeIncreaseButton(CharacterAttribute);

#[derive(Component)]
struct AttributeCostText(CharacterAttribute);

#[derive(Component)]
struct DerivedStatsText;

pub(crate) struct CharacterStatusPlugin;

impl Plugin for CharacterStatusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterStatusUiState>()
            .add_systems(OnEnter(ClientState::InGame), spawn_character_status)
            .add_systems(
                Update,
                (
                    toggle_character_status,
                    spend_attribute_point,
                    update_character_status,
                )
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_character_status);
    }
}

fn spawn_character_status(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(PANEL_RIGHT),
                top: Val::Px(PANEL_TOP),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(PANEL_HEIGHT),
                padding: UiRect::all(Val::Px(11.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.042, 0.065, 0.98)),
            BorderColor::all(Color::srgb(0.55, 0.68, 0.86)),
            GlobalZIndex(720),
            Visibility::Hidden,
            Pickable::IGNORE,
            DraggableUi::header(36.0),
            CharacterStatusRoot,
            Name::new("Character Status Panel"),
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|header| {
                    header.spawn((
                        Text::new("Character Status [C]"),
                        TextFont {
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.86, 0.52)),
                        Pickable::IGNORE,
                    ));
                    header.spawn((
                        Text::new("Points: 0"),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.42, 0.84, 1.0)),
                        Node {
                            margin: UiRect::left(Val::Auto),
                            ..default()
                        },
                        Pickable::IGNORE,
                        AvailableAttributePointsText,
                    ));
                });

            panel.spawn((
                Text::new("Costs rise every 10 attribute points. Attributes cap at 99."),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.66, 0.70, 0.78)),
                Pickable::IGNORE,
            ));

            for attribute in CharacterAttribute::ALL {
                spawn_attribute_row(panel, attribute);
            }

            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::top(Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.09, 0.14, 0.96)),
                BorderColor::all(Color::srgb(0.25, 0.34, 0.49)),
                Pickable::IGNORE,
            ))
            .with_children(|derived| {
                derived.spawn((
                    Text::new(
                        "Derived Stats\nHIT: --    FLEE: --\nMax HP: --    Max SP: --\nATK: --    Magic: --\nDEF: --    MDEF: --",
                    ),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.90, 1.0)),
                    Pickable::IGNORE,
                    DerivedStatsText,
                ));
            });
        });
}

fn spawn_attribute_row(panel: &mut ChildSpawnerCommands, attribute: CharacterAttribute) {
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(48.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.11, 0.94)),
            BorderColor::all(Color::srgb(0.22, 0.26, 0.34)),
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|main| {
                main.spawn((
                    Text::new(attribute.name()),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.94, 0.90, 0.74)),
                    Node {
                        width: Val::Px(90.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                main.spawn((
                    Text::new("1"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                    AttributeValueText(attribute),
                ));
                main.spawn((
                    Button,
                    Node {
                        width: Val::Px(48.0),
                        height: Val::Px(24.0),
                        margin: UiRect::left(Val::Auto),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.48, 0.28)),
                    BorderColor::all(Color::srgb(0.48, 0.88, 0.58)),
                    AttributeIncreaseButton(attribute),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("+ 2"),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                        AttributeCostText(attribute),
                    ));
                });
            });
            row.spawn((
                Text::new(attribute.description()),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.59, 0.64, 0.72)),
                Pickable::IGNORE,
            ));
        });
}

fn toggle_character_status(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CharacterStatusUiState>,
    mut panels: Query<&mut Visibility, With<CharacterStatusRoot>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyC) {
        return;
    }
    state.open = !state.open;
    for mut visibility in &mut panels {
        *visibility = if state.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn spend_attribute_point(
    interactions: Query<
        (&Interaction, &AttributeIncreaseButton),
        (Changed<Interaction>, With<Button>),
    >,
    player: Query<&CharacterStats, With<ControlledPlayer>>,
    mut commands: MessageWriter<PlayerCommand>,
) {
    let Ok(stats) = player.single() else {
        return;
    };
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed && stats.can_spend_point(button.0).is_ok() {
            commands.write(PlayerCommand::SpendAttributePoint {
                attribute: button.0,
            });
        }
    }
}

fn update_character_status(
    player: Query<
        (
            &CharacterStats,
            &BaseProgression,
            &Equipment,
            &Health,
            &Mana,
        ),
        With<ControlledPlayer>,
    >,
    mut texts: Query<(
        Option<&AvailableAttributePointsText>,
        Option<&AttributeValueText>,
        Option<&AttributeCostText>,
        Option<&DerivedStatsText>,
        &mut Text,
    )>,
    mut buttons: Query<(&AttributeIncreaseButton, &mut BackgroundColor)>,
) {
    let Ok((stats, progression, equipment, health, mana)) = player.single() else {
        return;
    };
    let bonuses = equipment_bonuses(equipment);
    let derived = equipment_derived_stats(stats, progression.level, equipment);

    for (points, attribute, cost, derived_text, mut text) in &mut texts {
        if points.is_some() {
            text.0 = format!("Points: {}", stats.available_points);
        } else if let Some(attribute) = attribute {
            let bonus = bonuses.attribute(attribute.0);
            text.0 = if bonus > 0 {
                format!("{} + {bonus}", stats.value(attribute.0))
            } else {
                stats.value(attribute.0).to_string()
            };
        } else if let Some(cost) = cost {
            text.0 = stats
                .next_attribute_cost(cost.0)
                .map_or_else(|| "MAX".to_string(), |cost| format!("+ {cost}"));
        } else if derived_text.is_some() {
            text.0 = format!(
                "Derived Stats\nHIT: {}    FLEE: {}\nHP: {}/{}    SP: {}/{}\nATK: {}    Magic: {}\nDEF: {}    MDEF: {}",
                derived.hit,
                derived.flee,
                health.current,
                derived.max_health,
                mana.current,
                derived.max_mana,
                derived.physical_attack,
                derived.magic_power,
                derived.physical_defense,
                derived.magic_defense,
            );
        }
    }

    for (button, mut background) in &mut buttons {
        background.0 = if stats.can_spend_point(button.0).is_ok() {
            Color::srgb(0.18, 0.48, 0.28)
        } else if stats.value(button.0) >= MAX_ATTRIBUTE_VALUE {
            Color::srgb(0.42, 0.30, 0.12)
        } else {
            Color::srgb(0.20, 0.21, 0.24)
        };
    }
}

fn despawn_character_status(
    panels: Query<Entity, With<CharacterStatusRoot>>,
    mut state: ResMut<CharacterStatusUiState>,
    mut commands: Commands,
) {
    for panel in &panels {
        commands.entity(panel).try_despawn();
    }
    state.open = false;
}
