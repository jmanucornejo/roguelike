
// use avian3d::math::Scalar;
// use avian3d::prelude::*;
use bevy_atmosphere::plugin::*;
use bevy_sprite3d::*;
use bevy_obj::ObjPlugin;
use local_ip_address::local_ip;
use client_plugins::interpolation::*;
use client_plugins::pointer::*;
use client_plugins::client_clock_sync::*;
use client_plugins::shared::*;

use bevy_health_bar3d::configuration::ForegroundColor;
use bevy_health_bar3d::prelude::{
    BarHeight, BarSettings, ColorScheme, HealthBarPlugin, Percentage,
};

use roguelike::*;

use bevy::{asset::LoadState, input::mouse::MouseWheel, log::LogPlugin, pbr::NotShadowCaster, prelude::*, render::render_resource::Texture, window::{PrimaryWindow, Window, WindowResolution}};
pub use bevy_renet::renet::transport::ClientAuthentication;
use bevy_renet::{renet::*, transport::NetcodeClientPlugin};
use bevy_renet::*;
use bevy_renet::renet::transport::NetcodeClientTransport;
use std::f32::consts::TAU;
use std::{
    collections::{HashMap, VecDeque}, net::{SocketAddr, UdpSocket}, time::{Duration, SystemTime}
};
use bevy_asset_loader::prelude::*;

use bevy_inspector_egui::prelude::ReflectInspectorOptions;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_inspector_egui::InspectorOptions;
use bevy::input::common_conditions::input_toggle_active;
// use smooth_bevy_cameras::{LookTransform, LookTransformBundle, LookTransformPlugin, Smoother};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin, TouchControls};
use bevy_rapier3d::prelude::*;


#[derive(Component)]
struct Billboard;

#[derive(Component)]
struct Hovered;



#[derive(Debug)]
struct PlayerInfo {
    client_entity: Entity,
    server_entity: Entity,
}

#[derive(Debug, Default, Resource)]
struct ClientLobby {
    players: HashMap<ClientId, PlayerInfo>,
}

#[derive(Debug, Resource)]
struct CurrentClientId(u64);





#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);




#[derive(AssetCollection, Resource, Debug)]
struct SealAssets {
    #[asset(texture_atlas_layout(tile_size_x = 64, tile_size_y = 67, columns = 8, rows = 8, padding_x = 0, padding_y = 0, offset_x = 0, offset_y = 0))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "spritesheets/monsters/seal.png")]
    sprite: Handle<Image>,    
}


#[derive(AssetCollection, Resource)]
struct MyAssets {
    #[asset(texture_atlas_layout(tile_size_x = 24, tile_size_y = 24, columns = 7, rows = 1, padding_x = 0, padding_y = 0, offset_x = 0, offset_y = 0))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "gabe-idle-run.png")]
    sprite: Handle<Image>,

    
}


#[derive(AssetCollection, Resource, Debug)]
struct PigAssets {
    #[asset(texture_atlas_layout(tile_size_x = 24, tile_size_y = 16, columns = 1, rows = 1, padding_x = 0, padding_y = 0, offset_x = 0, offset_y = 0))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "pig.png")]
    sprite: Handle<Image>,

    
}


#[derive(AssetCollection, Resource)]
struct SkyboxAssets {
    #[asset(path = "skyboxes/Ryfjallet_cubemap.png")]
    sprite: Handle<Image>,
}



#[derive(AssetCollection, Resource)]
struct ChaskiAssets {
    #[asset(texture_atlas_layout(tile_size_x = 128, tile_size_y = 128, columns = 8, rows = 8, padding_x = 0, padding_y = 0, offset_x = 0, offset_y = 0))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "spritesheets/chasqui/chasqui.png")]
    sprite: Handle<Image>,
}


