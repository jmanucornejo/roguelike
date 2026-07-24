use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};

use crate::shared::states::ClientState;

const ACTION_BAR_SLOT_COUNT: usize = 10;
const ACTION_BAR_SLOT_SIZE: f32 = 30.0;
const ACTION_BAR_SLOT_GAP: f32 = 1.0;
const ACTION_BAR_HANDLE_WIDTH: f32 = 14.0;
const ACTION_BAR_PADDING: f32 = 4.0;
const ACTION_BAR_HEIGHT: f32 = 38.0;
const ACTION_BAR_BOTTOM_MARGIN: f32 = 24.0;
const ACTION_BAR_WIDTH: f32 = ACTION_BAR_PADDING * 2.0
    + ACTION_BAR_HANDLE_WIDTH
    + ACTION_BAR_SLOT_SIZE * ACTION_BAR_SLOT_COUNT as f32
    + ACTION_BAR_SLOT_GAP * ACTION_BAR_SLOT_COUNT as f32;

#[derive(Component)]
struct ActionBarRoot;

#[derive(Resource, Debug)]
pub(crate) struct ActionBarState {
    position: Vec2,
    drag_offset: Vec2,
    dragging: bool,
    initialized: bool,
}

impl Default for ActionBarState {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            drag_offset: Vec2::ZERO,
            dragging: false,
            initialized: false,
        }
    }
}

impl ActionBarState {
    pub(crate) fn captures_pointer(&self, pointer_position: Vec2) -> bool {
        self.initialized
            && pointer_position.x >= self.position.x
            && pointer_position.x <= self.position.x + ACTION_BAR_WIDTH
            && pointer_position.y >= self.position.y
            && pointer_position.y <= self.position.y + ACTION_BAR_HEIGHT
    }

    fn drag_handle_contains(&self, pointer_position: Vec2) -> bool {
        self.captures_pointer(pointer_position)
            && pointer_position.x <= self.position.x + ACTION_BAR_PADDING + ACTION_BAR_HANDLE_WIDTH
    }
}

pub(crate) struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionBarState>()
            .add_systems(OnEnter(ClientState::InGame), spawn_action_bar)
            .add_systems(
                Update,
                drag_action_bar.run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_action_bar);
    }
}

fn spawn_action_bar(
    mut commands: Commands,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<ActionBarState>,
) {
    let Ok(window) = primary_window.single() else {
        return;
    };

    if !state.initialized {
        state.position = initial_action_bar_position(Vec2::new(window.width(), window.height()));
        state.initialized = true;
    }
    state.dragging = false;

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(state.position.x),
                top: Val::Px(state.position.y),
                width: Val::Px(ACTION_BAR_WIDTH),
                height: Val::Px(ACTION_BAR_HEIGHT),
                padding: UiRect::all(Val::Px(ACTION_BAR_PADDING)),
                column_gap: Val::Px(ACTION_BAR_SLOT_GAP),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.055, 0.065, 0.085, 0.94)),
            BorderColor::all(Color::srgba(0.64, 0.68, 0.76, 0.95)),
            GlobalZIndex(300),
            Pickable::IGNORE,
            ActionBarRoot,
            Name::new("Action Bar"),
        ))
        .with_children(|bar| {
            spawn_drag_handle(bar);

            for slot_index in 0..ACTION_BAR_SLOT_COUNT {
                spawn_action_slot(bar, slot_index);
            }
        });
}

fn spawn_drag_handle(bar: &mut ChildSpawnerCommands) {
    bar.spawn((
        Node {
            width: Val::Px(ACTION_BAR_HANDLE_WIDTH),
            height: Val::Px(ACTION_BAR_SLOT_SIZE),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.18, 0.23)),
        BorderColor::all(Color::srgb(0.32, 0.35, 0.43)),
        Pickable::IGNORE,
        Name::new("Action Bar Drag Handle"),
    ))
    .with_children(|handle| {
        for _ in 0..3 {
            handle.spawn((
                Node {
                    width: Val::Px(7.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.72, 0.75, 0.82)),
                Pickable::IGNORE,
            ));
        }
    });
}

