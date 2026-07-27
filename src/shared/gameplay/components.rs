use bevy::prelude::*;
use bevy_rapier3d::prelude::{CharacterLength, KinematicCharacterController, QueryFilterFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::shared::constants::{CHARACTER_CONTROLLER_OFFSET, CHARACTER_GROUND_SNAP_DISTANCE};
// use crate::shared::enums::DamageType;

/// Stable database identity for a player character.
///
/// Bevy's [`Entity`] and Renet's client id only identify a character for the
/// lifetime of a running process. This id survives reconnects and server restarts.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Component,
)]
pub struct CharacterId(pub u64);

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
