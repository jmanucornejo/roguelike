use bevy::prelude::*;
use bevy_renet::RenetServer;

use crate::{
    server::{
        persistence::{PersistenceClient, PersistenceRequest},
        state::CharacterPersistenceQueue,
    },
    shared::{
        gameplay::{
            components::{CharacterId, CharacterStats, Equipment, EquipmentSlot, Health, Mana},
            entities::Player,
            items::{equipment_derived_stats, item_definition, Inventory, ItemDefinitionId},
            progression::BaseProgression,
        },
        network::{channels::ServerChannel, messages::ServerMessages},
    },
};

#[derive(Event, Debug)]
pub struct RequestEquipItem {
    pub player: Entity,
    pub item_id: ItemDefinitionId,
}

#[derive(Event, Debug)]
pub struct RequestUnequipItem {
    pub player: Entity,
    pub slot: EquipmentSlot,
}

pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(equip_inventory_item)
            .add_observer(unequip_inventory_item);
    }
}

fn first_available_slot(item_id: ItemDefinitionId, equipment: &Equipment) -> Option<EquipmentSlot> {
    item_definition(item_id)?
        .equipment_slots
        .iter()
        .copied()
        .find(|slot| equipment.item(*slot).is_none())
}

fn equip_item(
    inventory: &mut Inventory,
    equipment: &mut Equipment,
    item_id: ItemDefinitionId,
) -> Option<EquipmentSlot> {
    if inventory.quantity(item_id) == 0 {
        return None;
    }
    let slot = first_available_slot(item_id, equipment)?;
    inventory.remove(item_id, 1).then(|| {
        equipment.set(slot, Some(item_id));
        slot
    })
}

fn unequip_item(
    inventory: &mut Inventory,
    equipment: &mut Equipment,
    slot: EquipmentSlot,
) -> Option<ItemDefinitionId> {
    let item_id = equipment.item(slot)?;
    equipment.set(slot, None);
    inventory.add(item_id, 1);
    Some(item_id)
}

fn sync_equipment_resources(
    stats: &CharacterStats,
    progression: &BaseProgression,
    equipment: &Equipment,
    health: &mut Health,
    mana: &mut Mana,
) {
    let derived = equipment_derived_stats(stats, progression.level, equipment);
    health.max = derived.max_health;
    health.current = health.current.min(health.max);
    mana.max = derived.max_mana;
    mana.current = mana.current.min(mana.max);
}

fn equip_inventory_item(
    trigger: On<RequestEquipItem>,
    mut server: ResMut<RenetServer>,
    mut players: Query<(
        Entity,
        &Player,
        &CharacterId,
        &CharacterStats,
        &BaseProgression,
        &mut Inventory,
        &mut Equipment,
        &mut Health,
        &mut Mana,
    )>,
    persistence: Option<Res<PersistenceClient>>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
) {
    let request = trigger.event();
    let Ok((
        player_entity,
        player,
        character_id,
        stats,
        progression,
        mut inventory,
        mut equipment,
        mut health,
        mut mana,
    )) = players.get_mut(request.player)
    else {
        return;
    };

    let Some(slot) = equip_item(&mut inventory, &mut equipment, request.item_id) else {
        warn!(
            "Rejected equip of unavailable item {} for character {} or no compatible empty slot",
            request.item_id.0, character_id.0
        );
        return;
    };
    sync_equipment_resources(stats, progression, &equipment, &mut health, &mut mana);

    send_inventory_and_equipment(
        &mut server,
        player.id,
        player_entity,
        &inventory,
        &equipment,
    );

    if character_id.0 != 0 {
        if let Some(persistence) = persistence.as_deref() {
            let request_id = persistence_queue.next_request_id();
            if let Err(error) = persistence.send(PersistenceRequest::EquipInventoryItem {
                request_id,
                character_id: *character_id,
                item_id: request.item_id,
                slot,
            }) {
                error!(
                    "Could not persist item {} in {} for character {}: {}",
                    request.item_id.0,
                    slot.name(),
                    character_id.0,
                    error
                );
            }
        }
    }
}

