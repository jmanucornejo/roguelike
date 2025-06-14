pub mod pathing;
pub mod monsters;
pub mod client_plugins;
pub mod server_plugins;
pub mod shared;

// use avian3d::{parry::shape, prelude::*};
use bevy_spatial::{kdtree::KDTree3};
use bevy::{prelude::*, render::render_resource::{AsBindGroup, ShaderRef}};
use bevy_platform::collections::hash_map::HashMap;
use bevy_renet::renet::*;
use serde::{Deserialize, Serialize};
use shared::components::*;
use std::{f32::consts::PI, time::Duration};
use bevy::render::render_resource::{ShaderStages, ShaderType};
use bevy::reflect::TypePath;

use bevy_rapier3d::prelude::*;



#[derive(Component, Debug)]
pub struct PrevState {
    pub translation: Vec3,
    pub rotation: Facing
}



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





#[derive(Debug, Component)]
pub struct Player {
    pub id: ClientId,
}

#[derive(Debug, Component)]
pub struct NPC {
    pub id: ClientId,
}



#[derive(Component)]
pub struct NearestNeighbourComponent;

pub type NNTree = KDTree3<NearestNeighbourComponent>;


#[derive(Debug, PartialEq, Serialize, Deserialize, Component, Clone)]
pub struct AttackSpeed(pub f32);


#[derive(Component, Debug)]
pub struct MapEntity; 



#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NetworkedEntities {
    pub entities: Vec<Entity>,
    pub translations: Vec<[f32; 3]>,
}






