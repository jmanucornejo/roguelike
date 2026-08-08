use bevy::{
    prelude::*,
    window::{CursorOptions, PrimaryWindow},
};
use bevy_renet::{RenetClient, RenetReceive};

use crate::shared::states::ClientState;

const CONNECTION_LOST_Z_INDEX: i32 = 10_000;
const TEXT_COLOR: Color = Color::srgb(0.95, 0.92, 0.77);
const MUTED_TEXT_COLOR: Color = Color::srgb(0.67, 0.71, 0.78);
const PANEL_BACKGROUND: Color = Color::srgba(0.035, 0.042, 0.065, 0.99);
const NORMAL_BUTTON: Color = Color::srgb(0.14, 0.18, 0.26);
const HOVERED_BUTTON: Color = Color::srgb(0.2, 0.29, 0.43);
const PRESSED_BUTTON: Color = Color::srgb(0.14, 0.38, 0.23);

#[derive(Resource, Default)]
struct ConnectionLostDetails {
    reason: String,
}

#[derive(Component)]
struct ConnectionLostRoot;

#[derive(Component)]
struct ReturnToMenuButton;

pub(crate) struct ConnectionLostPlugin;

impl Plugin for ConnectionLostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectionLostDetails>()
            .add_systems(
                Update,
                detect_lost_connection
                    .run_if(in_state(ClientState::InGame))
                    .after(RenetReceive),
            )
            .add_systems(
                OnEnter(ClientState::ConnectionLost),
                (show_system_cursor, spawn_connection_lost_dialog),
            )
            .add_systems(
                Update,
                (handle_return_to_menu, update_button_color)
                    .run_if(in_state(ClientState::ConnectionLost)),
            )
            .add_systems(
                OnExit(ClientState::ConnectionLost),
                despawn_connection_lost_dialog,
            );
    }
}

fn detect_lost_connection(
    client: Option<Res<RenetClient>>,
    mut details: ResMut<ConnectionLostDetails>,
    mut next_state: ResMut<NextState<ClientState>>,
) {
    let reason = match client {
        Some(client) if !client.is_disconnected() => return,
        Some(client) => client
            .disconnect_reason()
            .map(|reason| reason.to_string())
            .unwrap_or_else(|| "the server connection ended unexpectedly".to_string()),
        None => "the client connection is no longer available".to_string(),
    };

    warn!("Connection lost: {reason}");
    details.reason = reason;
    next_state.set(ClientState::ConnectionLost);
}

fn show_system_cursor(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = windows.single_mut() {
        cursor.visible = true;
    }
}

fn spawn_connection_lost_dialog(mut commands: Commands, details: Res<ConnectionLostDetails>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                top: Val::ZERO,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.015, 0.025, 0.82)),
            GlobalZIndex(CONNECTION_LOST_Z_INDEX),
            ConnectionLostRoot,
            Name::new("Connection Lost Screen"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(430.0),
                    padding: UiRect::all(Val::Px(28.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(7.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BACKGROUND),
                BorderColor::all(Color::srgb(0.58, 0.18, 0.16)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Connection Lost"),
                    TextFont {
                        font_size: FontSize::Px(32.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.28, 0.22)),
                ));
                panel.spawn((
                    Text::new("The connection to the server was interrupted."),
                    TextFont {
                        font_size: FontSize::Px(17.0),
                        ..default()
                    },
                    TextColor(TEXT_COLOR),
                ));
                panel.spawn((
                    Text::new(format!("Reason: {}", details.reason)),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(MUTED_TEXT_COLOR),
                ));
                panel
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(290.0),
                            height: Val::Px(48.0),
                            margin: UiRect::top(Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(NORMAL_BUTTON),
                        BorderColor::all(Color::srgb(0.42, 0.56, 0.76)),
                        ReturnToMenuButton,
                    ))
                    .with_child((
                        Text::new("Return to Main Menu"),
                        TextFont {
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(TEXT_COLOR),
                    ));
            });
        });
}

fn handle_return_to_menu(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ReturnToMenuButton>)>,
    mut next_state: ResMut<NextState<ClientState>>,
) {
    if interactions
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        next_state.set(ClientState::InMenu);
    }
}

fn update_button_color(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ReturnToMenuButton>),
    >,
) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed => PRESSED_BUTTON.into(),
            Interaction::Hovered => HOVERED_BUTTON.into(),
            Interaction::None => NORMAL_BUTTON.into(),
        };
    }
}

fn despawn_connection_lost_dialog(
    mut commands: Commands,
    roots: Query<Entity, With<ConnectionLostRoot>>,
) {
    for root in &roots {
        commands.entity(root).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_game_connection_opens_the_connection_lost_dialog() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<ClientState>()
            .add_plugins(ConnectionLostPlugin);
        app.world_mut()
            .resource_mut::<NextState<ClientState>>()
            .set(ClientState::InGame);

        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<ClientState>>().get(),
            ClientState::ConnectionLost
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ConnectionLostRoot>>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
