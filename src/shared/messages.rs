use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use crate::shared::components::{Facing, MonsterKind, SpriteId};
// use crate::shared::enums::DamageType;

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientSyncMessages {
    Ping {
        client_time: u128,
    },
    SyncTimeRequest  {
        client_time: u128
    },
    LatencyRequest  {
        client_time: u128
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerSyncMessages {
    Pong {
        client_time: u128,
        server_time: u128,
    },
    SyncTimeResponse  {
        client_time: u128,
        server_time: u128
    },
    LatencyResponse  {
        client_time: u128
    }
}





#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessages {
    PlayerCreate {
        entity: Entity,
        id: ClientId,
        translation: [f32; 3],
        server_time: u128
    },
    SpawnMonster {
        entity: Entity,
        kind: MonsterKind,
        translation: [f32; 3],
        server_time: u128
    },
    SpawnEntity {
        entity: Entity,
        sprite_id: SpriteId,
        translation: [f32; 3],
        facing: Facing
    },
    PlayerRemove {
        id: ClientId,
    },
    DespawnEntity {
        entity: Entity,
    },
    #[cfg(feature = "absolute_interpolation")]
    MoveAbsolute {
        entity: Entity,
        x: i32,
        y: i32,
        z: i32,
        server_time: u128,
        //real_translation: [f32; 3],
    },
    #[cfg(not(feature = "absolute_interpolation"))]
    MoveDelta {
        entity: Entity,
        x: i32,
        y: i32,
        z: i32,
        server_time: u128,
        //real_translation: [f32; 3],
    },
    /*HealingTick {
        entity: Entity,
        healing: u32
    },*/
    DamageTick {
        entity: Entity,
        damage: u32,
        damage_type: crate::server_plugins::combat::DamageType
    },
    HealthChange {
        entity: Entity,
        max: u32,
        current: u32,
    },
    Attack {
        entity: Entity,
        enemy: Entity,
        attack_speed: f32,
        auto_attack: bool        
    },
    SpawnProjectile {
        entity: Entity,
        translation: [f32; 3],
    },
    DespawnProjectile {
        entity: Entity,
    },
}


#[derive(Debug, Serialize, Deserialize, Event)]
pub enum PlayerCommand {
    Move { destination_at: Vec3 },
    BasicAttack { entity: Entity },
    Cast { cast_at: Vec3 }
}
