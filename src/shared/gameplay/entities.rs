use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use serde::{Deserialize, Serialize};

use super::components::Facing;

#[derive(Component, Debug)]
pub struct TargetState {
    pub translation: Vec3,
    pub rotation: Facing,
}

#[derive(Component, PartialEq)]
pub struct MovementDelta {
    pub translation: IVec3,
    pub rotation: Facing,
    pub server_time: u128,
    pub real_translation: [f32; 3],
}

#[derive(Debug, Component)]
pub struct Player {
    pub id: ClientId,
}

#[derive(Debug, Component)]
pub struct NPC {
    pub id: ClientId,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Component, Clone)]
pub struct AttackSpeed(pub f32);

#[derive(Component, Debug)]
pub struct MapEntity;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NetworkedEntities {
    pub entities: Vec<Entity>,
    pub translations: Vec<[f32; 3]>,
}
