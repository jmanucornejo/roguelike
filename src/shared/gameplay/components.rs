use bevy::prelude::*;
use bevy_rapier3d::prelude::{CharacterLength, KinematicCharacterController, QueryFilterFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::shared::{
    constants::{CHARACTER_CONTROLLER_OFFSET, CHARACTER_GROUND_SNAP_DISTANCE},
    gameplay::items::ItemDefinitionId,
};
// use crate::shared::enums::DamageType;

/// Stable database identity for a player character.
///
/// Bevy's [`Entity`] and Renet's client id only identify a character for the
/// lifetime of a running process. This id survives reconnects and server restarts.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Component,
)]
pub struct CharacterId(pub u64);

/// The character's authoritative currency balance.
#[derive(
    Component, Reflect, Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize,
)]
pub struct Gold(pub u64);

#[derive(Component, Reflect, Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct Dead;

/// Marks a player who has deliberately stopped to sit.
///
/// Sitting is transient gameplay state: it is replicated while connected but
/// is not persisted when a character logs out.
#[derive(Component, Reflect, Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct Sitting;

#[derive(Component, Reflect, Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct CharacterStats {
    pub might: u16,
    pub finesse: u16,
    pub agility: u16,
    pub vitality: u16,
    pub intellect: u16,
    pub spirit: u16,
    pub available_points: u32,
}

pub const MAX_ATTRIBUTE_VALUE: u16 = 99;
pub const STARTING_ATTRIBUTE_POINTS: u32 = 48;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Reflect, Serialize, Deserialize)]
#[repr(u8)]
pub enum CharacterAttribute {
    Might = 0,
    Finesse = 1,
    Agility = 2,
    Vitality = 3,
    Intellect = 4,
    Spirit = 5,
}

