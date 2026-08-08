#[cfg(feature = "client")]
use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_platform::collections::hash_map::HashMap;
use bevy_rapier3d::prelude::*;

#[cfg(feature = "client")]
use crate::shared::gameplay::maps::map_definition;
use crate::shared::gameplay::{
    entities::MapEntity,
    maps::{CurrentMap, MapDefinition, MAP_DEFINITIONS, STARTING_MAP_NAME},
};

/// Loads every playable map into the authoritative, non-rendering server
/// physics world. Visual-only decorations and lighting are client concerns.
pub fn setup_server_level(mut commands: Commands, asset_server: Res<AssetServer>) {
    for definition in &MAP_DEFINITIONS {
        spawn_map_scene(&mut commands, &asset_server, definition);
    }
    spawn_server_starting_map_collision(&mut commands);
}

/// Replaces the rendered/client-side collision map with the one selected by
/// the authoritative server. Other maps remain loaded only on the server.
#[cfg(feature = "client")]
pub fn replace_client_map(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    existing_map_entities: &Query<Entity, With<MapEntity>>,
    map_name: &str,
) -> &'static str {
    for entity in existing_map_entities.iter() {
        commands.entity(entity).despawn();
    }

    let definition = map_definition(map_name);
    spawn_map_scene(commands, asset_server, definition);
    if definition.name == STARTING_MAP_NAME {
        spawn_starting_map_decorations(commands, meshes, materials, asset_server);
    }
    info!("Client loaded map '{}'", definition.name);
    definition.name
}

#[cfg(feature = "client")]
pub fn setup_world_lighting(mut commands: Commands) {
    spawn_world_lighting(&mut commands);
}

fn spawn_map_scene(
    commands: &mut Commands,
    asset_server: &AssetServer,
    definition: &MapDefinition,
) {
    let scene = asset_server.load(definition.asset_path);
    commands
        .spawn((
            WorldAssetRoot(scene),
            Transform::from_translation(Vec3::from_array(definition.server_origin)),
            Name::new(format!("Map: {}", definition.name)),
            MapEntity,
            CurrentMap(definition.name.to_string()),
            RigidBody::Fixed,
        ))
        .insert(AsyncSceneCollider {
            shape: Some(ComputedColliderShape::TriMesh(TriMeshFlags::default())),
            named_shapes: HashMap::default(),
        });
}

fn spawn_server_starting_map_collision(commands: &mut Commands) {
    commands.spawn((
        Transform::from_xyz(0.0, 0.99, 0.0),
        Name::new("Wall Collision"),
        MapEntity,
        CurrentMap(STARTING_MAP_NAME.to_string()),
        Collider::cuboid(1.5, 2.5, 5.5),
        RigidBody::Fixed,
    ));
}

#[cfg(feature = "client")]
fn spawn_starting_map_decorations(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    let map = CurrentMap(STARTING_MAP_NAME.to_string());
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(3.0, 5.0, 11.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, 0.99, 0.0),
        Name::new("Wall"),
        MapEntity,
        map.clone(),
        Collider::cuboid(1.5, 2.5, 5.5),
        RigidBody::Fixed,
    ));

    let tree = asset_server.load("models/palm_tree.glb#Scene0");
    for (translation, scale) in [
        (Vec3::new(20.0, -1.0, 20.0), 0.7),
        (Vec3::new(10.0, -1.0, 18.0), 0.5),
    ] {
        commands.spawn((
            WorldAssetRoot(tree.clone()),
            Transform {
                translation,
                scale: Vec3::splat(scale),
                ..default()
            },
            Name::new("Palm tree"),
            MapEntity,
            map.clone(),
        ));
    }
}

#[cfg(feature = "client")]
fn spawn_world_lighting(commands: &mut Commands) {
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.0),
            ..default()
        },
        Name::new("World Sun"),
    ));
}
