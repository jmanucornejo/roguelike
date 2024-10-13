pub mod pathing;
pub mod monsters;
pub mod client_plugins;
pub mod server_plugins;

use avian3d::prelude::*;
use bevy_spatial::{kdtree::KDTree3};
use bevy::{prelude::*, render::render_resource::{AsBindGroup, ShaderRef}};
use bevy_renet::renet::*;
use bevy_renet::*;
use serde::{Deserialize, Serialize};
use std::{f32::consts::PI, time::Duration};
use bevy::render::render_resource::{ShaderStages, ShaderType};
use bevy::reflect::TypePath;



// 0.14 (Solution 1)
#[derive(States, Default, Hash, Debug, PartialEq, Clone, Eq)]
pub enum AppState {
    // Make this the default instead of `InMenu`.
    #[default]
    Setup,
    _InMenu,
    InGame,
}


pub const PLAYER_MOVE_SPEED: f32 = 10.0;
pub const LINE_OF_SIGHT: f32 = 12.0;
pub const TRANSLATION_PRECISION: f32 = 0.001;

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    SyncTimeRequest  {
        client_time: u128
    },
    LatencyRequest  {
        client_time: u128
    },
}




#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    SyncTimeResponse  {
        client_time: u128,
        server_time: u128
    },
    LatencyResponse  {
        client_time: u128
    }
}

#[derive(Debug, Default, Component)]
pub struct Velocity(pub Vec3);


#[derive(Component, Debug)]
pub struct PrevState {
    pub translation: Vec3,
    pub rotation: Facing
}

#[derive(Debug, Default, Component, Deserialize, Serialize,Clone)]
pub struct SpriteId(pub u16);


#[derive(Debug, Default, Component)]
pub struct LineOfSight(pub Vec<Entity>);

#[derive(Debug, Default, Component)]
pub struct SeenBy(pub Vec<Entity>);

#[derive(Component, Debug)]
pub struct TargetState {
    pub translation: Vec3,
    pub rotation: Facing
}
#[derive(Component,  PartialEq)]

pub struct MovementDelta {
    pub translation: IVec3,
    pub rotation: Facing,
    pub server_time: u128,
    pub real_translation: [f32; 3]
}





#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Component, Clone)]
pub struct Facing(pub i8);

#[derive(Debug, Component)]
pub struct Player {
    pub id: ClientId,
}

#[derive(Debug, Component)]
pub struct NPC {
    pub id: ClientId,
}


#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Component, Resource)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub destination_at: Option<Pos>
}

#[derive(Debug, Serialize, Deserialize, Event)]
pub enum PlayerCommand {
    Move { destination_at: Vec3 },
    BasicAttack { cast_at: Vec3 },
}

pub enum ClientChannel {
    Input,
    Command,
    SyncTimeRequest
}
pub enum ServerChannel {
    ServerMessages,
    NetworkedEntities,
    SyncTimeResponse
}


#[derive(Component)]
pub struct NearestNeighbourComponent;

pub type NNTree = KDTree3<NearestNeighbourComponent>;


#[derive(Debug, PartialEq, Component, Clone)]
pub struct Monster {
    pub hp: i32,
    //pub speed: f32,
    pub kind: MonsterKind,
   // pub move_destination: Vec3,
    //pub move_timer: Timer
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Component, Clone)]
pub enum MonsterKind {
    Pig,
    Orc,
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
    MoveDelta {
        entity: Entity,
        x: i32,
        y: i32,
        z: i32,
        server_time: u128,
        //real_translation: [f32; 3],
    },
    SpawnProjectile {
        entity: Entity,
        translation: [f32; 3],
    },
    DespawnProjectile {
        entity: Entity,
    },
}


#[derive(Component)]
pub struct MonsterParent; 

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NetworkedEntities {
    pub entities: Vec<Entity>,
    pub translations: Vec<[f32; 3]>,
}

pub const PROTOCOL_ID: u64 = 1000;



