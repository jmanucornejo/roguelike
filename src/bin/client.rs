
use avian3d::math::Scalar;
use avian3d::prelude::Collider;
use avian3d::prelude::RigidBody;
use avian3d::prelude::*;
use bevy_sprite3d::*;
use bevy_obj::ObjPlugin;
use local_ip_address::local_ip;
use pathfinding::prelude::astar;
use pathing::*;
use client_plugins::interpolation::*;
use client_plugins::client_clock_sync::*;
use client_plugins::shared_resources::*;

use roguelike::*;

use bevy::{asset::LoadState, input::mouse::MouseWheel, log::LogPlugin, pbr::NotShadowCaster, prelude::*, render::render_resource::Texture, window::{PrimaryWindow, Window, WindowResolution}};
pub use bevy_renet::renet::transport::ClientAuthentication;
use bevy_renet::{renet::*, transport::NetcodeClientPlugin};
use bevy_renet::*;
use bevy_renet::renet::transport::NetcodeClientTransport;
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
use std::f32::consts::TAU;
use std::ops::Div;
use std::ops::Mul;


#[derive(Component, Debug)]
struct OldMovementState {
    position: Vec3,
    server_time: u128
}

#[derive(Component)]
struct MyMovementState {
    position: Vec3,
    server_time: u128
}


#[derive(Component)]
struct ControlledPlayer;

#[derive(Component)]
struct Billboard;

#[derive(Default, Resource, )]
struct NetworkMapping(HashMap<Entity, Entity>);

#[derive(Default, Resource)]
struct Latency(u16);

#[derive(Default, Resource)]
struct SyncData {
    timer: Timer,
    samples: Vec<u16>,
    total_rtt: u128,
    total_offset: u128,
    sync_attempts: usize,
    max_attempts: usize,
}

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


#[derive(Component)]
struct Target;


#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);


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
struct ChaskiAssets {
    #[asset(texture_atlas_layout(tile_size_x = 54, tile_size_y = 129, columns = 8, rows = 1, padding_x = 0, padding_y = 0, offset_x = 0, offset_y = 40))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "chasqui-spritesheet.png")]
    sprite: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
struct GridTarget {
    #[asset(path = "grid-transparent.png")]
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
                title: "Renet Demo Client".to_string(),
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
                .load_collection::<GridTarget>()
                .load_collection::<ChaskiAssets>()
        )
        .add_plugins(  
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
        )
        .add_plugins(ObjPlugin) 
        .add_plugins((PanOrbitCameraPlugin, MaterialPlugin::<WaterMaterial>::default()))
        // .add_plugins(LookTransformPlugin)
        
        //.add_plugins(DefaultPlugins)
        .add_plugins(RenetClientPlugin)        
        .insert_resource(PlayerInput::default())
        .insert_resource(ClientLobby::default())
        //.insert_resource(avian3d::prelude::SpatialQueryPipeline::default())
        .add_plugins((
            PhysicsPlugins::default(),
        ))
        .insert_resource(Map::default())
        .insert_resource(NetworkMapping::default())
        .insert_resource(ServerTime::default())
        .insert_resource(ClockOffset::default())
        .insert_resource(Latency::default())
        .insert_resource(SyncData  {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            samples: Vec::new(),
            total_rtt: 0,
            total_offset: 0,
            sync_attempts: 0,
            max_attempts: 10, // Number of sync requests to send
        })
        .add_event::<PlayerCommand>()
        .add_plugins(NetcodeClientPlugin)   
      