fn main() {
    let mut app: App = App::new();
     
    app.add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "info,wgpu_core=warn,wgpu_hal=off,rechannel=warn".into(),
            level: bevy::log::Level::DEBUG,
            ..Default::default()
        }).set(WindowPlugin {
            primary_window: Some(Window  {
                resolution: WindowResolution::new(720., 720.),
                title: "Tribute Client".to_string(),
                resizable: false,
                ..default()
            }),
            ..default()   
        }))
        .init_state::<AppState>()
        .add_loading_state(
            LoadingState::new(AppState::Setup)
                .continue_to_state(AppState::InGame)
                .load_collection::<MyAssets>()
                .load_collection::<PigAssets>()
                .load_collection::<SealAssets>()
           
                .load_collection::<ChaskiAssets>()
                .load_collection::<SkyboxAssets>()
        )
        /*.add_plugins(  
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
        )*/
        .add_plugins(ObjPlugin) 
        .add_plugins((PanOrbitCameraPlugin, MaterialPlugin::<WaterMaterial>::default()))
        // .add_plugins(LookTransformPlugin)
        .add_plugins(AtmospherePlugin)
        //.add_plugins(DefaultPlugins)
        .add_plugins(RenetClientPlugin)        
        .insert_resource(PlayerInput::default())
        .insert_resource(ClientLobby::default())
        //.insert_resource(avian3d::prelude::SpatialQueryPipeline::default())
        /* .add_plugins((
            PhysicsPlugins::default(),
        ))*/
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        //.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().with_default_system_setup(false))
        .add_plugins(RapierDebugRenderPlugin {           
            ..default()
        })
        .insert_resource(Map::default())
        .insert_resource(NetworkMapping::default())
        .insert_resource(ServerTime::default())
        .insert_resource(ClockOffset::default())
        .add_event::<PlayerCommand>()
        .add_plugins(NetcodeClientPlugin)   
      
        .add_systems(OnEnter(AppState::InGame), (setup_level, setup_camera, move_water))
        .add_plugins(Sprite3dPlugin)
        .add_plugins((
            InterpolationPlugin, 
            ClientClockSyncPlugin,
            //client_plugins::music::MusicPlugin, 
            client_plugins::pointer::PointerPlugin,
            client_plugins::health::HealthPlugin,
            //client_plugins::water::WaterPlugin,
        ))
        // .add_plugins(PathingPlugin)
      
        .add_systems(Update, 
            (
               
                // print_hits.run_if(in_state(AppState::InGame)).after(update_cursor_system),
     
                //camera_zoom.run_if(in_state(AppState::InGame)),
                client_send_input.run_if(in_state(AppState::InGame)),              
                
                client_send_player_commands.run_if(in_state(AppState::InGame)),
                billboard.run_if(in_state(AppState::InGame)),
               
            )
        )  
        .add_systems(
            FixedUpdate, (        
                
                client_sync_players.run_if(in_state(AppState::InGame)),
                draw_player_sprites.run_if(in_state(AppState::InGame)).after(client_sync_players),
                camera_follow.run_if(in_state(AppState::InGame)),
                sprite_movement.run_if(in_state(AppState::InGame)),

            )
        );
            
    create_renet_transport(&mut app);
       
    app.run();
}




fn create_renet_transport(app: &mut App)  {

    // create client
    let client = RenetClient::new(connection_config());
    app.insert_resource(client);

    let current_time = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap();

    let client_id = current_time.as_millis() as u64;

    let server_addr = SocketAddr::new(local_ip().unwrap(), 42069);

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();

    let transport: NetcodeClientTransport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
   
    app.insert_resource(transport);
    app.insert_resource(CurrentClientId(client_id));

}

fn client_send_input(
    player_input: Res<PlayerInput>, 
    mut client: ResMut<RenetClient>
) {
    let input_message = bincode::serialize(&*player_input).unwrap();

    // info!("Sent input message {:?}!", input_message );
    client.send_message(ClientChannel::Input, input_message);
}


fn client_send_player_commands(mut player_commands: EventReader<PlayerCommand>, mut client: ResMut<RenetClient>) {
    for command in player_commands.read() {
        let command_message = bincode::serialize(command).unwrap();

        info!("Sent command message {:?}!", command_message );
        client.send_message(ClientChannel::Command, command_message);
    }
}

