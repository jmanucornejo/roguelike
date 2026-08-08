use bevy::{app::AppExit, prelude::*};

use crate::{
    client::{presentation::ui_drag::DraggableUi, state::ControlledPlayer},
    shared::{gameplay::components::Dead, network::messages::PlayerCommand, states::ClientState},
};

pub(crate) const DEATH_SCREEN_Z_INDEX: i32 = 2000;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeathPenalty(pub u64);

#[derive(Component)]
struct DeathScreenRoot;

#[derive(Component)]
struct DeathPenaltyText;

#[derive(Component, Clone, Copy)]
enum DeathButtonAction {
    Respawn,
    Quit,
}

pub(crate) struct DeathScreenPlugin;

impl Plugin for DeathScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ClientState::InGame), spawn_death_screen)
            .add_systems(
                Update,
                (update_death_screen, handle_death_buttons).run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_death_screen);
    }
}

fn spawn_death_screen(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.0, 0.0, 0.72)),
            GlobalZIndex(DEATH_SCREEN_Z_INDEX),
            Visibility::Hidden,
            DeathScreenRoot,
            Name::new("Death Screen"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(360.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(7.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.035, 0.04, 0.98)),
                BorderColor::all(Color::srgb(0.72, 0.12, 0.12)),
                GlobalZIndex(DEATH_SCREEN_Z_INDEX),
                DraggableUi::header(58.0),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("You Have Died"),
                    TextFont {
                        font_size: FontSize::Px(30.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.18, 0.16)),
                ));
                panel.spawn((
                    Text::new("Base EXP lost: 0"),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.78, 0.72)),
                    DeathPenaltyText,
                ));
                spawn_button(panel, "Return to Save Point", DeathButtonAction::Respawn);
                spawn_button(panel, "Quit Game", DeathButtonAction::Quit);
            });
        });
}

fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, action: DeathButtonAction) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(260.0),
                height: Val::Px(42.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.08, 0.08)),
            BorderColor::all(Color::srgb(0.68, 0.28, 0.24)),
            action,
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
}

fn update_death_screen(
    player: Query<Option<&DeathPenalty>, (With<ControlledPlayer>, With<Dead>)>,
    mut roots: Query<&mut Visibility, With<DeathScreenRoot>>,
    mut penalty_text: Query<&mut Text, With<DeathPenaltyText>>,
) {
    let penalty = player
        .single()
        .ok()
        .flatten()
        .map_or(0, |penalty| penalty.0);
    let dead = !player.is_empty();
    for mut visibility in &mut roots {
        *visibility = if dead {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut text in &mut penalty_text {
        **text = format!("Base EXP lost: {penalty}");
    }
}

fn handle_death_buttons(
    interactions: Query<(&Interaction, &DeathButtonAction), (Changed<Interaction>, With<Button>)>,
    mut player_commands: MessageWriter<PlayerCommand>,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            DeathButtonAction::Respawn => {
                player_commands.write(PlayerCommand::RespawnAtSavePoint);
            }
            DeathButtonAction::Quit => {
                app_exit.write(AppExit::Success);
            }
        }
    }
}

fn despawn_death_screen(roots: Query<Entity, With<DeathScreenRoot>>, mut commands: Commands) {
    for root in &roots {
        commands.entity(root).try_despawn();
    }
}