        .add_systems(Startup, (setup_level,setup_camera,move_water))
        .add_plugins(Sprite3dPlugin)
        .add_plugins((InterpolationPlugin, ClientClockSyncPlugin))
        // .add_plugins(PathingPlugin)
        .add_systems(OnEnter(AppState::InGame), ((setup_player, setup_target)))
        .add_systems(Update, 
            (
               
                update_cursor_system.run_if(in_state(AppState::InGame)),
                // print_hits.run_if(in_state(AppState::InGame)).after(update_cursor_system),
                player_input.run_if(in_state(AppState::InGame)),             
                //camera_zoom.run_if(in_state(AppState::InGame)),
                client_send_input.run_if(in_state(AppState::InGame)),              
                client_send_player_commands.run_if(in_state(AppState::InGame)),
                //transform_movement_interpolate.run_if(in_state(AppState::InGame))
                //interpolate_system.run_if(in_state(AppState::InGame))
                billboard.run_if(in_state(AppState::InGame)),
                // client_lerp.run_if(in_state(AppState::InGame)),
               
            )
        )  
        .add_systems(
            FixedUpdate, (        
                
                // client_move_entities.run_if(in_state(AppState::InGame)),
                client_sync_players.run_if(in_state(AppState::InGame)),
                // client_sync_players.run_if(in_state(AppState::InGame)).after(client_sync_time_system),
                // client_sync_entities.run_if(in_state(AppState::InGame)),
                //click_move_players_system.run_if(in_state(AppState::InGame)),
                camera_follow.run_if(in_state(AppState::InGame)),

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

//fn update_projectiles_system(mut commands: Commands, mut projectiles: Query<(Entity, &mut Projectile)>, time: Res<Time>) {
fn player_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    target_query: Query<&Transform, With<Target>>,
    mut player_commands: EventWriter<PlayerCommand>,
    mut commands: Commands,
    player_entities: Query<Entity, With<ControlledPlayer>>,
) {
    player_input.left = keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
    player_input.right = keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
    player_input.up = keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
    player_input.down = keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);

    if mouse_button_input.just_pressed(MouseButton::Left) {
        let target_transform = target_query.single();

        let mut move_translation = target_transform.translation;
        move_translation.x = move_translation.x.round();
        move_translation.z = move_translation.z.round();

        player_input.destination_at = Some(Pos(move_translation.x as i32, move_translation.z as i32));

        if let Ok(player_entity) = &player_entities.get_single() {
            info!("Hay un player entity: {:?}!", player_entity );
            commands.entity(*player_entity).insert(PlayerCommand::Move {
                destination_at: move_translation,
            });
        }      

        player_commands.send(PlayerCommand::Move {
            destination_at: move_translation,
        });
        player_commands.send(PlayerCommand::BasicAttack {
            cast_at: target_transform.translation,
        });
    }
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


fn client_sync_players(
    mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
   // mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    client_id: Res<CurrentClientId>,
    mut lobby: ResMut<ClientLobby>,
    mut network_mapping: ResMut<NetworkMapping>,
    assets            : Res<MyAssets>,
    chaski            : Res<ChaskiAssets>,
    pig_assets            : Res<PigAssets>,
    mut sprite_params : Sprite3dParams,       
    mut entities: Query<(Entity, &Transform, &mut MyMovementState, &mut OldMovementState, &mut TargetPos, &mut PositionHistory)>, 
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
                        Sprite3d {
                            image: chaski.sprite.clone(),
                            pixels_per_metre: 37.5,
                            alpha_mode: AlphaMode::Blend,
                            unlit: true,
                            transform: Transform::from_xyz(translation[0], translation[1]+1.0, translation[2]),
                            // transform: Transform::from_xyz(0., 0., 0.),
                            // pivot: Some(Vec2::new(0.5, 0.5)),

                            ..default()
                        }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()), Name::new("Player"),
                        //Collider::capsule(0.4, 1.0),
                        //RigidBody::Dynamic     
                    ),
                        
                );

                if client_id == id.raw() {
                    client_entity
                        .insert(ControlledPlayer) 
                        .insert(Billboard)
                        .insert(Velocity::default())
                        .insert(Facing(0) )
                        .insert(NotShadowCaster)
                        .insert(PositionHistory::new(Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}))
                        .insert(PrevState { translation: Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}, rotation: Facing(0) })  
                        .insert(TargetState { translation: Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}, rotation: Facing(0) })
                        .insert(TargetPos { position: Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}})
                        .insert(OldMovementState { position:Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}, server_time })
                        .insert(MyMovementState { position:Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}, server_time})
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
                    commands.entity(client_entity).despawn();
                    network_mapping.0.remove(&server_entity);
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
                    Name::new("Pig")
                    )
                );       

                monster_entity
                    .insert(Billboard)
                    .insert(Velocity::default())
                    .insert(Facing(0))
                    .insert(OldMovementState { position: translation.into(), server_time })
                    .insert(MyMovementState { position: translation.into(), server_time });
                /*let monster_entity = commands.spawn(PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                });*/
                network_mapping.0.insert(entity, monster_entity.id());
            },
            ServerMessages::MoveDelta { entity, x, y,z, rotation, server_time} => {
                //println!("Message received  {} ", server_time);
                //println!("server_entity {} ", entity);
                //println!("network_mapping {:?} ", network_mapping.0);
                if let Some(client_entity) = network_mapping.0.get(&entity) {

                    //println!("client_entity {} ", client_entity);
                  
                    if let Ok( (final_entity, transform,  mut state,  old_state, mut target_pos, mut position_history)) = entities.get_mut(*client_entity) {                    

                        let quantized_delta = IVec3 { 
                            x: x,
                            y: y,
                            z: z
                        };       
                        position_history.add_delta(quantized_delta,server_time);       

                    }
                }               
            }
        }
    }
    /*
    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let networked_entities: NetworkedEntities = bincode::deserialize(&message).unwrap();

        for i in 0..networked_entities.entities.len() {
            if let Some(entity) = network_mapping.0.get(&networked_entities.entities[i]) {
                let mut translation: Vec3 = networked_entities.translations[i].into();
                translation.y = 2.0;


                //println!("Entity translation {:?}.", translation);
                /*let transform = Transform {
                    translation,
                    ..Default::default()
                };*/
                // println!("Netwrok translation {:?}.", transform.translation);
                //commands.entity(*entity).insert(transform);

                let movement_state = TargetPos {
                    position: translation
                };

                commands.entity(*entity).insert(movement_state);
            }
        }
    }*/
}