fn draw_player_sprites( 
    mut commands: Commands,
    mut sprite_params : Sprite3dParams,  
    mut entities: Query<(Entity, &Transform), ( Or<(Added<Player>, Added<ControlledPlayer>)>)>,
    chaski: Res<ChaskiAssets>,
){
    for (entity, transform,) in entities.iter_mut() {   

        let texture_atlas = TextureAtlas {
            layout: chaski.layout.clone(),
            index: 0,
        };
        
        let sprite_entity = commands.spawn(
            (
                       
                Sprite3d {
                    image: chaski.sprite.clone(),
                    pixels_per_metre: 48.,
                    //pixels_per_metre: 128.,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    transform: Transform::from_xyz(0., -1.0, 0.),
                    // transform: Transform::from_xyz(0., 0., 0.),
                    //pivot: Some(Vec2::new(0.5, 0.5)),
                    pivot: Some(Vec2::new(0.5, 0.)), // para que gire sobre los pies y no del centro.
                    ..default()
                }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()),
                Name::new("PlayerSprite"),
                Billboard
            )).id();

        commands.entity(entity).push_children(&[sprite_entity]);

        println!("Draw player sprite {:?}", transform);     


    }
}
  

pub fn client_sync_players(
    mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
    //mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    client_id: Res<CurrentClientId>,
    mut lobby: ResMut<ClientLobby>,
    mut network_mapping: ResMut<NetworkMapping>,
    assets            : Res<MyAssets>,
    chaski            : Res<ChaskiAssets>,
    pig_assets            : Res<PigAssets>,
    seal_assets            : Res<SealAssets>,
    mut sprite_params : Sprite3dParams,       
    mut entities: Query<(Entity, &Transform, &mut PositionHistory)>, 
    mut server_time_res: ResMut<ServerTime>,
) {
    let client_id = client_id.0;
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate { id, translation, entity , server_time } => {
                println!("Player {} connected at translation  {:?}", id, translation);     

                let texture_atlas = TextureAtlas {
                    layout: chaski.layout.clone(),
                    index: 0,
                };
                
                
                let mut client_entity = commands.spawn(
            (
                        PbrBundle {
                            mesh: sprite_params.meshes.add(Mesh::from(Capsule3d::new(0.5, 1.))),
                            material: sprite_params.materials.add(Color::srgba(0.8, 0.7, 0.6, 0.5)),
                            transform: Transform::from_xyz(translation[0], translation[1], translation[2]),
                            ..Default::default()
                        },  
                        /*Sprite3d {
                            image: chaski.sprite.clone(),
                            pixels_per_metre: 48.,
                            //pixels_per_metre: 128.,
                            alpha_mode: AlphaMode::Blend,
                            unlit: true,
                            transform: Transform::from_xyz(translation[0], translation[1]-2.0, translation[2]),
                            // transform: Transform::from_xyz(0., 0., 0.),
                            //pivot: Some(Vec2::new(0.5, 0.5)),
                            pivot: Some(Vec2::new(0.5, 0.)), // para que gire sobre los pies y no del centro.
                            ..default()
                        }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()),*/
                        Name::new("Player"),
                        //Collider::capsule(0.4, 1.0),
                        //RigidBody::Dynamic     
                       
                        Collider::capsule_y(0.5, 0.5),
                        RigidBody::KinematicPositionBased,
                        //Mass(5.0),
                        Health {
                            max: 100,
                            current: 100,
                        },
                        Mana {
                            max: 100,
                            current: 100,
                        },
                        BarSettings::<Health> {
                            offset: -1.05,
                            width: 1.2,
                            height: BarHeight::Static(0.10),
                            ..default()
                        },
                        BarSettings::<Mana> {
                            offset: -1.15,
                            width: 1.2,
                            height: BarHeight::Static(0.10),
                            ..default()
                        },
                        //RigidBody::Kinematic,   
                         //Collider::capsule(0.4, 1.0),
                    ),
                        
                );

                if client_id == id.raw() {
                    client_entity
                        .insert(ControlledPlayer) 
                        //.insert(Billboard)
                        .insert(GameVelocity::default())
                        .insert(Facing(0) )
                        //.insert(NotShadowCaster)
                        .insert(PositionHistory::new(Vec3 {x: translation[0], y: translation[1]+0.0, z: translation[2]}))
                        .insert(AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)));

                    //server_time_res.0 = server_time;
                }

                let player_info = PlayerInfo {
                    server_entity: entity,
                    client_entity: client_entity.id(),
                };
                lobby.players.insert(id, player_info);
                network_mapping.0.insert(entity, client_entity.id());
            }
            ServerMessages::PlayerRemove { id } => {
                println!("Player {} disconnected.", id);
                if let Some(PlayerInfo {
                    server_entity,
                    client_entity,
                }) = lobby.players.remove(&id)
                {

                    // a veces el mensaje de desconexión llega para un cliente que aun no spawneaba a esa entidad
                    // y crasheaba.
                    if let Some(_entity_exists) = commands.get_entity(client_entity) {
                        commands.entity(client_entity).despawn();
                        network_mapping.0.remove(&server_entity);
                    }
                   
                }
            }
            ServerMessages::SpawnProjectile { entity, translation } => {

                //let mut meshes = sprite_params.meshes.clone();

                let projectile_entity = commands.spawn(PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                });
                network_mapping.0.insert(entity, projectile_entity.id());
            }
            ServerMessages::DespawnProjectile { entity } => {
                if let Some(entity) = network_mapping.0.remove(&entity) {
                    commands.entity(entity).despawn();
                }
            }
            ServerMessages::SpawnMonster { entity, kind, translation , server_time} => {
    
                let texture_atlas: TextureAtlas = match kind {
                    MonsterKind::Pig  => {
                        TextureAtlas {
                            layout: pig_assets.layout.clone(),
                            index: 0,
                        }
                    },
                    MonsterKind::Orc  => {
                        TextureAtlas {
                            layout: pig_assets.layout.clone(),
                            index: 0,
                        }
                    }
               
                };
                
             

                let mut monster_entity = commands.spawn((
                    Sprite3d {
                        image: pig_assets.sprite.clone(),
                        pixels_per_metre: 25.,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        transform: Transform::from_translation(translation.into()),
                        // pivot: Some(Vec2::new(0.5, 0.5)),
        
                        ..default()
                    }.bundle_with_atlas(&mut sprite_params, texture_atlas.clone()),    
                    kind,
                    Name::new("Pig"),
                   
                    )
                );       

                monster_entity                
                    //.insert(Billboard)
                    .insert(GameVelocity::default())
                    .insert(PositionHistory::new(Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}))
                    .insert(Facing(0))
                    ;

                /*let monster_entity = commands.spawn(PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                });*/
                network_mapping.0.insert(entity, monster_entity.id());
            },
            ServerMessages::SpawnEntity { entity, sprite_id, translation , facing} => {    
           
                let texture_atlas: TextureAtlas =  TextureAtlas {
                    layout: seal_assets.layout.clone(),
                    index: 58,
                };         
              
     

                let mut client_entity = commands.spawn((
                    Sprite3d {
                        image: seal_assets.sprite.clone(),
                        pixels_per_metre: 25.,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        transform: Transform::from_translation(translation.into()),
                        // pivot: Some(Vec2::new(0.5, 0.5)),
        
                        ..default()
                    }.bundle_with_atlas(&mut sprite_params, texture_atlas.clone()),    
                    MonsterKind::Pig,
                    Collider::capsule_y(0.5, 0.5),
                    RigidBody::KinematicPositionBased,
                    /*Collider::capsule(0.4, 1.0),
                    RigidBody::Kinematic,   */
                    //Mass(5.0),
                    Monster {
                        hp: 100,
                        kind: MonsterKind::Pig 
                    },
                    NotShadowCaster,
                
                    Name::new("Pig")
                    )
                );       

                println!("PIG SPAWNED AT  {:?} ", translation);

                println!("Client entity  {:?} ", client_entity.id());

                client_entity
                    .insert(  Health {
                        max: 100,
                        current: 100,
                    })
                    .insert(
                        BarSettings::<Health> {
                        offset: 0.,
                        width: 0.,
                        height: BarHeight::Static(0.0),
                        ..default()
                    })
                    //.insert(Billboard)
                    .insert(GameVelocity::default())
                    .insert(PositionHistory::new(Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}))
                    .insert(Facing(0));

                /*let client_entity = commands.spawn(PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                });*/
                network_mapping.0.insert(entity, client_entity.id());
            },
            ServerMessages::DespawnEntity { entity } => {
                println!("Entity despawned {:?} ", entity);
                if let Some(entity) = network_mapping.0.remove(&entity) {
                    commands.entity(entity).despawn_recursive();
                }
            }
            ServerMessages::MoveDelta { entity, x, y,z, server_time} => {
                //println!("Message received  {} ", server_time);
                //println!("server_entity {} ", entity);
                //  println!("network_mapping {:?} ", network_mapping.0);
                if let Some(client_entity) = network_mapping.0.get(&entity) {

                    //println!("client_entity {} ", client_entity);
                  
                    if let Ok( (final_entity, transform, mut position_history)) = entities.get_mut(*client_entity) {                    

                        let quantized_delta = IVec3 { 
                            x: x,
                            y: y,
                            z: z
                        };       
                        position_history.add_delta(quantized_delta,server_time);       

                    }
                }               
            },
            ServerMessages::HealthChange { entity, max, current} => {
                 println!("Cambio el HP {}, {} ", max, current);
                if let Some(client_entity) = network_mapping.0.get(&entity) {
                    commands.entity(*client_entity).insert(Health { max, current });
                }
            }
        }
    }

}