impl CharacterAttribute {
    pub const ALL: [Self; 6] = [
        Self::Might,
        Self::Finesse,
        Self::Agility,
        Self::Vitality,
        Self::Intellect,
        Self::Spirit,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Might => "Might",
            Self::Finesse => "Finesse",
            Self::Agility => "Agility",
            Self::Vitality => "Vitality",
            Self::Intellect => "Intellect",
            Self::Spirit => "Spirit",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Might => "Raises physical attack.",
            Self::Finesse => "Raises physical HIT.",
            Self::Agility => "Raises FLEE.",
            Self::Vitality => "Raises maximum HP.",
            Self::Intellect => "Raises maximum SP and magic power.",
            Self::Spirit => "Raises maximum SP and magic power.",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeSpendError {
    InsufficientPoints { required: u32, available: u32 },
    AttributeAtMaximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedCharacterStats {
    pub hit: u32,
    pub flee: u32,
    pub max_health: u32,
    pub max_mana: u32,
    pub physical_attack: u32,
    pub magic_power: u32,
    pub physical_defense: u32,
    pub magic_defense: u32,
}

impl Default for CharacterStats {
    fn default() -> Self {
        Self {
            might: 1,
            finesse: 1,
            agility: 1,
            vitality: 1,
            intellect: 1,
            spirit: 1,
            available_points: STARTING_ATTRIBUTE_POINTS,
        }
    }
}

impl CharacterStats {
    pub const fn value(&self, attribute: CharacterAttribute) -> u16 {
        match attribute {
            CharacterAttribute::Might => self.might,
            CharacterAttribute::Finesse => self.finesse,
            CharacterAttribute::Agility => self.agility,
            CharacterAttribute::Vitality => self.vitality,
            CharacterAttribute::Intellect => self.intellect,
            CharacterAttribute::Spirit => self.spirit,
        }
    }

    /// Classic Ragnarok cost for raising a base attribute from `value` to
    /// `value + 1`. Attributes at the cap cannot be raised.
    pub const fn attribute_point_cost(value: u16) -> Option<u32> {
        if value >= MAX_ATTRIBUTE_VALUE {
            return None;
        }
        Some(((value.saturating_sub(1) / 10) as u32) + 2)
    }

    pub const fn next_attribute_cost(&self, attribute: CharacterAttribute) -> Option<u32> {
        Self::attribute_point_cost(self.value(attribute))
    }

    /// Classic Ragnarok reward for advancing from `level` to `level + 1`.
    pub const fn attribute_points_for_next_base_level(level: u16) -> u32 {
        (level / 5) as u32 + 3
    }

    pub fn can_spend_point(
        &self,
        attribute: CharacterAttribute,
    ) -> Result<(), AttributeSpendError> {
        let Some(required) = self.next_attribute_cost(attribute) else {
            return Err(AttributeSpendError::AttributeAtMaximum);
        };
        if self.available_points < required {
            return Err(AttributeSpendError::InsufficientPoints {
                required,
                available: self.available_points,
            });
        }
        Ok(())
    }

    pub fn spend_point(
        &mut self,
        attribute: CharacterAttribute,
    ) -> Result<u16, AttributeSpendError> {
        self.can_spend_point(attribute)?;
        let cost = self
            .next_attribute_cost(attribute)
            .expect("validated attribute must have a cost");
        let value = match attribute {
            CharacterAttribute::Might => &mut self.might,
            CharacterAttribute::Finesse => &mut self.finesse,
            CharacterAttribute::Agility => &mut self.agility,
            CharacterAttribute::Vitality => &mut self.vitality,
            CharacterAttribute::Intellect => &mut self.intellect,
            CharacterAttribute::Spirit => &mut self.spirit,
        };
        *value += 1;
        self.available_points -= cost;
        Ok(*value)
    }

    pub fn grant_base_levels(&mut self, previous_level: u16, levels_gained: u16) -> u32 {
        let mut awarded = 0_u32;
        for offset in 0..levels_gained {
            let level = previous_level.saturating_add(offset);
            awarded = awarded.saturating_add(Self::attribute_points_for_next_base_level(level));
        }
        self.available_points = self.available_points.saturating_add(awarded);
        awarded
    }

    pub fn derived(&self, base_level: u16) -> DerivedCharacterStats {
        let level = u32::from(base_level.max(1));
        DerivedCharacterStats {
            hit: level.saturating_add(u32::from(self.finesse)),
            flee: level.saturating_add(u32::from(self.agility)),
            max_health: 30_u32
                .saturating_add(level.saturating_mul(5))
                .saturating_add(u32::from(self.vitality).saturating_mul(5)),
            max_mana: 5_u32
                .saturating_add(level.saturating_mul(2))
                .saturating_add(u32::from(self.intellect).saturating_mul(2))
                .saturating_add(u32::from(self.spirit)),
            physical_attack: level.saturating_add(u32::from(self.might).saturating_mul(2)),
            magic_power: level
                .saturating_add(u32::from(self.intellect).saturating_mul(2))
                .saturating_add(u32::from(self.spirit)),
            physical_defense: u32::from(self.vitality),
            magic_defense: u32::from(self.spirit),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Reflect, Serialize, Deserialize)]
#[repr(u16)]
pub enum EquipmentSlot {
    HeadUpper = 0,
    HeadMiddle = 1,
    HeadLower = 2,
    Armor = 3,
    MainHand = 4,
    OffHand = 5,
    Garment = 6,
    Shoes = 7,
    AccessoryLeft = 8,
    AccessoryRight = 9,
}

impl EquipmentSlot {
    pub const ALL: [Self; 10] = [
        Self::HeadUpper,
        Self::HeadMiddle,
        Self::HeadLower,
        Self::Armor,
        Self::MainHand,
        Self::OffHand,
        Self::Garment,
        Self::Shoes,
        Self::AccessoryLeft,
        Self::AccessoryRight,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: u16) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::HeadUpper => "Upper Head",
            Self::HeadMiddle => "Middle Head",
            Self::HeadLower => "Lower Head",
            Self::Armor => "Armor",
            Self::MainHand => "Main Hand",
            Self::OffHand => "Off Hand",
            Self::Garment => "Garment",
            Self::Shoes => "Shoes",
            Self::AccessoryLeft => "Accessory 1",
            Self::AccessoryRight => "Accessory 2",
        }
    }
}

#[derive(Component, Reflect, Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Equipment {
    pub slots: [Option<ItemDefinitionId>; 10],
}

impl Equipment {
    pub fn item(&self, slot: EquipmentSlot) -> Option<ItemDefinitionId> {
        self.slots[slot.index()]
    }

    pub fn set(&mut self, slot: EquipmentSlot, item: Option<ItemDefinitionId>) {
        self.slots[slot.index()] = item;
    }
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavePoint {
    pub map_name: String,
    pub position: [f32; 3],
}

#[cfg(test)]
mod character_foundation_tests {
    use super::*;