/* 
fn setup_server_time_and_latency(
    time: Res<Time>,
    mut client: ResMut<RenetClient>,
) {

    let sync_request_message = bincode::serialize(&ClientMessage::SyncTimeRequest { client_time: time.elapsed().as_millis() }).unwrap();

    client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
    //client.send_message(reliable_channel_id, ping_message);
    info!("Sent sync time request!");
}*/

/*
fn client_sync_time_system(
    time: Res<Time>,
    mut sync_data: ResMut<SyncData>,
    mut client: ResMut<RenetClient>,
    mut server_time_res: ResMut<ServerTime>,
    mut latency: ResMut<Latency>,
    mut clock_offset: ResMut<ClockOffset>,
) {

    sync_data.timer.tick(time.delta());

    if sync_data.timer.finished() {
       // let sync_request_message = bincode::serialize(&ClientMessage::LatencyRequest { client_time: time.elapsed().as_millis() }).unwrap();
        //client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
    }

    while let Some(message) = client.receive_message(ClientChannel::SyncTimeRequest) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessage::SyncTimeResponse { client_time, server_time } => {                      

                let rtt = time.elapsed().as_millis() - client_time;
                let one_way_latency = rtt / 2;
                server_time_res.0 = server_time + one_way_latency;
                clock_offset.0 = server_time_res.0 - time.elapsed().as_millis();
                latency.0 = one_way_latency as u16;
                info!("server_time_res {:?}, latency  {:?}", server_time_res.0, one_way_latency);

                sync_data.total_rtt += rtt;
                sync_data.total_offset +=  clock_offset.0;
                sync_data.sync_attempts += 1;

                if sync_data.sync_attempts >= sync_data.max_attempts {
                    let avg_rtt = sync_data.total_rtt / sync_data.sync_attempts as u128;
                    let one_way_latency = avg_rtt / 2;
                    //latency.0 = one_way_latency;

                    server_time_res.0 = server_time + one_way_latency;
                    latency.0 = one_way_latency as u16;
                    clock_offset.0 = sync_data.total_offset / sync_data.sync_attempts as u128;
                    info!("one_way_latency {:?}",one_way_latency);
                    info!("offset {:?}",clock_offset.0);
                   
                   
                    // Adjust client clock
                    //client_time.0 = estimated_server_time;

                    // Reset sync data for next sync cycle
                    // sync_data.pending_requests = 0;
                    // sync_data.total_rtt = 0;
                    // sync_data.sync_attempts = 0;
                } 
                else {
                    let sync_request_message = bincode::serialize(&ClientMessage::SyncTimeRequest { client_time: time.elapsed().as_millis() }).unwrap();
                    client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);

                }
              
            },
            ServerMessage::LatencyResponse { client_time } => {           

                //info!("client_time{:?}",client_time);
                let rtt = (time.elapsed().as_millis() - client_time) as u16;

                sync_data.samples.push(rtt);

                if(sync_data.samples.len() == 9) {

                    sync_data.samples.sort();

                    //let mid_point = sync_data.samples.get(4);

                    let median = sync_data.samples[4];
                    info!("median {:?}",median);
       
                    sync_data.samples.retain(|sample|  if *sample > median.mul(2) && *sample > 20 {  
                        false
                    }
                    else {
                        true
                    });
                    info!("median {:?}",sync_data.samples);
       
                    latency.0 = sync_data.samples.iter().sum::<u16 >() / sync_data.samples.len() as u16 ;
                    info!("average_latency {:?}",latency.0);

                    sync_data.samples.clear();

                }
               
                /* 
                sync_data.total_rtt += rtt;
                sync_data.sync_attempts += 1;

                if sync_data.sync_attempts >= sync_data.max_attempts {
                    let avg_rtt = sync_data.total_rtt / sync_data.sync_attempts as u128;
                    let one_way_latency = avg_rtt / 2;
                    //latency.0 = one_way_latency;

                 
                    info!("one_way_latency {:?}",one_way_latency);
                   
                    // Adjust client clock
                    //client_time.0 = estimated_server_time;

                    // Reset sync data for next sync cycle
                    // sync_data.pending_requests = 0;
                    // sync_data.total_rtt = 0;
                    // sync_data.sync_attempts = 0;
                } */
            }
        }
    }
} */




