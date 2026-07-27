use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};
use std::f32::consts::TAU;

use crate::{
    client::{
        presentation::action_bar::{ActionBarBindings, ActionBarState},
        state::ControlledPlayer,
    },
    shared::{
        gameplay::items::{
            item_definition, Inventory, ItemDefinitionId, ITEM_DEFINITIONS, LUCKY_CLOVER, PIG_MEAT,
            RED_HERB,
        },
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

const INVENTORY_PANEL_WIDTH: f32 = 188.0;
const INVENTORY_PANEL_MAX_HEIGHT: f32 = 120.0;
const INVENTORY_PANEL_MARGIN: f32 = 12.0;
const DOUBLE_CLICK_SECONDS: f64 = 0.35;
const GROUND_ITEM_ARC_HEIGHT: f32 = 1.5;
const GROUND_ITEM_ARC_SECONDS: f32 = 0.7;

#[derive(Component)]
struct InventoryPanelRoot;

#[derive(Component)]
struct InventoryRow(ItemDefinitionId);

#[derive(Component)]
struct InventoryQuantityText(ItemDefinitionId);

#[derive(Component)]
struct InventoryDragGhost;

#[derive(Component, Debug)]
pub(crate) struct GroundItemDropAnimation {
    landing: Vec3,
    elapsed: f32,
}

pub(crate) fn falling_ground_item(landing: Vec3) -> (Transform, GroundItemDropAnimation) {
    (
        Transform::from_translation(landing),
        GroundItemDropAnimation {
            landing,
            elapsed: 0.0,
        },
    )
}

fn ground_item_arc_height(progress: f32) -> f32 {
    4.0 * GROUND_ITEM_ARC_HEIGHT * progress * (1.0 - progress)
}

#[derive(Resource, Debug, Default)]
pub(crate) struct InventoryUiState {
    dragging: Option<ItemDefinitionId>,
    drag_ghost: Option<Entity>,
    last_click: Option<(ItemDefinitionId, f64)>,
}

impl InventoryUiState {
    pub(crate) fn captures_pointer(&self, pointer_position: Vec2, window_size: Vec2) -> bool {
        pointer_position.x >= window_size.x - INVENTORY_PANEL_MARGIN - INVENTORY_PANEL_WIDTH
            && pointer_position.x <= window_size.x - INVENTORY_PANEL_MARGIN
            && pointer_position.y >= INVENTORY_PANEL_MARGIN
            && pointer_position.y <= INVENTORY_PANEL_MARGIN + INVENTORY_PANEL_MAX_HEIGHT
    }
}

pub(crate) struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUiState>()
            .add_systems(OnEnter(ClientState::InGame), spawn_inventory_panel)
            .add_systems(
                Update,
                (
                    inventory_item_interactions,
                    update_inventory_drag.after(inventory_item_interactions),
                    update_inventory_panel,
                    animate_ground_item_drops,
                )
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_inventory_panel);
    }
}

fn animate_ground_item_drops(
    time: Res<Time>,
    mut commands: Commands,
    mut drops: Query<(Entity, &mut Transform, &mut GroundItemDropAnimation)>,
) {
    for (entity, mut transform, mut animation) in &mut drops {
        animation.elapsed += time.delta_secs();
        let progress = (animation.elapsed / GROUND_ITEM_ARC_SECONDS).clamp(0.0, 1.0);
        let height = ground_item_arc_height(progress);
        transform.translation = animation.landing + Vec3::Y * height;
        transform.rotation = Quat::from_rotation_y(progress * TAU)
            * Quat::from_rotation_x(progress * std::f32::consts::PI * 0.35);

        if progress >= 1.0 {
            transform.translation = animation.landing;
            transform.rotation = Quat::IDENTITY;
            commands.entity(entity).remove::<GroundItemDropAnimation>();
        }
    }
}

pub(crate) fn item_color(item_id: ItemDefinitionId) -> Color {
    match item_id {
        PIG_MEAT => Color::srgb(0.86, 0.38, 0.34),
        RED_HERB => Color::srgb(0.82, 0.12, 0.16),
        LUCKY_CLOVER => Color::srgb(0.18, 0.75, 0.25),
        _ => Color::srgb(0.82, 0.82, 0.86),
    }
}

fn spawn_inventory_panel(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(INVENTORY_PANEL_WIDTH),
                padding: UiRect::all(Val::Px(7.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.045, 0.052, 0.07, 0.94)),
            BorderColor::all(Color::srgba(0.60, 0.64, 0.72, 0.96)),
            GlobalZIndex(295),
            Pickable::IGNORE,
            InventoryPanelRoot,
            Name::new("Inventory Panel"),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Inventory"),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.92, 0.77)),
                Pickable::IGNORE,
            ));

            for definition in ITEM_DEFINITIONS {
                panel
                    .spawn((
                        Node {
                            height: Val::Px(24.0),
                            padding: UiRect::horizontal(Val::Px(4.0)),
                            column_gap: Val::Px(7.0),
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.9)),
                        BorderColor::all(Color::srgb(0.24, 0.27, 0.34)),
                        Visibility::Hidden,
                        Button,
                        InventoryRow(definition.id),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(14.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(item_color(definition.id)),
                            BorderColor::all(Color::srgb(0.85, 0.85, 0.88)),
                            Pickable::IGNORE,
                        ));
                        row.spawn((
                            Text::new(definition.name),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.91, 0.92, 0.95)),
                            Pickable::IGNORE,
                        ));
                        row.spawn((
                            Text::new("x0"),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.84, 0.38)),
                            Node {
                                margin: UiRect::left(Val::Auto),
                                ..default()
                            },
                            Pickable::IGNORE,
                            InventoryQuantityText(definition.id),
                        ));
                    });
            }
        });
}

