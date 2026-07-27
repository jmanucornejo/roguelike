use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Reflect, Serialize, Deserialize,
)]
pub struct ItemDefinitionId(pub u32);

pub const PIG_MEAT: ItemDefinitionId = ItemDefinitionId(1);
pub const RED_HERB: ItemDefinitionId = ItemDefinitionId(2);
pub const LUCKY_CLOVER: ItemDefinitionId = ItemDefinitionId(3);
pub const GROUND_ITEM_VISUAL_HALF_HEIGHT: f32 = 0.06;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemDefinition {
    pub id: ItemDefinitionId,
    pub name: &'static str,
    pub consumable: Option<ConsumableEffect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsumableEffect {
    RestoreHealth(u32),
}

pub const ITEM_DEFINITIONS: [ItemDefinition; 3] = [
    ItemDefinition {
        id: PIG_MEAT,
        name: "Pig Meat",
        consumable: None,
    },
    ItemDefinition {
        id: RED_HERB,
        name: "Red Herb",
        consumable: Some(ConsumableEffect::RestoreHealth(10)),
    },
    ItemDefinition {
        id: LUCKY_CLOVER,
        name: "Lucky Clover",
        consumable: None,
    },
];

pub fn item_definition(id: ItemDefinitionId) -> Option<&'static ItemDefinition> {
    ITEM_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
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
}