fn client_move_entities(
    mut query: Query<(&mut MyMovementState, &mut OldMovementState, &mut MovementDelta, &mut TargetPos)>, 
   
) {

    for(mut state, mut old_state, mut delta, mut target_pos) in query.iter_mut() {
        
        if(delta.translation == IVec3 { x: 0, y: 0, z: 0} ) {           
            continue;
        }

        /*if(delta.server_time <= state.server_time) {    
            continue;
        }*/

        
        
        let unquantized_delta = delta.translation.as_vec3().mul(TRANSLATION_PRECISION);
        let new_position =   state.position +  unquantized_delta;
        old_state.position = state.position;
        old_state.server_time = state.server_time;


        if(new_position != Vec3::from(delta.real_translation)) {
            
            println!("starting_position  {} ", old_state.position);
            println!("unquantized_delta.  {} ", unquantized_delta);
            println!("new_position.  {} ", new_position);     
            println!("real_translation.  {:?} ", delta.real_translation);
        }
        old_state.position = state.position;
        old_state.server_time = state.server_time.clone();

        target_pos.position = new_position;

        // let quantized_position = transform.translation.div(TRANSLATION_PRECISION).as_ivec3(); // TRANSLATION_PRECISION == 0.01

        //let new_translation = quantized_position + delta.translation;
        state.position  =  new_position;
        state.server_time = delta.server_time;
        /*transform.translation +=  unquantized_delta;

        transform.translation = transform.translation.lerp(new_position, delta.server_time;

        */
        delta.translation = IVec3 { x: 0, y: 0, z: 0};
          
        
    }
    
    /*let target_translation = next_snapshot.translation;
    let target_rot = next_snapshot.rotation;
    let target_server_time = next_snapshot.server_time_millis;

    if current_time > target_server_time {
        current_snapshot = next_snapshot;

        transform.translation = target_translation;
        transform.rotation = target_rotation;

        remove_next_snapshot_from_queue();
        continue;
    }

    let progress = (current_time - current_snapshot.arrival_time) / (target_server_time - current_snapshot.arrival_time);

    transform.translation = lerp(
        current_snapshot.translation,
        target_translation,
        progress,
    );
    transform.rotation = lerp(
        current_snapshot.rotation,
        target_rot,
        progress,
    );

    break;*/

}