    #[test]
    fn equipment_exposes_all_ten_distinct_slots() {
        let mut indices = EquipmentSlot::ALL.map(EquipmentSlot::index);
        indices.sort_unstable();

        assert_eq!(indices, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(Equipment::default().slots, [None; 10]);
    }

    #[test]
    fn spending_an_attribute_point_is_validated_and_updates_the_value() {
        let mut stats = CharacterStats {
            available_points: 5,
            ..default()
        };

        assert_eq!(stats.spend_point(CharacterAttribute::Might), Ok(2));
        assert_eq!(stats.might, 2);
        assert_eq!(stats.available_points, 3);

        stats.might = 11;
        assert_eq!(stats.spend_point(CharacterAttribute::Might), Ok(12));
        assert_eq!(stats.available_points, 0);
        assert_eq!(
            stats.spend_point(CharacterAttribute::Might),
            Err(AttributeSpendError::InsufficientPoints {
                required: 3,
                available: 0,
            })
        );

        stats.might = MAX_ATTRIBUTE_VALUE;
        assert_eq!(
            stats.spend_point(CharacterAttribute::Might),
            Err(AttributeSpendError::AttributeAtMaximum)
        );
        assert_eq!(stats.available_points, 0);
    }

    #[test]
    fn base_levels_grant_points_and_attributes_calculate_derived_stats() {
        let mut stats = CharacterStats::default();
        assert_eq!(stats.available_points, STARTING_ATTRIBUTE_POINTS);
        assert_eq!(stats.grant_base_levels(4, 2), 7);
        assert_eq!(stats.available_points, STARTING_ATTRIBUTE_POINTS + 7);
        assert_eq!(
            stats.derived(1),
            DerivedCharacterStats {
                hit: 2,
                flee: 2,
                max_health: 40,
                max_mana: 10,
                physical_attack: 3,
                magic_power: 4,
                physical_defense: 1,
                magic_defense: 1,
            }
        );
    }

    #[test]
    fn classic_attribute_cost_and_level_reward_boundaries_match_ragnarok() {
        assert_eq!(CharacterStats::attribute_point_cost(1), Some(2));
        assert_eq!(CharacterStats::attribute_point_cost(10), Some(2));
        assert_eq!(CharacterStats::attribute_point_cost(11), Some(3));
        assert_eq!(CharacterStats::attribute_point_cost(90), Some(10));
        assert_eq!(CharacterStats::attribute_point_cost(91), Some(11));
        assert_eq!(CharacterStats::attribute_point_cost(99), None);

        assert_eq!(CharacterStats::attribute_points_for_next_base_level(1), 3);
        assert_eq!(CharacterStats::attribute_points_for_next_base_level(4), 3);
        assert_eq!(CharacterStats::attribute_points_for_next_base_level(5), 4);
        assert_eq!(CharacterStats::attribute_points_for_next_base_level(94), 21);
        assert_eq!(CharacterStats::attribute_points_for_next_base_level(98), 22);
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Pos(pub i32, pub i32);

#[derive(Debug, Default, Resource)]
pub struct Map {
    pub blocked_paths: HashSet<Pos>,
}

#[derive(Debug, Default, Component)]
pub struct GameVelocity(pub Vec3);

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Component)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub destination_at: Option<Pos>,
}

#[derive(Debug, PartialEq, Component, Clone)]
pub struct AttackingTimer(pub Timer);

#[derive(Component, Reflect, Debug, Serialize, Deserialize, Clone)]
pub struct Health {
    pub max: u32,
    pub current: u32,
}

#[derive(Component, Reflect, Debug, Serialize, Deserialize, Clone)]
pub struct Mana {
    pub max: u32,
    pub current: u32,
}

#[derive(Debug, Component)]
pub struct Building {
    pub blocked_paths: Vec<Pos>,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Component, Clone, Copy)]
pub enum MonsterKind {
    Pig,
    Orc,
}

#[derive(Debug, PartialEq, Eq, Hash, Component, Clone, Copy)]
pub enum MonsterAggression {
    /// Retaliates after any direct attack, but never initiates combat.
    Passive,
    /// Acquires nearby players without needing to be provoked.
    Aggressive,
    /// Ignores normal attacks and retaliates only against direct spells.
    SpellReactive,
}

#[derive(Debug, Default, Component, Deserialize, Serialize, Clone)]
pub struct SpriteId(pub u16);

pub const PASSIVE_MONSTER_PLACEHOLDER_SPRITE: u16 = 1;
pub const AGGRESSIVE_MONSTER_PLACEHOLDER_SPRITE: u16 = 2;
pub const SPELL_REACTIVE_MONSTER_PLACEHOLDER_SPRITE: u16 = 3;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Component, Clone)]
pub struct Facing(pub u8);