fn billboard(
    mut camera_query: Query< (&Transform, &PanOrbitCamera),  (With<Camera>, Without<Billboard>, Changed<Transform>) >,
    //mut player_query: Query<&mut Transform, (With<ControlledPlayer>, Without<Monster>)>,
    mut entities_query: Query<(&mut Transform), (With<Billboard>)>
) {

 
    if let Ok((mut camera_transform, pan_cam)) = camera_query.get_single_mut() {
 
         for (mut entity_transform) in entities_query.iter_mut() {     
          
            if let Some(yaw) = pan_cam.yaw {
                entity_transform.rotation =  Quat::from_rotation_y(yaw);    
            }
            
            //println!("Entity rotation {} camera rotation at translation  {:?}",  entity_transform.rotation, camera_transform.rotation);   
            //println!("Pitch {:?}", pan_cam.pitch);  
            if let Some(pitch) = pan_cam.pitch {

                let pitch_cosine = pitch.clamp(-1.0, 1.0); 
                let stretch_y = 1.0 / pitch_cosine;

                //entity_transform.scale.y = 1. + pitch ;   

             
                let camera_forward = camera_transform.forward();

                // Calculate the horizontal forward direction (flattened to ignore Y component)
                let horizontal_forward = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize();
    
                // Compute the cosine of the pitch angle between camera_forward and horizontal_forward
                let pitch_cosine = camera_forward.dot(horizontal_forward) 
                                   / (camera_forward.length() * horizontal_forward.length());
    
                // Calculate the pitch angle (theta) in radians
                let pitch_angle = pitch_cosine.acos();
    
                // Normalize pitch angle to a range of 0 to 1 (0 when horizontal, 1 when vertical)
                let pitch_ratio = pitch_angle / std::f32::consts::FRAC_PI_2;
    
                // Smooth stretch: lerp between 1.0 (no stretch) and MAX_STRETCH based on pitch_ratio
                //let stretch_y = 1.0 + pitch_ratio * (1.5 - 1.0);
                let max_stretch = 3.5; // Define your max stretch factor
                let stretch_y = (1.0 / pitch.cos()).clamp(1.0, max_stretch);
                //let stretch_y = 1. + pitch;
                // Apply the stretch to the billboard’s Y scale
                let k = 1.0; // Adjust this value to control the intensity of the stretch
                let stretch_y =  1.0 / pitch.cos();
                entity_transform.scale = Vec3::new(1.0, stretch_y, 1.0);

            }
          
            //entity_transform.rotation = camera_transform.rotation; 
            /*let quat = Quat::from_rotation_y(camera_transform.rotation.y);
            println!("quat {:?}",  quat);     
            println!("Entity rotation {} camera rotation at translation  {:?}",  entity_transform.rotation, camera_transform.rotation);     
            //entity_transform.rotation = quat;
            //entity_transform.rotation.y = camera_transform.rotation.y; 
            //entity_transform.look_at(camera_transform.translation, Vec3::Y);
            let quat =camera_transform.rotation.inverse();
            entity_transform.rotation.y = quat.y;*/
        }
    }

   
}


