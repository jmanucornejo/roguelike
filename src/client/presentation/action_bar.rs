use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};

use crate::{
    client::{presentation::inventory::item_color, state::ControlledPlayer},
    shared::{
        gameplay::action_bar::{ActionBarBinding, ActionBarLayout, ACTION_BAR_SLOT_COUNT},
        gameplay::items::{Inventory, ItemDefinitionId},
        gameplay::progression::CharacterClass,
        gameplay::skills::{skill_definition, SkillId, SkillTree},
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

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

#[derive(Component)]
struct ActionBarDragGhost;

#[derive(Resource, Debug, Default)]
struct ActionBarSlotDrag {
    source_slot: Option<usize>,
    ghost: Option<Entity>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct ActionBarBindings {
    layout: ActionBarLayout,
}

impl ActionBarBindings {
    pub(crate) fn bind_item(&mut self, slot_index: usize, item_id: ItemDefinitionId) -> bool {
        self.layout
            .set(slot_index, Some(ActionBarBinding::Item(item_id)))
    }

    pub(crate) fn bind_skill(&mut self, slot_index: usize, skill_id: SkillId) -> bool {
        self.layout
            .set(slot_index, Some(ActionBarBinding::Skill(skill_id)))
    }

    pub(crate) fn binding(&self, slot_index: usize) -> Option<ActionBarBinding> {
        self.layout.binding(slot_index)
    }

    pub(crate) fn replace(&mut self, layout: ActionBarLayout) {
        self.layout = layout;
    }

    fn swap(&mut self, first_slot: usize, second_slot: usize) -> bool {
        self.layout.swap(first_slot, second_slot)
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
            .init_resource::<ActionBarSlotDrag>()
            .add_systems(OnEnter(ClientState::InGame), spawn_action_bar)
            .add_systems(
                Update,
                (
                    drag_action_bar,
                    drag_action_bar_slot,
                    update_slots,
                    activate_bound_item,
                )
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
    bar.spawn((
        Node {
            width: Val::Px(ACTION_BAR_SLOT_SIZE),
            height: Val::Px(ACTION_BAR_SLOT_SIZE),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.03, 0.045, 0.88)),
        BorderColor::all(Color::srgb(0.43, 0.46, 0.53)),
        Pickable::IGNORE,
        Name::new(format!("Action Slot {}", slot_index + 1)),
    ))
    .with_children(|slot| {
        slot.spawn((
            Node {
                width: Val::Px(19.0),
                height: Val::Px(19.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
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

fn update_slots(
    bindings: Res<ActionBarBindings>,
    inventory: Query<&Inventory, With<ControlledPlayer>>,
    skill_tree: Query<&SkillTree, With<ControlledPlayer>>,
    mut icons: Query<(&ActionItemIcon, &mut Visibility, &mut BackgroundColor)>,
    mut quantities: Query<(&ActionItemQuantity, &mut Text)>,
) {
    let inventory = inventory.single().ok();
    let skill_tree = skill_tree.single().ok();

    for (icon, mut visibility, mut color) in &mut icons {
        if let Some(binding) = bindings.binding(icon.0) {
            *visibility = Visibility::Inherited;
            color.0 = binding_color(binding);
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    for (quantity, mut text) in &mut quantities {
        text.0 = match bindings.binding(quantity.0) {
            Some(ActionBarBinding::Item(item_id)) => inventory
                .map(|inventory| inventory.quantity(item_id))
                .unwrap_or_default()
                .to_string(),
            Some(ActionBarBinding::Skill(skill_id)) => skill_tree
                .map(|skill_tree| format!("L{}", skill_tree.rank(skill_id)))
                .unwrap_or_default(),
            _ => String::new(),
        };
    }
}

fn binding_color(binding: ActionBarBinding) -> Color {
    match binding {
        ActionBarBinding::Spell(1) => Color::srgb(0.18, 0.38, 0.92),
        ActionBarBinding::Spell(2) => Color::srgb(0.56, 0.19, 0.84),
        ActionBarBinding::Spell(3) => Color::srgb(0.91, 0.31, 0.08),
        ActionBarBinding::Spell(4) => Color::srgb(0.08, 0.62, 0.34),
        ActionBarBinding::Spell(_) => Color::srgb(0.45, 0.45, 0.65),
        ActionBarBinding::Item(item_id) => item_color(item_id),
        ActionBarBinding::Skill(skill_id) => skill_definition(skill_id)
            .map(|definition| match definition.class {
                CharacterClass::Novice => Color::srgb(0.55, 0.62, 0.72),
                CharacterClass::Swordsman => Color::srgb(0.78, 0.22, 0.18),
                CharacterClass::Mage => Color::srgb(0.32, 0.28, 0.88),
                CharacterClass::Archer => Color::srgb(0.22, 0.66, 0.30),
                CharacterClass::Acolyte => Color::srgb(0.90, 0.76, 0.30),
                CharacterClass::Merchant => Color::srgb(0.82, 0.48, 0.16),
                CharacterClass::Thief => Color::srgb(0.52, 0.22, 0.66),
                _ => Color::srgb(0.48, 0.42, 0.34),
            })
            .unwrap_or(Color::srgb(0.42, 0.42, 0.48)),
    }
}

pub(crate) fn pressed_action_bar_slot(keyboard: &ButtonInput<KeyCode>) -> Option<usize> {
    let keys = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
    ];
    keys.into_iter().position(|key| keyboard.just_pressed(key))
}

fn activate_bound_item(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<ActionBarBindings>,
    mut player_commands: MessageWriter<PlayerCommand>,
) {
    let Some(slot_index) = pressed_action_bar_slot(&keyboard) else {
        return;
    };
    if let Some(ActionBarBinding::Item(item_id)) = bindings.binding(slot_index) {
        player_commands.write(PlayerCommand::UseItem { item_id });
    }
}

fn drag_action_bar_slot(
    mut commands: Commands,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    state: Res<ActionBarState>,
    mut bindings: ResMut<ActionBarBindings>,
    mut drag: ResMut<ActionBarSlotDrag>,
    mut ghosts: Query<&mut Node, With<ActionBarDragGhost>>,
    mut player_commands: MessageWriter<PlayerCommand>,
) {
    let Ok(window) = primary_window.single() else {
        return;
    };
    let pointer = window.cursor_position();

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(pointer) = pointer {
            if let Some(source_slot) = state.slot_at(pointer) {
                if let Some(binding) = bindings.binding(source_slot) {
                    let ghost = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(pointer.x - 11.0),
                                top: Val::Px(pointer.y - 11.0),
                                width: Val::Px(22.0),
                                height: Val::Px(22.0),
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(binding_color(binding)),
                            BorderColor::all(Color::WHITE),
                            GlobalZIndex(1002),
                            Pickable::IGNORE,
                            ActionBarDragGhost,
                            Name::new("Dragged action bar binding"),
                        ))
                        .id();
                    drag.source_slot = Some(source_slot);
                    drag.ghost = Some(ghost);
                }
            }
        }
    }

    if let (Some(pointer), Some(ghost)) = (pointer, drag.ghost) {
        if let Ok(mut node) = ghosts.get_mut(ghost) {
            node.left = Val::Px(pointer.x - 11.0);
            node.top = Val::Px(pointer.y - 11.0);
        }
    }

    if !mouse_buttons.just_released(MouseButton::Left) {
        return;
    }

    let source_slot = drag.source_slot.take();
    let target_slot = pointer.and_then(|pointer| state.slot_at(pointer));
    if let (Some(source_slot), Some(target_slot)) = (source_slot, target_slot) {
        if source_slot != target_slot && bindings.swap(source_slot, target_slot) {
            player_commands.write(PlayerCommand::SwapActionBarSlots {
                first_slot: source_slot as u8,
                second_slot: target_slot as u8,
            });
        }
    }

    if let Some(ghost) = drag.ghost.take() {
        commands.entity(ghost).try_despawn();
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
    ghosts: Query<Entity, With<ActionBarDragGhost>>,
    mut state: ResMut<ActionBarState>,
    mut drag: ResMut<ActionBarSlotDrag>,
) {
    state.dragging = false;
    drag.source_slot = None;
    drag.ghost = None;
    for entity in bars.iter().chain(ghosts.iter()) {
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
    fn consumables_can_replace_spell_slots_including_f4() {
        let mut bindings = ActionBarBindings::default();

        assert!(bindings.bind_item(0, crate::shared::gameplay::items::RED_HERB));
        assert!(bindings.bind_item(3, crate::shared::gameplay::items::RED_HERB));
        assert!(bindings.bind_item(4, crate::shared::gameplay::items::RED_HERB));
        assert_eq!(
            bindings.binding(3),
            Some(ActionBarBinding::Item(
                crate::shared::gameplay::items::RED_HERB
            ))
        );
    }

    #[test]
    fn dragging_a_spell_to_an_item_slot_swaps_both_bindings() {
        let mut bindings = ActionBarBindings::default();
        bindings.bind_item(9, crate::shared::gameplay::items::RED_HERB);

        assert!(bindings.swap(0, 9));
        assert_eq!(
            bindings.binding(0),
            Some(ActionBarBinding::Item(
                crate::shared::gameplay::items::RED_HERB
            ))
        );
        assert_eq!(bindings.binding(9), Some(ActionBarBinding::Spell(1)));
    }

    #[test]
    fn learned_skills_can_replace_any_action_bar_slot() {
        let mut bindings = ActionBarBindings::default();

        assert!(bindings.bind_skill(3, SkillId(301)));
        assert_eq!(
            bindings.binding(3),
            Some(ActionBarBinding::Skill(SkillId(301)))
        );
    }
}
