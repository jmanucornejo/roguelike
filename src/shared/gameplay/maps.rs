use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::shared::constants::{DEFAULT_CHARACTER_SPAWN, WATER_LEVEL};

pub const STARTING_MAP_NAME: &str = "prontera";
pub const SUN_TEMPLE_MAP_NAME: &str = "living_sun_temple_town";

/// Runtime information shared by the server and client for a playable map.
///
/// Maps are separated in the server physics world so their colliders can all
/// remain loaded at once. Network positions use these server-space coordinates;
/// persistence converts them back to map-local coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapDefinition {
    pub name: &'static str,
    pub asset_path: &'static str,
    pub server_origin: [f32; 3],
    pub spawn: [f32; 3],
    pub navigation_min: [i32; 2],
    pub navigation_max: [i32; 2],
    pub water_level: Option<f32>,
}

pub const MAP_DEFINITIONS: [MapDefinition; 2] = [
    MapDefinition {
        name: STARTING_MAP_NAME,
        asset_path: "terrain/bujama-3.glb#Scene0",
        server_origin: [0.0, 0.0, 0.0],
        spawn: DEFAULT_CHARACTER_SPAWN,
        navigation_min: [-150, -150],
        navigation_max: [150, 150],
        water_level: Some(WATER_LEVEL),
    },
    MapDefinition {
        name: SUN_TEMPLE_MAP_NAME,
        asset_path: "terrain/living_sun_temple_town.glb#Scene0",
        server_origin: [1000.0, 0.0, 0.0],
        // The terrain at the authored origin is roughly y=9.5. Starting a
        // little above it lets the character controller settle safely.
        spawn: [0.0, 12.0, 0.0],
        navigation_min: [-145, -140],
        navigation_max: [145, 140],
        water_level: None,
    },
];

#[derive(Component, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CurrentMap(pub String);

impl Default for CurrentMap {
    fn default() -> Self {
        Self(STARTING_MAP_NAME.to_string())
    }
}

pub fn map_definition(name: &str) -> &'static MapDefinition {
    MAP_DEFINITIONS
        .iter()
        .find(|definition| definition.name == name)
        .unwrap_or(&MAP_DEFINITIONS[0])
}

pub fn canonical_map_name(name: &str) -> &'static str {
    map_definition(name).name
}

pub fn map_server_origin(name: &str) -> Vec3 {
    Vec3::from_array(map_definition(name).server_origin)
}

pub fn map_spawn_position(name: &str) -> Vec3 {
    let definition = map_definition(name);
    Vec3::from_array(definition.server_origin) + Vec3::from_array(definition.spawn)
}

pub fn map_to_server_position(name: &str, local_position: Vec3) -> Vec3 {
    map_server_origin(name) + local_position
}

pub fn map_to_local_position(name: &str, server_position: Vec3) -> Vec3 {
    server_position - map_server_origin(name)
}

pub fn next_map_name(name: &str) -> &'static str {
    let index = MAP_DEFINITIONS
        .iter()
        .position(|definition| definition.name == name)
        .unwrap_or(0);
    MAP_DEFINITIONS[(index + 1) % MAP_DEFINITIONS.len()].name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_positions_round_trip_between_local_and_server_space() {
        let local = Vec3::new(2.0, 12.0, -4.0);
        let server = map_to_server_position(SUN_TEMPLE_MAP_NAME, local);

        assert_eq!(server, Vec3::new(1002.0, 12.0, -4.0));
        assert_eq!(map_to_local_position(SUN_TEMPLE_MAP_NAME, server), local);
    }

    #[test]
    fn unknown_persisted_maps_safely_fall_back_to_the_starting_map() {
        assert_eq!(canonical_map_name("removed_map"), STARTING_MAP_NAME);
        assert_eq!(map_definition("removed_map"), &MAP_DEFINITIONS[0]);
    }
}
