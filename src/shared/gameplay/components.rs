use bevy::prelude::*;
use bevy_rapier3d::prelude::{CharacterLength, KinematicCharacterController, QueryFilterFlags};
use serde::{Deserialize, Serialize};

use crate::shared::constants::{CHARACTER_CONTROLLER_OFFSET, CHARACTER_GROUND_SNAP_DISTANCE};
// use crate::shared::enums::DamageType;

#[derive(Clone, Debug, Eq, Hash, Ord, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Pos(pub i32, pub i32);

#[derive(Debug, Default, Resource)]
pub struct Map {
    pub blocked_paths: Vec<Pos>,
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

#[derive(Component, Reflect, Debug)]
pub struct Mana {
    pub max: u32,
    pub current: u32,
}

#[derive(Debug, Component)]
pub struct Building {
    pub blocked_paths: Vec<Pos>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Component, Clone)]
pub enum MonsterKind {
    Pig,
    Orc,
}

#[derive(Debug, Default, Component, Deserialize, Serialize, Clone)]
pub struct SpriteId(pub u16);

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Component, Clone)]
pub struct Facing(pub u8);

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
