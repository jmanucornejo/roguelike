use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

use super::items::ItemDefinitionId;
use super::skills::SkillId;

pub const ACTION_BAR_SLOT_COUNT: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActionBarBinding {
    Spell(u16),
    Item(ItemDefinitionId),
    Skill(SkillId),
}

#[derive(Component, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionBarLayout {
    pub slots: [Option<ActionBarBinding>; ACTION_BAR_SLOT_COUNT],
}

impl Default for ActionBarLayout {
    fn default() -> Self {
        Self {
            slots: [
                Some(ActionBarBinding::Spell(1)),
                Some(ActionBarBinding::Spell(2)),
                Some(ActionBarBinding::Spell(3)),
                Some(ActionBarBinding::Spell(4)),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        }
    }
}

impl ActionBarLayout {
    pub fn binding(&self, slot_index: usize) -> Option<ActionBarBinding> {
        self.slots.get(slot_index).copied().flatten()
    }

    pub fn set(&mut self, slot_index: usize, binding: Option<ActionBarBinding>) -> bool {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return false;
        };
        *slot = binding;
        true
    }

    pub fn swap(&mut self, first_slot: usize, second_slot: usize) -> bool {
        if first_slot >= ACTION_BAR_SLOT_COUNT || second_slot >= ACTION_BAR_SLOT_COUNT {
            return false;
        }
        self.slots.swap(first_slot, second_slot);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::gameplay::items::RED_HERB;
    use crate::shared::gameplay::skills::SkillId;

    #[test]
    fn new_characters_start_with_the_four_test_spells() {
        let layout = ActionBarLayout::default();

        assert_eq!(layout.binding(0), Some(ActionBarBinding::Spell(1)));
        assert_eq!(layout.binding(3), Some(ActionBarBinding::Spell(4)));
        assert_eq!(layout.binding(4), None);
    }

    #[test]
    fn every_slot_can_be_replaced_with_an_item() {
        let mut layout = ActionBarLayout::default();

        assert!(layout.set(3, Some(ActionBarBinding::Item(RED_HERB))));
        assert_eq!(layout.binding(3), Some(ActionBarBinding::Item(RED_HERB)));
        assert!(!layout.set(
            ACTION_BAR_SLOT_COUNT,
            Some(ActionBarBinding::Item(RED_HERB))
        ));
    }

    #[test]
    fn bindings_can_be_reordered_across_the_entire_bar() {
        let mut layout = ActionBarLayout::default();

        assert!(layout.swap(0, 9));
        assert_eq!(layout.binding(0), None);
        assert_eq!(layout.binding(9), Some(ActionBarBinding::Spell(1)));
        assert!(!layout.swap(0, ACTION_BAR_SLOT_COUNT));
    }

    #[test]
    fn learned_skills_can_occupy_action_bar_slots() {
        let mut layout = ActionBarLayout::default();

        assert!(layout.set(5, Some(ActionBarBinding::Skill(SkillId(301)))));
        assert_eq!(
            layout.binding(5),
            Some(ActionBarBinding::Skill(SkillId(301)))
        );
    }
}