fn unequip_inventory_item(
    trigger: On<RequestUnequipItem>,
    mut server: ResMut<RenetServer>,
    mut players: Query<(
        Entity,
        &Player,
        &CharacterId,
        &CharacterStats,
        &BaseProgression,
        &mut Inventory,
        &mut Equipment,
        &mut Health,
        &mut Mana,
    )>,
    persistence: Option<Res<PersistenceClient>>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
) {
    let request = trigger.event();
    let Ok((
        player_entity,
        player,
        character_id,
        stats,
        progression,
        mut inventory,
        mut equipment,
        mut health,
        mut mana,
    )) = players.get_mut(request.player)
    else {
        return;
    };
    let Some(_item_id) = unequip_item(&mut inventory, &mut equipment, request.slot) else {
        return;
    };
    sync_equipment_resources(stats, progression, &equipment, &mut health, &mut mana);
    send_inventory_and_equipment(
        &mut server,
        player.id,
        player_entity,
        &inventory,
        &equipment,
    );

    if character_id.0 != 0 {
        if let Some(persistence) = persistence.as_deref() {
            let request_id = persistence_queue.next_request_id();
            if let Err(error) = persistence.send(PersistenceRequest::UnequipInventoryItem {
                request_id,
                character_id: *character_id,
                slot: request.slot,
            }) {
                error!(
                    "Could not persist unequip from {} for character {}: {}",
                    request.slot.name(),
                    character_id.0,
                    error
                );
            }
        }
    }
}

fn send_inventory_and_equipment(
    server: &mut RenetServer,
    client_id: u64,
    player_entity: Entity,
    inventory: &Inventory,
    equipment: &Equipment,
) {
    let inventory_message = bincode::serialize(&ServerMessages::InventoryUpdated {
        entity: player_entity,
        inventory: inventory.clone(),
    })
    .expect("inventory update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, inventory_message);

    let equipment_message = bincode::serialize(&ServerMessages::EquipmentUpdated {
        entity: player_entity,
        equipment: equipment.clone(),
    })
    .expect("equipment update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, equipment_message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::gameplay::items::{CLOTH_ARMOR, LUCKY_CLOVER, RED_HERB};

    #[test]
    fn accessories_fill_left_then_right_and_reject_a_third() {
        let mut inventory = Inventory::default();
        inventory.add(LUCKY_CLOVER, 2);
        let mut equipment = Equipment::default();

        assert_eq!(
            equip_item(&mut inventory, &mut equipment, LUCKY_CLOVER),
            Some(EquipmentSlot::AccessoryLeft)
        );
        assert_eq!(
            equip_item(&mut inventory, &mut equipment, LUCKY_CLOVER),
            Some(EquipmentSlot::AccessoryRight)
        );
        assert_eq!(
            equip_item(&mut inventory, &mut equipment, LUCKY_CLOVER),
            None
        );
        assert_eq!(inventory.quantity(LUCKY_CLOVER), 0);
    }

    #[test]
    fn unequipping_returns_the_item_to_inventory() {
        let mut inventory = Inventory::default();
        let mut equipment = Equipment::default();
        equipment.set(EquipmentSlot::AccessoryLeft, Some(LUCKY_CLOVER));

        assert_eq!(
            unequip_item(&mut inventory, &mut equipment, EquipmentSlot::AccessoryLeft),
            Some(LUCKY_CLOVER)
        );
        assert_eq!(inventory.quantity(LUCKY_CLOVER), 1);
        assert_eq!(equipment.item(EquipmentSlot::AccessoryLeft), None);
    }

    #[test]
    fn consumables_without_equipment_metadata_cannot_be_equipped() {
        assert_eq!(first_available_slot(RED_HERB, &Equipment::default()), None);
    }

    #[test]
    fn equipment_resource_sync_applies_and_removes_maximum_health_bonus() {
        let stats = CharacterStats::default();
        let progression = BaseProgression::default();
        let mut equipment = Equipment::default();
        let mut health = Health {
            current: 45,
            max: 50,
        };
        let mut mana = Mana {
            current: 10,
            max: 10,
        };

        equipment.set(EquipmentSlot::Armor, Some(CLOTH_ARMOR));
        sync_equipment_resources(&stats, &progression, &equipment, &mut health, &mut mana);
        assert_eq!(health.max, 50);
        assert_eq!(health.current, 45);

        equipment.set(EquipmentSlot::Armor, None);
        sync_equipment_resources(&stats, &progression, &equipment, &mut health, &mut mana);
        assert_eq!(health.max, 40);
        assert_eq!(health.current, 40);
    }
}
