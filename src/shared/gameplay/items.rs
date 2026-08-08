use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::shared::gameplay::components::{
    CharacterAttribute, CharacterStats, DerivedCharacterStats, Equipment, EquipmentSlot,
};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Reflect, Serialize, Deserialize,
)]
pub struct ItemDefinitionId(pub u32);

pub const PIG_MEAT: ItemDefinitionId = ItemDefinitionId(1);
pub const RED_HERB: ItemDefinitionId = ItemDefinitionId(2);
pub const LUCKY_CLOVER: ItemDefinitionId = ItemDefinitionId(3);
pub const BASIC_SWORD: ItemDefinitionId = ItemDefinitionId(4);
pub const CLOTH_ARMOR: ItemDefinitionId = ItemDefinitionId(5);
pub const SIMPLE_BOOTS: ItemDefinitionId = ItemDefinitionId(6);
pub const APPRENTICE_STAFF: ItemDefinitionId = ItemDefinitionId(7);
pub const GROUND_ITEM_VISUAL_HALF_HEIGHT: f32 = 0.06;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EquipmentBonuses {
    pub might: u16,
    pub finesse: u16,
    pub agility: u16,
    pub vitality: u16,
    pub intellect: u16,
    pub spirit: u16,
    pub physical_attack: u32,
    pub magic_power: u32,
    pub physical_defense: u32,
    pub magic_defense: u32,
    pub hit: u32,
    pub flee: u32,
    pub max_health: u32,
    pub max_mana: u32,
}

impl EquipmentBonuses {
    pub const NONE: Self = Self {
        might: 0,
        finesse: 0,
        agility: 0,
        vitality: 0,
        intellect: 0,
        spirit: 0,
        physical_attack: 0,
        magic_power: 0,
        physical_defense: 0,
        magic_defense: 0,
        hit: 0,
        flee: 0,
        max_health: 0,
        max_mana: 0,
    };

    pub const fn attribute(self, attribute: CharacterAttribute) -> u16 {
        match attribute {
            CharacterAttribute::Might => self.might,
            CharacterAttribute::Finesse => self.finesse,
            CharacterAttribute::Agility => self.agility,
            CharacterAttribute::Vitality => self.vitality,
            CharacterAttribute::Intellect => self.intellect,
            CharacterAttribute::Spirit => self.spirit,
        }
    }

    pub fn add_assign(&mut self, other: Self) {
        self.might = self.might.saturating_add(other.might);
        self.finesse = self.finesse.saturating_add(other.finesse);
        self.agility = self.agility.saturating_add(other.agility);
        self.vitality = self.vitality.saturating_add(other.vitality);
        self.intellect = self.intellect.saturating_add(other.intellect);
        self.spirit = self.spirit.saturating_add(other.spirit);
        self.physical_attack = self.physical_attack.saturating_add(other.physical_attack);
        self.magic_power = self.magic_power.saturating_add(other.magic_power);
        self.physical_defense = self.physical_defense.saturating_add(other.physical_defense);
        self.magic_defense = self.magic_defense.saturating_add(other.magic_defense);
        self.hit = self.hit.saturating_add(other.hit);
        self.flee = self.flee.saturating_add(other.flee);
        self.max_health = self.max_health.saturating_add(other.max_health);
        self.max_mana = self.max_mana.saturating_add(other.max_mana);
    }