fn setup_camera(
    mut commands: Commands,
) {

    
    /*commands
        .spawn(LookTransformBundle {
            transform: LookTransform {
                eye: Vec3::new(0.0, 20., 2.5),
                target: Vec3::new(0.0, 2.5, 0.0),
                up: Vec3::Y,
            },
            smoother: Smoother::new(0.0),
        })
        .insert(Camera3dBundle {
            transform: Transform::from_xyz(0., 20.0, 2.5).looking_at(Vec3::new(10.0, 0.5, 0.0), Vec3::Y),
            ..default()
        })
        .insert(PanOrbitCamera {
            // Set focal point (what the camera should look at)
            focus: Vec3::new(0.0, 1.0, 0.0),
            // Set the starting position, relative to focus (overrides camera's transform).
            yaw: Some(TAU / 8.0),
            pitch: Some(TAU / 8.0),
            radius: Some(5.0),
            // Set limits on rotation and zoom
            yaw_upper_limit: Some(TAU / 4.0),
            yaw_lower_limit: Some(-TAU / 4.0),
            pitch_upper_limit: Some(TAU / 3.0),
            pitch_lower_limit: Some(-TAU / 3.0),
            zoom_upper_limit: Some(5.0),
            zoom_lower_limit: Some(1.0),
            // Adjust sensitivity of controls
            orbit_sensitivity: 1.5,
            pan_sensitivity: 0.5,
            zoom_sensitivity: 0.5,
            // Allow the camera to go upside down
            allow_upside_down: true,
            // Change the controls (these match Blender)
            button_orbit: MouseButton::Middle,
            button_pan: MouseButton::Middle,
            modifier_pan: Some(KeyCode::ShiftLeft),
            // Reverse the zoom direction
            reversed_zoom: true,
            // Use alternate touch controls
            touch_controls: TouchControls::TwoFingerOrbit,
            ..default()
        });*/
            //Ryfjallet_cubemap

            
        //let skybox_handle = assets.load("skyboxes/skybox.png");
      
        

        commands.spawn((
            // Note we're setting the initial position below with yaw, pitch, and radius, hence
            // we don't set transform on the camera.
            Camera3dBundle {
                transform: Transform::from_translation(Vec3::new(0.0, 25.5, 5.0)),
                ..default()
            },
            PanOrbitCamera {
                 // Panning the camera changes the focus, and so you most likely want to disable
                // panning when setting the focus manually
                pan_sensitivity: 0.0,
                zoom_upper_limit: Some(35.0),
                button_orbit: MouseButton::Right,
                pitch: Some(TAU / 8.0),
                // If you want to fully control the camera's focus, set smoothness to 0 so it
                // immediately snaps to that location. If you want the 'follow' to be smoothed,
                // leave this at default or set it to something between 0 and 1.
                pan_smoothness: 0.0,
                pitch_upper_limit: Some(TAU / 6.0),
                pitch_lower_limit: Some(-0.0),
                ..default()
            },
            AtmosphereCamera::default()
        ));
}