fn client_lerp(
    mut query: Query<(&mut MyMovementState, &mut OldMovementState, &mut Transform)>, 
   
) {

    for(mut state, mut old_state, mut transform) in query.iter_mut() {

        println!("old_state,server_time.  {} ", old_state.server_time);
        println!("state,server_time.  {} ", state.server_time);

        let time_passed =  Duration::from_millis((state.server_time - old_state.server_time) as u64);
        let a = time_passed.as_secs_f32();

        
        // let a = fixed_time.overstep_fraction();
        transform.translation = old_state.position.lerp(state.position, a);
        

    }
    
    /*let target_translation = next_snapshot.translation;
    let target_rot = next_snapshot.rotation;
    let target_server_time = next_snapshot.server_time_millis;

    if current_time > target_server_time {
        current_snapshot = next_snapshot;

        transform.translation = target_translation;
        transform.rotation = target_rotation;

        remove_next_snapshot_from_queue();
        continue;
    }

    let progress = (current_time - current_snapshot.arrival_time) / (target_server_time - current_snapshot.arrival_time);

    transform.translation = lerp(
        current_snapshot.translation,
        target_translation,
        progress,
    );
    transform.rotation = lerp(
        current_snapshot.rotation,
        target_rot,
        progress,
    );

    break;*/

}

fn client_sync_entities(
    mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
   // mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    client_id: Res<CurrentClientId>,
    player_entities: Query<Entity, With<ControlledPlayer>>,
    mut network_mapping: ResMut<NetworkMapping>,
    assets            : Res<MyAssets>,
    chaski            : Res<ChaskiAssets>,
    pig_assets            : Res<PigAssets>,
    mut sprite_params : Sprite3dParams,       
) {
    let client_id = client_id.0;

    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate { id, translation, entity , server_time} => {
                println!("Player {} connected.", id);                     
           
                if client_id == id.raw() {
                    
                    let texture_atlas = TextureAtlas {
                        layout: chaski.layout.clone(),
                        index: 0,
                    };                    

                    let mut client_entity = commands.spawn((Sprite3d {
                        image: chaski.sprite.clone(),
                        pixels_per_metre: 40.,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        transform: Transform::from_xyz(translation[0], translation[1]+1.0, translation[2]),
                        // transform: Transform::from_xyz(0., 0., 0.),
                        // pivot: Some(Vec2::new(0.5, 0.5)),
    
                        ..default()
                    }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()), Name::new("Player")));

                    
                    client_entity
                        .insert(ControlledPlayer) 
                        .insert(Billboard)
                        .insert(Velocity::default())
                        .insert(Facing(0) )
                        .insert(NotShadowCaster)                     
                        .insert(TargetPos { position: Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}})
                        .insert(MyMovementState { position:Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}, server_time})
                        .insert(OldMovementState { position:Vec3 {x: translation[0], y: translation[1]+1.0, z: translation[2]}, server_time})
                        .insert(AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)));

                    network_mapping.0.insert(entity, client_entity.id());
                }

                
            },
            _ => {}
        
           
        }
    }
  
    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let networked_entities: NetworkedEntities = bincode::deserialize(&message).unwrap();

        for i in 0..networked_entities.entities.len() {
            if let Some(entity) = network_mapping.0.get(&networked_entities.entities[i]) {
                // Update entity
                let mut translation: Vec3 = networked_entities.translations[i].into();
                translation.y = 2.0;

                let movement_state = TargetPos {
                    position: translation
                };

                commands.entity(*entity).insert(movement_state);
            }
            else {
                // Spawn entity

                let texture_atlas: TextureAtlas = TextureAtlas {
                    layout: pig_assets.layout.clone(),
                    index: 0,
                };
                
   
                let mut monster_entity = commands.spawn((
                    Sprite3d {
                        image: pig_assets.sprite.clone(),
                        pixels_per_metre: 24.,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        transform: Transform::from_translation(networked_entities.translations[i].into()),
                        // pivot: Some(Vec2::new(0.5, 0.5)),
        
                        ..default()
                    }.bundle_with_atlas(&mut sprite_params, texture_atlas.clone()),    
                    MonsterKind::Pig,
                    Name::new("Pig")
                    )
                );       

                monster_entity
                    .insert(Billboard)
                    .insert(Velocity::default())
                    .insert(Facing(0));
    
                network_mapping.0.insert(networked_entities.entities[i], monster_entity.id());
             
            }
        }
        for (server_entity, client_entity) in network_mapping.0.clone() {

            if let Ok(player_entity) = &player_entities.get_single() {

                if(!networked_entities.entities.contains(&server_entity) && client_entity != *player_entity) {
                    println!("Despawn entity {} .", client_entity);     
                    commands.entity(client_entity).despawn();
                    network_mapping.0.remove(&server_entity);
                   
                }
               
            }    

        } 
    }
}