pub fn setup_level(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,  
    asset_server: Res<AssetServer>,
    mut _graphs: ResMut<Assets<AnimationGraph>>,
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

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(3., 5., 11.)))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, 0.99, 0.0),
        /*PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(3., 5., 11.))),
            //material: material,
            material: materials.add(Color::srgb(0.3, 0.5, 0.3)),
            transform: Transform::from_xyz(0.0, 0.99, 0.0),
            ..Default::default()
        },  */
        Name::new("Wall"),  
        MapEntity,
        // Rapier3d Settings
        Collider::cuboid(1.5, 2.5, 5.5),
        RigidBody::Fixed
        //Avian3s Settings
        //Collider::cuboid(3., 5., 11.),
        //RigidBody::Static,
    
    ));

    
    /*commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(11., 5., 1.))),
            //material: material,
            material: materials.add(Color::srgb(0.3, 0.5, 0.3)),
            transform: Transform::from_xyz(1., 0.99, 6.),
            ..Default::default()
        },  
        Name::new("Wall"),  
        MapEntity,
        Collider::cuboid(11., 5., 1.),
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

    /*
    let wall_handle = asset_server.load("models/wall_door_-_19mb2.glb#Scene0");

    commands.spawn((      
        SceneBundle {
            scene: wall_handle.clone(),
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                scale: Vec3::splat(0.7),
                //rotation,
                ..Default::default()
            },
            
            ..Default::default()
        },
        Name::new("Wall"),
        MapEntity,
        ColliderConstructorHierarchy::new(ColliderConstructor::ConvexDecompositionFromMesh) 
        )  
    );*/

    let tree_handle = asset_server.load("models/palm_tree.glb#Scene0");

    commands.spawn((    
        SceneRoot(tree_handle.clone()),
        Transform {
            translation: Vec3::new(20.0, -1.0, 20.0),
            scale: Vec3::splat(0.7),
            //rotation,
            ..Default::default()
        },   
        /*SceneBundle {
            scene: tree_handle.clone(),
            transform: Transform {
                translation: Vec3::new(20.0, -1.0, 20.0),
                scale: Vec3::splat(0.7),
                //rotation,
                ..Default::default()
            },
            
            ..Default::default()
        },*/
        Name::new("Palm tree"),
        MapEntity,
        //ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh) 
        )  
    );

    commands.spawn((    
        SceneRoot(tree_handle.clone()),
        Transform {
            translation: Vec3::new(10.0, -1.0, 18.0),
            scale: Vec3::splat(0.5),
            //rotation,
            ..Default::default()
        },  
        /*SceneBundle {
            scene: tree_handle.clone(),
            transform: Transform {
                translation: Vec3::new(10.0, -1.0, 18.0),
                scale: Vec3::splat(0.5),
                //rotation,
                ..Default::default()
            },
            
            ..Default::default()
        },*/
        Name::new("Palm tree"),
        MapEntity,
        //ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh) 
        )  
    );




    /*let blood_handle = asset_server.load("models/light_beam.glb#Scene0");

    commands.spawn((      
        SceneBundle {
            scene: blood_handle.clone(),
            transform: Transform {
                translation: Vec3::new(10.0, -1.0, 10.0),
                scale: Vec3::splat(1.0),
                //rotation,
                ..Default::default()
            },
            
            ..Default::default()
        },
        Name::new("Blood"),
        MapEntity,
        //ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh) 
        )  
    );*/


    // Load textures
    /*let black_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5514_seamless_2.jpg.png");
    let red_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5487_seamless.jpg");
    let green_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5498_seamless.jpg");
    let blue_texture_handle: Handle<Image> = asset_server.load("textures/IMGP5525_seamless.jpg");

     // Load the RGB mask
     let mask_texture_handle: Handle<Image> = asset_server.load("terrain/terrain_mask_RGB.png");


     let shader_handle: Handle<Shader> = asset_server.load("shaders/bujama.wgsl");

    let scene_handle: Handle<Scene> = asset_server.load("terrain/bujama.glb#Scene0");*/
    

    let scene_handle: Handle<Scene> = asset_server.load("terrain/bujama-3.glb#Scene0");
    /*let mesh: Handle<Mesh> = asset_server.load("terrain/bujama-3.gltf#Scene0");
    println!("mesh: {:?}", mesh);
    
    let m = &meshes.get(&mesh);
    println!("m: {:?}", m);
    let collider = Collider::from_bevy_mesh(m.unwrap(), &ComputedColliderShape::TriMesh).unwrap();*/
    //let scene_handle: Handle<Scene> = asset_server.load("terrain/bujama.glb#Scene0");
    commands.spawn((
        SceneRoot(scene_handle.clone()),
        Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            //scale: Vec3::splat(25.0),
            //rotation,
            ..Default::default()
        },

        /*SceneBundle {
            scene: scene_handle.clone(),
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                //scale: Vec3::splat(25.0),
                //rotation,
                ..Default::default()
            },
            ..Default::default()
        },*/
        Name::new("Map"),
        MapEntity,
        RigidBody::Fixed,
        //Collider::from_bevy_mesh(m.unwrap(), &ComputedColliderShape::TriMesh)
        //ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
        //RigidBody::Static
    )).insert(AsyncSceneCollider {
       // handle: scene_handle,
        // `TriMesh` gives us the most accurate collisions, at the cost of
        // physics complexity.
        shape: Some(ComputedColliderShape::TriMesh(TriMeshFlags::default())),
        named_shapes: HashMap::default(),
    });
    
    //.insert(collider);



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
    commands.spawn((
         DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform {
            translation:  Vec3::new(0.0, 2.0, 0.0),
             rotation: Quat::from_rotation_x(-PI / 4.),
            //rotation,
            ..Default::default()
        }
    ));

    /*commands.spawn(DirectionalLightBundle {
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
    });*/


}

// Update the time in the water shader
pub fn  move_water(
    time: Res<Time>, 
    mut water_materials: ResMut<Assets<WaterMaterial>>,
) {
    for  water in water_materials.iter_mut() {
        water.1.time += time.delta_secs();
    }
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
      
        .spawn((
            Mesh3d(meshes.add(Sphere { radius: 0.1 })),
            MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
            Transform::from_translation(translation),
            /*PbrBundle {
                mesh: meshes.add(Sphere { radius: 0.1 }),
                material: materials.add(Color::srgb(1.0, 0.0, 0.0)),
                transform: Transform::from_translation(translation),
                ..Default::default()
            }*/
        ))
        .insert(GameVelocity(direction * 10.))
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