fn spawn_action_slot(bar: &mut ChildSpawnerCommands, slot_index: usize) {
    let placeholder_spell = match slot_index {
        0 => Some((
            Color::srgb(0.18, 0.38, 0.92),
            Color::srgb(0.48, 0.76, 1.0),
            Color::srgb(0.86, 0.96, 1.0),
            "F1",
        )),
        1 => Some((
            Color::srgb(0.56, 0.19, 0.84),
            Color::srgb(0.82, 0.52, 1.0),
            Color::srgb(0.98, 0.88, 1.0),
            "F2",
        )),
        2 => Some((
            Color::srgb(0.91, 0.31, 0.08),
            Color::srgb(1.0, 0.72, 0.30),
            Color::srgb(1.0, 0.97, 0.76),
            "F3",
        )),
        _ => None,
    };
    let occupied = placeholder_spell.is_some();
    let background = if occupied {
        Color::srgb(0.105, 0.12, 0.23)
    } else {
        Color::srgba(0.025, 0.03, 0.045, 0.88)
    };

    bar.spawn((
        Node {
            width: Val::Px(ACTION_BAR_SLOT_SIZE),
            height: Val::Px(ACTION_BAR_SLOT_SIZE),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(background),
        BorderColor::all(Color::srgb(0.43, 0.46, 0.53)),
        Pickable::IGNORE,
        Name::new(format!("Action Slot {}", slot_index + 1)),
    ))
    .with_children(|slot| {
        let Some((orb_color, border_color, glow_color, hotkey)) = placeholder_spell else {
            return;
        };

        // A small glowing orb stands in for a real spell icon until the spell
        // inventory supplies textures.
        slot.spawn((
            Node {
                width: Val::Px(19.0),
                height: Val::Px(19.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(orb_color),
            BorderColor::all(border_color),
            Pickable::IGNORE,
        ))
        .with_child((
            Node {
                width: Val::Px(6.0),
                height: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(glow_color),
            Pickable::IGNORE,
        ));

        slot.spawn((
            Text::new(hotkey),
            TextFont {
                font_size: FontSize::Px(8.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::new(1.0, 1.0),
                color: Color::BLACK,
            },
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(1.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
    });
}

fn drag_action_bar(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<ActionBarState>,
    mut bar: Query<&mut Node, With<ActionBarRoot>>,
) {
    let Ok(window) = primary_window.single() else {
        return;
    };

    if mouse_buttons.just_released(MouseButton::Left) {
        state.dragging = false;
    }

    let Some(pointer_position) = window.cursor_position() else {
        return;
    };

    if mouse_buttons.just_pressed(MouseButton::Left) && state.drag_handle_contains(pointer_position)
    {
        state.dragging = true;
        state.drag_offset = pointer_position - state.position;
    }

    if !state.dragging || !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }

    let max_position = Vec2::new(
        (window.width() - ACTION_BAR_WIDTH).max(0.0),
        (window.height() - ACTION_BAR_HEIGHT).max(0.0),
    );
    state.position = (pointer_position - state.drag_offset).clamp(Vec2::ZERO, max_position);

    if let Ok(mut bar) = bar.single_mut() {
        bar.left = Val::Px(state.position.x);
        bar.top = Val::Px(state.position.y);
    }
}

fn despawn_action_bar(
    mut commands: Commands,
    bars: Query<Entity, With<ActionBarRoot>>,
    mut state: ResMut<ActionBarState>,
) {
    state.dragging = false;
    for entity in &bars {
        commands.entity(entity).try_despawn();
    }
}

fn initial_action_bar_position(window_size: Vec2) -> Vec2 {
    Vec2::new(
        ((window_size.x - ACTION_BAR_WIDTH) * 0.5).max(0.0),
        (window_size.y - ACTION_BAR_HEIGHT - ACTION_BAR_BOTTOM_MARGIN).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_position_is_centered_near_the_bottom() {
        let position = initial_action_bar_position(Vec2::new(720.0, 720.0));

        assert!((position.x - (720.0 - ACTION_BAR_WIDTH) * 0.5).abs() < f32::EPSILON);
        assert_eq!(
            position.y,
            720.0 - ACTION_BAR_HEIGHT - ACTION_BAR_BOTTOM_MARGIN
        );
    }

    #[test]
    fn action_bar_captures_pointer_inside_its_bounds() {
        let state = ActionBarState {
            position: Vec2::new(100.0, 200.0),
            initialized: true,
            ..default()
        };

        assert!(state.captures_pointer(Vec2::new(101.0, 201.0)));
        assert!(!state.captures_pointer(Vec2::new(99.0, 201.0)));
        assert!(state.drag_handle_contains(Vec2::new(100.0 + ACTION_BAR_PADDING, 201.0)));
    }
}
