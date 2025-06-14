use serde::{Deserialize, Serialize};
use bevy::prelude::*;
// use crate::shared::enums::DamageType;

#[derive(Clone, Debug, Eq, Hash, Ord,  Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Pos(pub i32, pub i32);

#[derive(Debug, Default, Resource)]
pub struct Map {
    pub blocked_paths: Vec<Pos>
}



#[derive(Debug, Default, Component)]
pub struct GameVelocity(pub Vec3);


#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Component, Resource)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub destination_at: Option<Pos>
}

#[derive(Debug, PartialEq, Component, Clone)]
pub struct AttackingTimer(pub Timer);

#[derive(Component, Reflect, Debug)]
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

#[derive(Debug, Default, Component, Deserialize, Serialize,Clone)]
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
    pub enemy_translation: Vec3
}



#[derive(Debug, PartialEq, Serialize, Deserialize, Component, Clone)]
pub struct Walking {
    pub target_translation: Vec3,
    pub path: Option<(Vec<Pos>, u32)>
}


#[derive(Debug, PartialEq, Component, Clone)]
pub struct Attacking {
    pub enemy: Entity,
    pub auto_attack: bool,
    //pub enemy_translation: Vec3,
    // pub timer: Timer
}

