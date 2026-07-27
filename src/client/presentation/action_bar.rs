use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};

use crate::{
    client::{presentation::inventory::item_color, state::ControlledPlayer},
    shared::{
        gameplay::items::{Inventory, ItemDefinitionId},
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

pub(crate) const ACTION_BAR_SLOT_COUNT: usize = 10;
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

#[derive(Component)]
struct ActionItemIcon(usize);

#[derive(Component)]
struct ActionItemQuantity(usize);

#[derive(Resource, Debug, Default)]
pub(crate) struct ActionBarBindings {
    items: [Option<ItemDefinitionId>; ACTION_BAR_SLOT_COUNT],
}

impl ActionBarBindings {
    pub(crate) fn bind_item(&mut self, slot_index: usize, item_id: ItemDefinitionId) -> bool {
        // F1-F3 remain spell slots. Consumables can be assigned to F4-F10.
        if !(3..ACTION_BAR_SLOT_COUNT).contains(&slot_index) {
            return false;
        }
        self.items[slot_index] = Some(item_id);
        true
    }

    fn item(&self, slot_index: usize) -> Option<ItemDefinitionId> {
        self.items.get(slot_index).copied().flatten()
    }
}

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

    pub(crate) fn slot_at(&self, pointer_position: Vec2) -> Option<usize> {
        if !self.initialized {
            return None;
        }

        let slots_left =
            self.position.x + ACTION_BAR_PADDING + ACTION_BAR_HANDLE_WIDTH + ACTION_BAR_SLOT_GAP;
        let slots_top = self.position.y + ACTION_BAR_PADDING;
        let relative = pointer_position - Vec2::new(slots_left, slots_top);
        if relative.x < 0.0 || relative.y < 0.0 || relative.y > ACTION_BAR_SLOT_SIZE {
            return None;
        }

        let stride = ACTION_BAR_SLOT_SIZE + ACTION_BAR_SLOT_GAP;
        let slot_index = (relative.x / stride).floor() as usize;
        if slot_index >= ACTION_BAR_SLOT_COUNT
            || relative.x - slot_index as f32 * stride > ACTION_BAR_SLOT_SIZE
        {
            return None;
        }
        Some(slot_index)
    }
}

pub(crate) struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionBarState>()
            .init_resource::<ActionBarBindings>()
            .add_systems(OnEnter(ClientState::InGame), spawn_action_bar)
            .add_systems(
                Update,
                (drag_action_bar, update_item_slots, use_bound_item)
                    .run_if(in_state(ClientState::InGame)),
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
        if let Some((orb_color, border_color, glow_color, hotkey)) = placeholder_spell {
            // A small glowing orb stands in for a real spell icon until the
            // spell inventory supplies textures.
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

            spawn_hotkey_label(slot, hotkey);
        } else {
            slot.spawn((
                Node {
                    width: Val::Px(19.0),
                    height: Val::Px(19.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                BorderColor::all(Color::srgb(0.92, 0.92, 0.95)),
                Visibility::Hidden,
                Pickable::IGNORE,
                ActionItemIcon(slot_index),
            ));
            slot.spawn((
                Text::new(""),
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
                    left: Val::Px(1.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                Pickable::IGNORE,
                ActionItemQuantity(slot_index),
            ));
            spawn_hotkey_label(slot, &format!("F{}", slot_index + 1));
        }
    });
}

fn spawn_hotkey_label(slot: &mut ChildSpawnerCommands, hotkey: &str) {
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
}

fn update_item_slots(
    bindings: Res<ActionBarBindings>,
    inventory: Query<&Inventory, With<ControlledPlayer>>,
    mut icons: Query<(&ActionItemIcon, &mut Visibility, &mut BackgroundColor)>,
    mut quantities: Query<(&ActionItemQuantity, &mut Text)>,
) {
    let inventory = inventory.single().ok();

    for (icon, mut visibility, mut color) in &mut icons {
        if let Some(item_id) = bindings.item(icon.0) {
            *visibility = Visibility::Inherited;
            color.0 = item_color(item_id);
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    for (quantity, mut text) in &mut quantities {
        text.0 = bindings
            .item(quantity.0)
            .map(|item_id| {
                inventory
                    .map(|inventory| inventory.quantity(item_id))
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_default();
    }
}

fn use_bound_item(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<ActionBarBindings>,
    mut player_commands: MessageWriter<PlayerCommand>,
) {
    let keys = [
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
    ];

    for (offset, key) in keys.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            if let Some(item_id) = bindings.item(offset + 3) {
                player_commands.write(PlayerCommand::UseItem { item_id });
            }
        }
    }
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

    #[test]
    fn slot_hit_testing_maps_the_fourth_slot_to_f4() {
        let state = ActionBarState {
            position: Vec2::new(100.0, 200.0),
            initialized: true,
            ..default()
        };
        let first_slot_left =
            100.0 + ACTION_BAR_PADDING + ACTION_BAR_HANDLE_WIDTH + ACTION_BAR_SLOT_GAP;
        let f4_center = Vec2::new(
            first_slot_left
                + 3.0 * (ACTION_BAR_SLOT_SIZE + ACTION_BAR_SLOT_GAP)
                + ACTION_BAR_SLOT_SIZE * 0.5,
            200.0 + ACTION_BAR_PADDING + ACTION_BAR_SLOT_SIZE * 0.5,
        );

        assert_eq!(state.slot_at(f4_center), Some(3));
    }

    #[test]
    fn consumables_cannot_replace_the_three_spell_slots() {
        let mut bindings = ActionBarBindings::default();

        assert!(!bindings.bind_item(0, crate::shared::gameplay::items::RED_HERB));
        assert!(bindings.bind_item(3, crate::shared::gameplay::items::RED_HERB));
        assert_eq!(
            bindings.item(3),
            Some(crate::shared::gameplay::items::RED_HERB)
        );
    }
}