impl From<ClientChannel> for u8 {
    fn from(channel_id: ClientChannel) -> Self {
        match channel_id {
            ClientChannel::Command => 0,
            ClientChannel::Input => 1,
            ClientChannel::SyncTimeRequest => 2,
        }
    }
}

impl ClientChannel {
    pub fn channels_config() -> Vec<ChannelConfig> {
        vec![
            ChannelConfig {
                channel_id: Self::Input.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::ZERO,
                },
            },
            ChannelConfig {
                channel_id: Self::Command.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::ZERO,
                },
            },
            ChannelConfig {
                channel_id: Self::SyncTimeRequest.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::Unreliable
            },
        ]
    }
}

impl From<ServerChannel> for u8 {
    fn from(channel_id: ServerChannel) -> Self {
        match channel_id {
            ServerChannel::NetworkedEntities => 0,
            ServerChannel::ServerMessages => 1,
            ServerChannel::SyncTimeResponse => 2,
        }
    }
}

impl ServerChannel {
    pub fn channels_config() -> Vec<ChannelConfig> {
        vec![
            ChannelConfig {
                channel_id: Self::NetworkedEntities.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: Self::ServerMessages.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::SyncTimeResponse.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::Unreliable
            },
        ]
    }
}


pub fn connection_config() -> ConnectionConfig {
    ConnectionConfig {
        available_bytes_per_tick: 1024 * 1024,
        client_channels_config: ClientChannel::channels_config(),
        server_channels_config: ServerChannel::channels_config(),
    }
}



