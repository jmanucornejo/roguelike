use crate::shared::gameplay::action_bar::{ActionBarBinding, ActionBarLayout};
use crate::shared::gameplay::components::{
    CharacterAttribute, CharacterId, CharacterStats, Facing, Health, Mana, MonsterKind, SpriteId,
};
use crate::shared::gameplay::items::{GroundItem, Inventory};
use crate::shared::gameplay::progression::{BaseProgression, JobProgression};
use crate::shared::gameplay::skills::{SkillId, SkillTree};
use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use serde::{Deserialize, Serialize};
// use crate::shared::enums::DamageType;

pub const CHARACTER_SLOT_COUNT: u8 = 9;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharacterSelectionSummary {
    pub id: u64,
    pub slot: u8,
    pub name: String,
    pub class_id: u16,
    pub base_level: u16,
    pub job_level: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountClientMessage {
    Login { username: String, password: String },
    CreateAccount { username: String, password: String },
    CreateCharacter { slot: u8, name: String },
    SelectCharacter { character_id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountServerMessage {
    CharacterList {
        username: String,
        characters: Vec<CharacterSelectionSummary>,
    },
    EnteringWorld,
    Error {
        message: String,
    },
}

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
        character_id: CharacterId,
        map_name: String,
        translation: [f32; 3],
        facing: Facing,
        health: Health,
        mana: Mana,
        progression: BaseProgression,
        job_progression: JobProgression,
        attack_speed: f32,
        sitting: bool,
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
    MovementRejected {
        entity: Entity,
        translation: [f32; 3],
        server_time: u128,
    },
    MovementInterrupted {
        entity: Entity,
        translation: [f32; 3],
        server_time: u128,
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
    ManaChange {
        entity: Entity,
        max: u32,
        current: u32,
    },
    ProgressionChanged {
        entity: Entity,
        progression: BaseProgression,
        job_progression: JobProgression,
    },
    PlayerDied {
        entity: Entity,
        experience_lost: u64,
    },
    PlayerRespawned {
        entity: Entity,
        map_name: String,
        translation: [f32; 3],
        health: Health,
        mana: Mana,
        server_time: u128,
    },
    MapChanged {
        entity: Entity,
        map_name: String,
        translation: [f32; 3],
        server_time: u128,
    },
    SpawnGroundItem {
        entity: Entity,
        item: GroundItem,
        translation: [f32; 3],
    },
    InventoryUpdated {
        entity: Entity,
        inventory: Inventory,
    },
    ItemPickedUp {
        entity: Entity,
    },
    EquipmentUpdated {
        entity: Entity,
        equipment: crate::shared::gameplay::components::Equipment,
    },
    ActionBarUpdated {
        action_bar: ActionBarLayout,
    },
    SkillTreeUpdated {
        skill_tree: SkillTree,
    },
    CharacterStatsUpdated {
        entity: Entity,
        stats: CharacterStats,
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
    SittingChanged {
        entity: Entity,
        sitting: bool,
    },
    FacingChanged {
        entity: Entity,
        facing: Facing,
    },
    SpellCastStarted {
        entity: Entity,
        spell_id: u16,
        target: Vec3,
        cast_time_ms: u32,
        facing: Facing,
    },
    SpellCastCompleted {
        entity: Entity,
        spell_id: u16,
        target: Vec3,
        cooldown_ms: u32,
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

/// Number of quantized positions placed in one unreliable message when
/// `batched_position_snapshots` is enabled. At the current bincode layout this
/// remains below Renet's 1,200-byte slice size, avoiding fragmented snapshots.
#[cfg(feature = "batched_position_snapshots")]
pub const MAX_POSITION_SNAPSHOTS_PER_BATCH: usize = 48;

#[cfg(feature = "batched_position_snapshots")]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuantizedEntityPosition {
    pub entity: Entity,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[cfg(feature = "batched_position_snapshots")]
impl From<&EntitySnapshot> for QuantizedEntityPosition {
    fn from(snapshot: &EntitySnapshot) -> Self {
        Self {
            entity: snapshot.entity,
            x: snapshot.x,
            y: snapshot.y,
            z: snapshot.z,
        }
    }
}

#[cfg(feature = "batched_position_snapshots")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntitySnapshotBatch {
    /// Shared by every position in this batch.
    pub server_time: u128,
    pub snapshots: Vec<QuantizedEntityPosition>,
}

#[derive(Debug, Serialize, Deserialize, Message)]
pub enum PlayerCommand {
    Move {
        destination_at: Vec3,
    },
    StopMoving,
    BasicAttack {
        entity: Entity,
        auto_attack: bool,
    },
    StopBasicAttack,
    ToggleSitting,
    Face {
        target: Vec3,
    },
    PickupItem {
        entity: Entity,
    },
    UseItem {
        item_id: crate::shared::gameplay::items::ItemDefinitionId,
    },
    EquipItem {
        item_id: crate::shared::gameplay::items::ItemDefinitionId,
    },
    UnequipItem {
        slot: crate::shared::gameplay::components::EquipmentSlot,
    },
    SetActionBarSlot {
        slot_index: u8,
        binding: Option<ActionBarBinding>,
    },
    SwapActionBarSlots {
        first_slot: u8,
        second_slot: u8,
    },
    CyclePlaceholderClass,
    SpendSkillPoint {
        skill_id: SkillId,
    },
    SpendAttributePoint {
        attribute: CharacterAttribute,
    },
    RespawnAtSavePoint,
    /// Temporary development command used to exercise authoritative map
    /// changes until map portals are authored.
    CycleMap,
    Cast {
        spell_id: u16,
        cast_at: Vec3,
        target_entity: Option<Entity>,
    },
}