fn inventory_item_interactions(
    time: Res<Time>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    interactions: Query<(&Interaction, &InventoryRow), Changed<Interaction>>,
    inventory: Query<&Inventory, With<ControlledPlayer>>,
    mut commands: Commands,
    mut state: ResMut<InventoryUiState>,
    mut player_commands: MessageWriter<PlayerCommand>,
) {
    let Ok(inventory) = inventory.single() else {
        return;
    };
    let Ok(window) = primary_window.single() else {
        return;
    };

    for (interaction, row) in &interactions {
        if *interaction != Interaction::Pressed || inventory.quantity(row.0) == 0 {
            continue;
        }
        let Some(definition) = item_definition(row.0) else {
            continue;
        };
        if definition.consumable.is_none() {
            continue;
        }

        let now = time.elapsed_secs_f64();
        if state.last_click.is_some_and(|(item_id, clicked_at)| {
            item_id == row.0 && now - clicked_at <= DOUBLE_CLICK_SECONDS
        }) {
            player_commands.write(PlayerCommand::UseItem { item_id: row.0 });
            state.last_click = None;
        } else {
            state.last_click = Some((row.0, now));
        }

        if let Some(ghost) = state.drag_ghost.take() {
            commands.entity(ghost).try_despawn();
        }
        state.dragging = Some(row.0);
        let pointer = window.cursor_position().unwrap_or_default();
        let ghost = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(pointer.x - 10.0),
                    top: Val::Px(pointer.y - 10.0),
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(item_color(row.0)),
                BorderColor::all(Color::WHITE),
                GlobalZIndex(1001),
                Pickable::IGNORE,
                InventoryDragGhost,
                Name::new("Dragged inventory item"),
            ))
            .id();
        state.drag_ghost = Some(ghost);
    }
}

fn update_inventory_drag(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    action_bar: Res<ActionBarState>,
    mut bindings: ResMut<ActionBarBindings>,
    mut state: ResMut<InventoryUiState>,
    mut ghosts: Query<&mut Node, With<InventoryDragGhost>>,
    mut commands: Commands,
) {
    let Some(item_id) = state.dragging else {
        return;
    };
    let Ok(window) = primary_window.single() else {
        return;
    };
    let pointer = window.cursor_position();

    if let (Some(pointer), Some(ghost)) = (pointer, state.drag_ghost) {
        if let Ok(mut node) = ghosts.get_mut(ghost) {
            node.left = Val::Px(pointer.x - 10.0);
            node.top = Val::Px(pointer.y - 10.0);
        }
    }

    if !mouse_buttons.just_released(MouseButton::Left) {
        return;
    }

    if let Some(pointer) = pointer {
        if let Some(slot_index) = action_bar.slot_at(pointer) {
            bindings.bind_item(slot_index, item_id);
        }
    }
    if let Some(ghost) = state.drag_ghost.take() {
        commands.entity(ghost).try_despawn();
    }
    state.dragging = None;
}

fn update_inventory_panel(
    inventory: Query<&Inventory, (With<ControlledPlayer>, Changed<Inventory>)>,
    mut rows: Query<(&InventoryRow, &mut Visibility)>,
    mut quantities: Query<(&InventoryQuantityText, &mut Text)>,
) {
    let Ok(inventory) = inventory.single() else {
        return;
    };

    for (row, mut visibility) in &mut rows {
        *visibility = if inventory.quantity(row.0) > 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (quantity, mut text) in &mut quantities {
        text.0 = format!("x{}", inventory.quantity(quantity.0));
    }
}

fn despawn_inventory_panel(
    mut commands: Commands,
    panels: Query<Entity, With<InventoryPanelRoot>>,
    ghosts: Query<Entity, With<InventoryDragGhost>>,
    mut state: ResMut<InventoryUiState>,
) {
    for panel in &panels {
        commands.entity(panel).try_despawn();
    }
    for ghost in &ghosts {
        commands.entity(ghost).try_despawn();
    }
    state.dragging = None;
    state.drag_ghost = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_panel_captures_only_its_screen_region() {
        let state = InventoryUiState::default();
        let window = Vec2::new(720.0, 720.0);

        assert!(state.captures_pointer(Vec2::new(600.0, 20.0), window));
        assert!(!state.captures_pointer(Vec2::new(500.0, 20.0), window));
        assert!(!state.captures_pointer(Vec2::new(600.0, 200.0), window));
    }

    #[test]
    fn new_ground_items_begin_at_the_authoritative_landing_point() {
        let landing = Vec3::new(2.0, 0.06, -3.0);
        let (transform, animation) = falling_ground_item(landing);

        assert_eq!(animation.landing, landing);
        assert_eq!(transform.translation, landing);
    }

    #[test]
    fn ground_item_arc_rises_then_returns_to_the_floor() {
        assert_eq!(ground_item_arc_height(0.0), 0.0);
        assert_eq!(ground_item_arc_height(0.5), GROUND_ITEM_ARC_HEIGHT);
        assert_eq!(ground_item_arc_height(1.0), 0.0);
    }
}