fn billboard(
    mut camera_query: Query< &Transform,  (With<Camera>, Without<Billboard>)>,
    //mut player_query: Query<&mut Transform, (With<ControlledPlayer>, Without<Monster>)>,
    mut entities_query: Query<&mut Transform, (With<Billboard>)>
) {

 
    let (mut camera_transform) = camera_query.single_mut();
     /*if let Ok(mut player_transform) = player_query.get_single_mut() {
        player_transform.rotation = camera_transform.rotation;           
    }*/
    for mut monster_transform in entities_query.iter_mut() {       
        monster_transform.rotation = camera_transform.rotation;  
    }
}



/*fn setup_camera(mut commands: Commands) {
    commands
        .spawn(Camera3dBundle::default())
        .insert(Camera3dBundle {
            transform: Transform::from_xyz(0., 8.0, 2.5).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
            ..default()
        });
}*/


fn setup_camera(mut commands: Commands) {
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
                button_orbit: MouseButton::Right,
                // If you want to fully control the camera's focus, set smoothness to 0 so it
                // immediately snaps to that location. If you want the 'follow' to be smoothed,
                // leave this at default or set it to something between 0 and 1.
                pan_smoothness: 0.0,
                pitch_lower_limit: Some(-0.0),
                ..default()
            },
            RayCaster::default()
        ));
}


fn camera_follow(
    mut camera_query: Query<(
       // &mut LookTransform, 
  
        &mut PanOrbitCamera),  (With<Camera>, Without<ControlledPlayer>)>,
    player_query: Query<&Transform, With<ControlledPlayer>>
) {
    let (
        //mut cam, 
        mut pan_cam) = camera_query.single_mut();
    if let Ok(player_transform) = player_query.get_single() {
     
        //cam.look = Transform::from_xyz(0., 8.0, 2.5).looking_at(player_transform.translation.into(), Vec3::Y);
         pan_cam.target_focus  = player_transform.translation.into();
         pan_cam.force_update = true;
        /*cam_transform.eye.x = player_transform.translation.x;
        cam_transform.eye.z = player_transform.translation.z + 15.5; // Con esto se mueve el angulo de la camara
        cam_transform.target = player_transform.translation;*/
    }
}


/*
fn camera_zoom(
    mut evr_scroll: EventReader<MouseWheel>,
    mut camera_query: Query<&mut LookTransform, (With<Camera>, Without<ControlledPlayer>)>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    for ev in evr_scroll.read() {
        match ev.unit {
            MouseScrollUnit::Line => {
              
                let mut cam_transform = camera_query.single_mut();
                if cam_transform.eye.y + ev.y > 2. && cam_transform.eye.y + ev.y < 25. {
                    println!("Current Scroll level: {}", cam_transform.eye.y);
                    cam_transform.eye.y += ev.y;
                }

                 
             
            }
            MouseScrollUnit::Pixel => {
                println!("Scroll (pixel units): vertical: {}, horizontal: {}", ev.y, ev.x);
            }
        }
    }
}*/

fn setup_player(
    mut commands: Commands, 
    assets            : Res<MyAssets>,
    mut sprite_params : Sprite3dParams,
    
) {  

    let texture_atlas = TextureAtlas {
        layout: assets.layout.clone(),
        index: 3,
    };
    
    commands.spawn(Sprite3d {
        image: assets.sprite.clone(),
        pixels_per_metre: 32.,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        // transform: Transform::from_xyz(0., 0., 0.),
        // pivot: Some(Vec2::new(0.5, 0.5)),

        ..default()
    }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()))
    .insert(AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)));


}