    pub fn derived(self, stats: &CharacterStats, base_level: u16) -> DerivedCharacterStats {
        let effective_stats = CharacterStats {
            might: stats.might.saturating_add(self.might),
            finesse: stats.finesse.saturating_add(self.finesse),
            agility: stats.agility.saturating_add(self.agility),
            vitality: stats.vitality.saturating_add(self.vitality),
            intellect: stats.intellect.saturating_add(self.intellect),
            spirit: stats.spirit.saturating_add(self.spirit),
            available_points: stats.available_points,
        };
        let mut derived = effective_stats.derived(base_level);
        derived.physical_attack = derived.physical_attack.saturating_add(self.physical_attack);
        derived.magic_power = derived.magic_power.saturating_add(self.magic_power);
        derived.physical_defense = derived
            .physical_defense
            .saturating_add(self.physical_defense);
        derived.magic_defense = derived.magic_defense.saturating_add(self.magic_defense);
        derived.hit = derived.hit.saturating_add(self.hit);
        derived.flee = derived.flee.saturating_add(self.flee);
        derived.max_health = derived.max_health.saturating_add(self.max_health);
        derived.max_mana = derived.max_mana.saturating_add(self.max_mana);
        derived
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemDefinition {
    pub id: ItemDefinitionId,
    pub name: &'static str,
    pub price: u64,
    pub consumable: Option<ConsumableEffect>,
    pub equipment_slots: &'static [EquipmentSlot],
    pub bonuses: EquipmentBonuses,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsumableEffect {
    RestoreHealth(u32),
}

pub const ITEM_DEFINITIONS: [ItemDefinition; 7] = [
    ItemDefinition {
        id: PIG_MEAT,
        name: "Pig Meat",
        price: 10,
        consumable: None,
        equipment_slots: &[],
        bonuses: EquipmentBonuses::NONE,
    },
    ItemDefinition {
        id: RED_HERB,
        name: "Red Herb",
        price: 5,
        consumable: Some(ConsumableEffect::RestoreHealth(10)),
        equipment_slots: &[],
        bonuses: EquipmentBonuses::NONE,
    },
    ItemDefinition {
        id: LUCKY_CLOVER,
        name: "Lucky Clover",
        price: 10,
        consumable: None,
        equipment_slots: &[EquipmentSlot::AccessoryLeft, EquipmentSlot::AccessoryRight],
        bonuses: EquipmentBonuses {
            spirit: 1,
            flee: 2,
            ..EquipmentBonuses::NONE
        },
    },
    ItemDefinition {
        id: BASIC_SWORD,
        name: "Basic Sword",
        price: 10,
        consumable: None,
        equipment_slots: &[EquipmentSlot::MainHand],
        bonuses: EquipmentBonuses {
            physical_attack: 5,
            ..EquipmentBonuses::NONE
        },
    },
    ItemDefinition {
        id: CLOTH_ARMOR,
        name: "Cloth Armor",
        price: 10,
        consumable: None,
        equipment_slots: &[EquipmentSlot::Armor],
        bonuses: EquipmentBonuses {
            physical_defense: 4,
            max_health: 10,
            ..EquipmentBonuses::NONE
        },
    },
    ItemDefinition {
        id: SIMPLE_BOOTS,
        name: "Simple Boots",
        price: 5,
        consumable: None,
        equipment_slots: &[EquipmentSlot::Shoes],
        bonuses: EquipmentBonuses {
            flee: 2,
            ..EquipmentBonuses::NONE
        },
    },
    ItemDefinition {
        id: APPRENTICE_STAFF,
        name: "Apprentice Staff",
        price: 10,
        consumable: None,
        equipment_slots: &[EquipmentSlot::MainHand],
        bonuses: EquipmentBonuses {
            magic_power: 5,
            max_mana: 5,
            ..EquipmentBonuses::NONE
        },
    },
];

pub fn item_definition(id: ItemDefinitionId) -> Option<&'static ItemDefinition> {
    ITEM_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
}

pub fn equipment_bonuses(equipment: &Equipment) -> EquipmentBonuses {
    let mut bonuses = EquipmentBonuses::default();
    for item_id in equipment.slots.iter().flatten() {
        if let Some(definition) = item_definition(*item_id) {
            bonuses.add_assign(definition.bonuses);
        }
    }
    bonuses
}

pub fn equipment_derived_stats(
    stats: &CharacterStats,
    base_level: u16,
    equipment: &Equipment,
) -> DerivedCharacterStats {
    equipment_bonuses(equipment).derived(stats, base_level)
}

pub fn equipment_bonus_summary(bonuses: EquipmentBonuses) -> String {
    let mut entries = Vec::new();
    for attribute in CharacterAttribute::ALL {
        let amount = bonuses.attribute(attribute);
        if amount > 0 {
            entries.push(format!("+{amount} {}", attribute.name()));
        }
    }
    for (amount, name) in [
        (bonuses.physical_attack, "ATK"),
        (bonuses.magic_power, "Magic"),
        (bonuses.physical_defense, "DEF"),
        (bonuses.magic_defense, "MDEF"),
        (bonuses.hit, "HIT"),
        (bonuses.flee, "FLEE"),
        (bonuses.max_health, "HP"),
        (bonuses.max_mana, "SP"),
    ] {
        if amount > 0 {
            entries.push(format!("+{amount} {name}"));
        }
    }
    entries.join(", ")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ItemStack {
    pub item_id: ItemDefinitionId,
    pub quantity: u32,
}

#[derive(Component, Clone, Debug, Default, Eq, PartialEq, Reflect, Serialize, Deserialize)]
pub struct Inventory {
    pub items: Vec<ItemStack>,
}

impl Inventory {
    pub fn add(&mut self, item_id: ItemDefinitionId, quantity: u32) {
        if quantity == 0 {
            return;
        }

        if let Some(stack) = self.items.iter_mut().find(|stack| stack.item_id == item_id) {
            stack.quantity = stack.quantity.saturating_add(quantity);
        } else {
            self.items.push(ItemStack { item_id, quantity });
            self.items.sort_by_key(|stack| stack.item_id);
        }
    }

    pub fn quantity(&self, item_id: ItemDefinitionId) -> u32 {
        self.items
            .iter()
            .find(|stack| stack.item_id == item_id)
            .map(|stack| stack.quantity)
            .unwrap_or_default()
    }

    pub fn remove(&mut self, item_id: ItemDefinitionId, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }

        let Some(index) = self.items.iter().position(|stack| stack.item_id == item_id) else {
            return false;
        };
        if self.items[index].quantity < quantity {
            return false;
        }

        self.items[index].quantity -= quantity;
        if self.items[index].quantity == 0 {
            self.items.remove(index);
        }
        true
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Reflect, Serialize, Deserialize)]
pub struct GroundItem {
    pub item_id: ItemDefinitionId,
    pub quantity: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_stacks_identical_items_and_ignores_zero_quantity() {
        let mut inventory = Inventory::default();

        inventory.add(PIG_MEAT, 1);
        inventory.add(PIG_MEAT, 2);
        inventory.add(RED_HERB, 0);

        assert_eq!(inventory.quantity(PIG_MEAT), 3);
        assert_eq!(inventory.quantity(RED_HERB), 0);
        assert_eq!(inventory.items.len(), 1);
    }

    #[test]
    fn inventory_removal_is_atomic_and_removes_empty_stacks() {
        let mut inventory = Inventory::default();
        inventory.add(RED_HERB, 2);

        assert!(!inventory.remove(RED_HERB, 3));
        assert_eq!(inventory.quantity(RED_HERB), 2);
        assert!(inventory.remove(RED_HERB, 1));
        assert_eq!(inventory.quantity(RED_HERB), 1);
        assert!(inventory.remove(RED_HERB, 1));
        assert_eq!(inventory.quantity(RED_HERB), 0);
        assert!(inventory.items.is_empty());
    }

    #[test]
    fn red_herb_restores_ten_health() {
        assert_eq!(
            item_definition(RED_HERB).and_then(|definition| definition.consumable),
            Some(ConsumableEffect::RestoreHealth(10))
        );
    }

    #[test]
    fn existing_items_have_placeholder_gold_prices() {
        assert_eq!(item_definition(PIG_MEAT).map(|item| item.price), Some(10));
        assert_eq!(item_definition(RED_HERB).map(|item| item.price), Some(5));
        assert_eq!(
            item_definition(LUCKY_CLOVER).map(|item| item.price),
            Some(10)
        );
    }

    #[test]
    fn lucky_clover_is_placeholder_accessory_equipment() {
        assert_eq!(
            item_definition(LUCKY_CLOVER).map(|item| item.equipment_slots),
            Some([EquipmentSlot::AccessoryLeft, EquipmentSlot::AccessoryRight].as_slice())
        );
        assert!(item_definition(RED_HERB).is_some_and(|item| item.equipment_slots.is_empty()));
        assert_eq!(
            item_definition(LUCKY_CLOVER).map(|item| item.bonuses),
            Some(EquipmentBonuses {
                spirit: 1,
                flee: 2,
                ..EquipmentBonuses::NONE
            })
        );
    }

    #[test]
    fn equipped_item_bonuses_stack_and_feed_derived_stats() {
        let mut equipment = Equipment::default();
        equipment.set(EquipmentSlot::AccessoryLeft, Some(LUCKY_CLOVER));
        equipment.set(EquipmentSlot::AccessoryRight, Some(LUCKY_CLOVER));
        equipment.set(EquipmentSlot::Armor, Some(CLOTH_ARMOR));

        let bonuses = equipment_bonuses(&equipment);
        assert_eq!(bonuses.spirit, 2);
        assert_eq!(bonuses.flee, 4);
        assert_eq!(bonuses.physical_defense, 4);
        assert_eq!(bonuses.max_health, 10);

        let derived = equipment_derived_stats(&CharacterStats::default(), 1, &equipment);
        assert_eq!(derived.flee, 6);
        assert_eq!(derived.max_health, 50);
        assert_eq!(derived.magic_power, 6);
        assert_eq!(derived.physical_defense, 5);
    }
}