/// Converts a world-space direction into the eight-direction facing convention:
/// `0 = +Z`, `2 = -X`, `4 = -Z`, and `6 = +X`.
pub fn facing_from_direction(direction: Vec3) -> Option<Facing> {
    let planar = Vec2::new(direction.x, direction.z);
    if planar.length_squared() <= f32::EPSILON {
        return None;
    }

    let octant_angle = std::f32::consts::TAU / 8.0;
    let octant = ((-planar.x).atan2(planar.y) / octant_angle).round() as i32;
    Some(Facing(octant.rem_euclid(8) as u8))
}

/// Converts a stored eight-direction facing back into its world-space direction.
pub fn world_direction_from_facing(facing: u8) -> Vec3 {
    let angle = facing as f32 * (std::f32::consts::TAU / 8.0);
    Vec3::new(-angle.sin(), 0.0, angle.cos())
}

#[derive(Debug, PartialEq, Component, Clone)]
pub struct Monster {
    pub hp: i32,
    //pub speed: f32,
    pub kind: MonsterKind,
    // pub move_destination: Vec3,
    //pub move_timer: Timer
}

#[derive(Debug, PartialEq, Component, Clone)]
pub struct Aggro {
    pub enemy: Entity,
    pub auto_attack: bool,
    pub enemy_translation: Vec3,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Component, Clone)]
pub struct Walking {
    pub target_translation: Vec3,
    /// Path cells and the index of the next waypoint to visit.
    pub path: Option<(Vec<Pos>, u32)>,
}

#[derive(Debug, PartialEq, Component, Clone)]
pub struct Attacking {
    pub enemy: Entity,
    pub auto_attack: bool,
    //pub enemy_translation: Vec3,
    // pub timer: Timer
}

#[derive(Component)]
pub struct Billboard;

#[derive(Component, Debug)]
pub enum Animation {
    Idle,
    Walking,
    Sitting,
    Attacking {
        entity: Entity,
        enemy: Entity,
        attack_speed: f32,
        auto_attack: bool,
    },
    Casting,
}

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

pub fn player_character_controller() -> KinematicCharacterController {
    KinematicCharacterController {
        offset: CharacterLength::Absolute(CHARACTER_CONTROLLER_OFFSET),
        snap_to_ground: Some(CharacterLength::Absolute(CHARACTER_GROUND_SNAP_DISTANCE)),
        filter_flags: QueryFilterFlags::EXCLUDE_KINEMATIC,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_directions_map_to_the_eight_facing_octants() {
        let directions = [
            (Vec3::Z, 0),
            (Vec3::new(-1.0, 0.0, 1.0), 1),
            (Vec3::NEG_X, 2),
            (Vec3::new(-1.0, 0.0, -1.0), 3),
            (Vec3::NEG_Z, 4),
            (Vec3::new(1.0, 0.0, -1.0), 5),
            (Vec3::X, 6),
            (Vec3::new(1.0, 0.0, 1.0), 7),
        ];

        for (direction, expected) in directions {
            assert_eq!(facing_from_direction(direction), Some(Facing(expected)));
            assert!(
                world_direction_from_facing(expected)
                    .normalize()
                    .dot(direction.normalize())
                    > 1.0 - 1e-5
            );
        }
        assert_eq!(facing_from_direction(Vec3::Y), None);
    }

    #[test]
    fn player_input_is_a_despawnable_entity_component() {
        let mut world = World::new();
        let entity = world.spawn(PlayerInput::default()).id();

        assert!(world.despawn(entity));
        world.flush();
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn player_controller_snaps_downhill_without_ignoring_real_drops() {
        let controller = player_character_controller();

        assert_eq!(
            controller.snap_to_ground,
            Some(CharacterLength::Absolute(CHARACTER_GROUND_SNAP_DISTANCE))
        );
    }
}