fn setup_target(mut commands: Commands,
    assets            : Res<GridTarget>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>) {

    let texture = assets.sprite.clone();
        /*
    commands.spawn(SpriteBundle {
        material: materials.add(texture.into()),
        ..Default::default()
    }))*/

            
        /*image: assets.sprite.clone(),
        pixels_per_metre: 32.,
        alpha_mode: AlphaMode::Blend,
        unlit: true,*/

    commands
        .spawn((PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(1., 0., 1.))),
            //material: materials.add(Color::srgb(1.0, 0.0, 0.0)),
            //material: materials.add((texture, alpha_mode: )),
            material:  materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                //unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            }),
            transform: Transform::from_xyz(0.0, 1., 0.0),
            ..Default::default()
        },
        NotShadowCaster, 
        Name::new("Target")))
        .insert(Target);

   
}


fn update_cursor_system(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut target_query: Query<&mut Transform, With<Target>>,
    camera_query: Query<(&Camera, &mut RayCaster, &GlobalTransform)>,
    spatial_query: SpatialQuery
) {
    let (camera, mut ray_caster,camera_transform) = camera_query.single();
    
    let mut target_transform = target_query.single_mut();
    if let Some(cursor_pos) = primary_window.single().cursor_position() {

        if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {

            let cam_transform = camera_transform.compute_transform();
            let direction = ray.direction;
            
          
            if let Some(first_hit) = spatial_query.cast_ray(
                cam_transform.translation,                    // Origin
                direction,                       // Direction
                Scalar::MAX,                         // Maximum time of impact (travel distance)
                true,                          // Does the ray treat colliders as "solid"
                SpatialQueryFilter::default(), // Query filter
            ) {
                //println!("First hit: {:?}", first_hit);
                println!(
                    "Hit entity {:?} at {} with normal {}",
                    first_hit.entity,
                    ray.origin + *ray.direction * first_hit.time_of_impact,
                    first_hit.normal,
                );

                let mut translation = ray.origin + *ray.direction * first_hit.time_of_impact;
                translation.x = translation.x.round();
                translation.z = translation.z.round();
                translation.y =  translation.y + 1.; 
                target_transform.translation = translation;

                
            }

            let mut hits = vec![];

            // Cast ray and get all hits
            spatial_query.ray_hits_callback(
                cam_transform.translation,                    // Origin
                direction,                       // Direction
                1000.0,                         // Maximum time of impact (travel distance)
                true,                          // Does the ray treat colliders as "solid"
                SpatialQueryFilter::default(), // Query filter
                |hit| {                        // Callback function
                    hits.push(hit);
                    true
                },
            );

            // Print hits
            for hit in hits.iter() {
                println!("Hit: {:?}", hit);
            }

        
          
            /*if let Some(distance) = ray.intersect_plane(Vec3::Y, InfinitePlane3d::new(Vec3::Y)) {
                //info!("Ray {:?}!", ray.direction * distance + ray.origin );
                let mut translation = ray.direction * distance + ray.origin;
                translation.x = translation.x.round();
                translation.z = translation.z.round();
                target_transform.translation = translation;
            }*/
        }
    }
}

fn print_hits(query: Query<(&RayCaster, &RayHits)>) {
    for (ray, hits) in &query {
        // For the faster iterator that isn't sorted, use `.iter()`
            println!("ray  {:?} ",ray);
        for hit in hits.iter_sorted() {
            println!(
                "Hit entity {:?} at {} with normal {}",
                hit.entity,
                ray.origin + *ray.direction * hit.time_of_impact,
                hit.normal,
            );
        }
    }
}

fn sprite_movement(
    mut query: Query<(&mut Velocity, &mut TextureAtlas)>,

) {    
    for (mut velocity, mut atlas) in &mut query {
        info!("atlas {:?}!", atlas );
        atlas.index = if atlas.index == 4 {
            1
        } else {
            atlas.index + 1
        };
    }
}
/// Perform linear interpolation from old position to new position (runs in Update)
fn interpolate_system(
    mut query: Query<(&OldMovementState, &TargetPos, &mut Transform)>,
    time: Res<Time<Fixed>>,
) {

    for (mut state_old, state, mut transform) in &mut query {
        //let (position_old, position, mut transform) = query.single_mut();

        let delta = state.position - state_old.position;
        let lerped: Vec3 = state_old.position + delta * time.overstep_fraction();

        transform.translation = lerped;

        info!("TRanslation  {:?}!", transform.translation);
        info!("Lerp =  {:?}!", lerped);
        info!("Current velocity =  {:?}!", delta / time.overstep_fraction());
    }
}


