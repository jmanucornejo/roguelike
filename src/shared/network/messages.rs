use crate::shared::gameplay::components::{Facing, Health, MonsterKind, SpriteId};
use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use serde::{Deserialize, Serialize};
// use crate::shared::enums::DamageType;

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientSyncMessages {
    Ping { client_time: u128 },
    SyncTimeRequest { client_time: u128 },
    LatencyRequest { client_time: u128 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerSyncMessages {
    Pong {
        client_time: u128,
        server_time: u128,
    },
    SyncTimeResponse {
        client_time: u128,
        server_time: u128,
    },
    LatencyResponse {
        client_time: u128,
    },
}

#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessages {
    PlayerCreate {
        entity: Entity,
        id: ClientId,
        translation: [f32; 3],
        server_time: u128,
    },
    SpawnMonster {
        entity: Entity,
        kind: MonsterKind,
        translation: [f32; 3],
        server_time: u128,
    },
    SpawnEntity {
        entity: Entity,
        sprite_id: SpriteId,
        translation: [f32; 3],
        facing: Facing,
        health: Option<Health>,
        server_time: u128,
    },
    PlayerRemove {
        id: ClientId,
    },
    DespawnEntity {
        entity: Entity,
    },
    /*HealingTick {
        entity: Entity,
        healing: u32
    },*/
    HealthChange {
        entity: Entity,
        amount: i32,
        max: u32,
        current: u32,
    },
    DamageNumber {
        entity: Entity,
        amount: i32,
    },
    Attack {
        entity: Entity,
        enemy: Entity,
        attack_speed: f32,
        auto_attack: bool,
    },
    AttackStopped {
        entity: Entity,
    },
    SpawnProjectile {
        entity: Entity,
        translation: [f32; 3],
    },
    DespawnProjectile {
        entity: Entity,
    },
}

/// An authoritative, independently useful position sample.
///
/// These samples use the unreliable entity channel. Because every packet contains an
/// absolute position and a server timestamp, losing or reordering one packet cannot
/// corrupt later movement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity: Entity,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub server_time: u128,
}

#[derive(Debug, Serialize, Deserialize, Message)]
pub enum PlayerCommand {
    Move { destination_at: Vec3 },
    BasicAttack { entity: Entity },
    Cast { cast_at: Vec3 },
}