pub fn setup_level(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,  
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    asset_server: Res<AssetServer>,
) {

    

    // Load the texture
    //let texture_handle = asset_server.load("textures/grass/grass1-albedo3.png");


    // Create a material with the texture
    /*let material = materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle),
        ..Default::default()
    });*/
    // plane
    /*commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(301., 0.5, 301.))),
            //material: material,
            material: materials.add(Color::srgb(0.3, 0.5, 0.3)),
            transform: Transform::from_xyz(0.0, 0.99, 0.0),
            ..Default::default()
        },  Name::new("Plane"),  
        ColliderConstructor::TrimeshFromMesh,
        RigidBody::Static,
    ));*/
  

     // Create a plane to represent the water surface
     //let water_mesh = meshes.add(Plane3d::default().mesh().size(50.0, 50.0));


        // Load the custom shader
    // let shader_handle: Handle<Shader> = asset_server.load("shaders/water.wgsl");

     // Add a custom material (we'll create this next)
     /*let water_material = materials.add(StandardMaterial {
         base_color: Color::srgba(0.0, 0.3, 0.6, 0.7), // Transparent blue color for water
         reflectance: 0.5,  // Make it slightly reflective
         perceptual_roughness: 0.1,  // Lower roughness for a more reflective, glossy surface
         metallic: 0.1,  // Water tends to have a little bit of a metallic reflection
         ..Default::default()
     });

    // Create a material using the shader
    let water_material = water_materials.add(WaterMaterial { time: 0.0 });

    Cuboid::default();

    commands.spawn((MaterialMeshBundle { 
        mesh: meshes.add(Mesh::from(Cuboid::new(31., 0.0, 31.))),
        transform: Transform::from_xyz(10.0, 2., 10.0),
        material: water_materials.add(WaterMaterial {
            time: 0.5 
        }),
        ..default()
    },  Name::new("Water")));*/


    let tree_handle = asset_server.load("models/palm_tree.glb#Scene0");

    commands.spawn(SceneBundle {
        scene: tree_handle.clone(),
        transform: Transform {
            translation: Vec3::new(20.0, 0.0, 20.0),
            scale: Vec3::splat(0.7),
            //rotation,
            ..Default::default()
        },
        ..Default::default()
    });


    // Load textures
    /*let black_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5514_seamless_2.jpg.png");
    let red_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5487_seamless.jpg");
    let green_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5498_seamless.jpg");
    let blue_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5525_seamless.jpg");

     // Load the RGB mask
     let mask_texture_handle: Handle<Image> = asset_server.load("terrain/terrain_mask_RGB.png");


     let shader_handle: Handle<Shader> = asset_server.load("shaders/bujama.wgsl");

    let scene_handle: Handle<Scene> = asset_server.load("terrain/bujama.glb#Scene0");*/
    
    let scene_handle: Handle<Scene> = asset_server.load("terrain/bujama-2.glb#Scene0");
    //let scene_handle: Handle<Scene> = asset_server.load("terrain/bujama.glb#Scene0");
    commands.spawn((
        SceneBundle {
            scene: scene_handle.clone(),
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                //scale: Vec3::splat(25.0),
                //rotation,
                ..Default::default()
            },
            ..Default::default()
        },
        Name::new("Map"),
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
        RigidBody::Static
    ));

    /*commands.spawn((
        RigidBody::Dynamic,
        Collider::cuboid(1.0, 1.0, 1.0),
        Mass(5.0),
        PbrBundle {
            mesh: meshes.add(Cuboid::default()),
            material: materials.add(Color::srgb(0.8, 0.7, 0.6)),
            transform: Transform::from_xyz(3.0, 5.0, 5.0),
            ..default()
        },
        GravityScale(1.0),
    ));*/


   /* commands.spawn((PbrBundle {
        mesh: meshes.add(Mesh::from(Cuboid::new(5., 4.0, 5.))),
        material: materials.add(Color::srgb(0.3, 0.0, 0.3)),
        transform: Transform::from_xyz(0.0, 0.99, 0.0),
        ..Default::default()
    },  
    Name::new("Box")))
    .insert(
        Building { 
            blocked_paths:  vec![
                Pos(2,2), Pos(2,1), Pos(2,0), Pos(2,-1), Pos(2,-2), 
                Pos(1,2), Pos(1,1), Pos(1,0), Pos(1,-1), Pos(1,-2),
                Pos(0,2), Pos(0,1), Pos(0,0), Pos(0,-1), Pos(0,-2),
                Pos(-1,2), Pos(-1,1), Pos(-1,0), Pos(-1,-1), Pos(-1,-2), 
                Pos(-2,2), Pos(-2,1), Pos(-2,0), Pos(-2,-1), Pos(-2,-2)
            ] 
        }
    );*/

    // light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        ..default()
    });


}

// Update the time in the water shader
pub fn  move_water(
    time: Res<Time>, 
    mut water_materials: ResMut<Assets<WaterMaterial>>,
) {
    for mut water in water_materials.iter_mut() {
        water.1.time += time.delta_seconds();
    }
}

#[derive(Debug, Default, Resource)]
pub struct Map {
    pub blocked_paths: Vec<Pos>
}

#[derive(Clone, Debug, Eq, Hash, Ord,  Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Pos(pub i32, pub i32);

#[derive(Debug, Component)]
pub struct Building {
    pub blocked_paths: Vec<Pos>,
}



#[derive(Debug, Component)]
pub struct Projectile {
    pub duration: Timer,
}

pub fn spawn_fireball(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    translation: Vec3,
    mut direction: Vec3,
) -> Entity {
    if !direction.is_normalized() {
        direction = Vec3::X;
    }
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere { radius: 0.1 }),
            material: materials.add(Color::srgb(1.0, 0.0, 0.0)),
            transform: Transform::from_translation(translation),
            ..Default::default()
        })
        .insert(Velocity(direction * 10.))
        .insert(Projectile {
            duration: Timer::from_seconds(1.5, TimerMode::Once),
        })
        .id()
}


#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]

pub struct WaterMaterial {
    #[uniform(0)]
    time: f32
}

impl Material for WaterMaterial {

    fn vertex_shader() -> ShaderRef {
        "shaders/water2.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/water2.wgsl".into()
    }

    
}