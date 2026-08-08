use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};
use std::f32::consts::TAU;

use crate::{
    client::{
        presentation::{
            action_bar::{ActionBarBindings, ActionBarState},
            ui_drag::DraggableUi,
        },
        state::ControlledPlayer,
    },
    shared::{
        gameplay::action_bar::ActionBarBinding,
        gameplay::components::{Equipment, EquipmentSlot},
        gameplay::items::{
            equipment_bonus_summary, item_definition, Inventory, ItemDefinitionId,
            APPRENTICE_STAFF, BASIC_SWORD, CLOTH_ARMOR, ITEM_DEFINITIONS, LUCKY_CLOVER, PIG_MEAT,
            RED_HERB, SIMPLE_BOOTS,
        },
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

const INVENTORY_PANEL_WIDTH: f32 = 280.0;
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
struct InventoryDetailsText;

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
                    update_inventory_item_details,
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
        BASIC_SWORD => Color::srgb(0.72, 0.75, 0.82),
        CLOTH_ARMOR => Color::srgb(0.45, 0.58, 0.76),
        SIMPLE_BOOTS => Color::srgb(0.48, 0.32, 0.20),
        APPRENTICE_STAFF => Color::srgb(0.52, 0.24, 0.76),
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
            DraggableUi::header(32.0),
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
                let item_label = if definition.consumable.is_some() {
                    format!("{} [Use]", definition.name)
                } else if !definition.equipment_slots.is_empty() {
                    let bonuses = equipment_bonus_summary(definition.bonuses);
                    format!("{} [Equip] ({bonuses})", definition.name)
                } else {
                    definition.name.to_owned()
                };
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
                            Text::new(item_label),
                            TextFont {
                                font_size: FontSize::Px(10.0),
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

            panel.spawn((
                Text::new("Hover an item to see its equipment comparison."),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.64, 0.72, 0.84)),
                Node {
                    min_height: Val::Px(48.0),
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                },
                Pickable::IGNORE,
                InventoryDetailsText,
            ));
        });
}

fn inventory_item_details(item_id: ItemDefinitionId, equipment: &Equipment) -> String {
    let Some(definition) = item_definition(item_id) else {
        return format!("Unknown item #{}", item_id.0);
    };
    let bonuses = equipment_bonus_summary(definition.bonuses);
    let mut details = if bonuses.is_empty() {
        definition.name.to_owned()
    } else {
        format!("{}: {bonuses}", definition.name)
    };
    if definition.equipment_slots.is_empty() {
        return details;
    }

    let comparison_slot = definition
        .equipment_slots
        .iter()
        .copied()
        .find(|slot| equipment.item(*slot).is_none())
        .or_else(|| definition.equipment_slots.first().copied());
    if let Some(slot) = comparison_slot {
        let comparison = equipment.item(slot).and_then(item_definition);
        if let Some(comparison) = comparison {
            let equipped_bonuses = equipment_bonus_summary(comparison.bonuses);
            details.push_str(&format!(
                "\n{} currently: {} ({equipped_bonuses})",
                slot.name(),
                comparison.name
            ));
        } else {
            details.push_str(&format!("\n{} currently: Empty", slot.name()));
        }
    }
    details
}

fn update_inventory_item_details(
    rows: Query<(&InventoryRow, &Interaction)>,
    equipment: Query<&Equipment, With<ControlledPlayer>>,
    mut details: Query<&mut Text, With<InventoryDetailsText>>,
) {
    let Ok(equipment) = equipment.single() else {
        return;
    };
    let hovered = rows.iter().find_map(|(row, interaction)| {
        matches!(*interaction, Interaction::Hovered | Interaction::Pressed).then_some(row.0)
    });
    let text = hovered.map_or_else(
        || "Hover an item to see its equipment comparison.".to_owned(),
        |item_id| inventory_item_details(item_id, equipment),
    );
    for mut details in &mut details {
        details.0.clone_from(&text);
    }
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

        let now = time.elapsed_secs_f64();
        let can_activate =
            definition.consumable.is_some() || !definition.equipment_slots.is_empty();
        let double_clicked = can_activate
            && state.last_click.is_some_and(|(item_id, clicked_at)| {
                item_id == row.0 && now - clicked_at <= DOUBLE_CLICK_SECONDS
            });
        if double_clicked {
            if definition.consumable.is_some() {
                player_commands.write(PlayerCommand::UseItem { item_id: row.0 });
            } else {
                player_commands.write(PlayerCommand::EquipItem { item_id: row.0 });
            }
            state.last_click = None;
            if let Some(ghost) = state.drag_ghost.take() {
                commands.entity(ghost).try_despawn();
            }
            state.dragging = None;
            continue;
        } else if can_activate {
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
    mut player_commands: MessageWriter<PlayerCommand>,
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
            if bindings.bind_item(slot_index, item_id) {
                player_commands.write(PlayerCommand::SetActionBarSlot {
                    slot_index: slot_index as u8,
                    binding: Some(ActionBarBinding::Item(item_id)),
                });
            }
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

    #[test]
    fn item_details_compare_equipment_with_the_compatible_slot() {
        let mut equipment = Equipment::default();
        equipment.set(EquipmentSlot::MainHand, Some(BASIC_SWORD));

        let details = inventory_item_details(APPRENTICE_STAFF, &equipment);
        assert!(details.contains("Apprentice Staff: +5 Magic, +5 SP"));
        assert!(details.contains("Main Hand currently: Basic Sword (+5 ATK)"));
    }
}
