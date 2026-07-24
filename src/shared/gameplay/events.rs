use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum HealthChangeType {
    Normal,
    Critical,
}

#[derive(Event, Debug)]
pub struct HealthChange {
    pub entity: Entity,
    pub source: Option<Entity>,
    pub amount: i32,
    pub damage: u32,
    pub damage_type: HealthChangeType,
}

#[derive(Event, Debug)]
pub struct DeathEvent {
    pub entity: Entity,
    pub killer: Option<Entity>, // optional: who caused the death
}