fn camera_follow(
    mut camera_query: Query<(
       // &mut LookTransform, 
  
        &mut PanOrbitCamera),  (With<Camera>, Without<ControlledPlayer>)>,
    player_query: Query<&Transform, (With<ControlledPlayer>, Changed<Transform>)>
) {
    let (mut pan_cam) = camera_query.single_mut();
    if let Ok(player_transform) = player_query.get_single() {
     
        //cam.look = Transform::from_xyz(0., 8.0, 2.5).looking_at(player_transform.translation.into(), Vec3::Y);
         pan_cam.target_focus  = player_transform.translation.into();
         pan_cam.force_update = true;
        /*cam_transform.eye.x = player_transform.translation.x;
        cam_transform.eye.z = player_transform.translation.z + 15.5; // Con esto se mueve el angulo de la camara
        cam_transform.target = player_transform.translation;*/
    }
}


fn camera_with_parent(
    q_child: Query<(&Parent, &Transform), With<Camera>>,
    q_parent: Query<&GlobalTransform>,
) {
    for (parent, child_transform) in q_child.iter() {
        // `parent` contains the Entity ID we can use
        // to query components from the parent:
        let parent_global_transform = q_parent.get(parent.get());

        // do something with the components
    }
}

fn sprite_movement(
    time: Res<Time>,
    mut q_parent: Query<(&mut AnimationTimer, &mut GameVelocity)>,
    mut q_child: Query<(&Parent, &mut TextureAtlas)>,
) {    
    for (parent, mut atlas) in q_child.iter_mut() {

        if let Ok ((mut timer, velocity)) = q_parent.get_mut(parent.get()) {
           
            if velocity.0 == Vec3::ZERO {
                continue;
            }       
          

      
            let x = (velocity.0.x * 1000.0).round() / 1000.0;
            let z = (velocity.0.z * 1000.0).round() / 1000.0;
            println!("Z {:?}, X {:?}", z, x);    
            
            // Mirando hacia arriba
            if(z > 0. && x == 0.0) {
                println!("Mirando hacia arriba {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 0..7;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 7 {
                        0
                    }
                    else {
                        atlas.index + 1
                    };
                }
            }
             // Mirando hacia la arriba a la derecha
             else if(z > 0. && x < 0.0) {
                println!("Mirando hacia la arriba a la derecha {:?}", velocity);   
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 8..15;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 15 {
                        8
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
            // Mirando hacia la derecha
            else if(z == 0. && x < 0.0) {
                println!("Mirando hacia la derecha {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 16..23;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 23  {                   
                        16
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
            // Mirando hacia la abajo a la derecha
            else if(z < 0. && x < 0.0) {
                println!("Mirando hacia la abajo a la derecha {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 24..31;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 31  {                   
                        24
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
            // Mirando hacia abajo
            else if(z < 0. && x == 0.0) {
                println!("Mirando hacia abajo {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 32..39;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 39  {                   
                        32
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
            // Mirando hacia la abajo a la izquierda
            else if(z < 0. && x > 0.0) {
                println!("Mirando hacia la abajo a la izquierda {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 40..47;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 47  {                   
                        40
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
            // Mirando hacia la izquierda
            else if(z == 0. && x > 0.0) {
                println!("Mirando hacia la izquierda {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 48..55;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 55  {                   
                        48
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
            
            // Mirando hacia la arriba a la izquierda
            else if(z > 0. && x > 0.0) {
                println!("Mirando hacia la arriba a la izquierda {:?}", velocity);     
                timer.tick(time.delta());
                if timer.just_finished() {
                    let a = 56..63;
                    atlas.index = if !a.contains(&atlas.index) || atlas.index == 63  {                   
                        56
                    }
                    else {
                        atlas.index + 1
                    };
                }  
            }
           
           
          
    
    
        }
    }

    
}

