use serde::{Deserialize, Serialize};
use bevy::prelude::*;


#[derive(Event, Debug)]
pub struct DeathEvent {
    pub entity: Entity,
    pub killer: Option<Entity>, // optional: who caused the death
}
