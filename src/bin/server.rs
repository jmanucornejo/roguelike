


use bevy::ecs::schedule::ScheduleLabel;
// use avian3d::prelude::{Collider, GravityScale, LockedAxes, RigidBody};
use bevy::log::{LogPlugin};
use bevy::time::Stopwatch;
use bevy_egui::egui::debug_text::print;
use bevy_obj::ObjPlugin;

///use avian3d::math::{AdjustPrecision, Quaternion, Scalar, Vector};
////use avian3d::prelude::{CoefficientCombine, Collider, ColliderParent, Collisions, Friction, GravityScale, LinearVelocity, LockedAxes, Mass, Position, PostProcessCollisions, Restitution, RigidBody, Rotation, Sensor};
// use avian3d::{PhysicsPlugins};

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

use bevy_renet::netcode::{
    NetcodeServerPlugin, NetcodeServerTransport, NetcodeTransportError,
    ServerAuthentication, ServerConfig,
};
//use bevy_renet::renet::transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
//use bevy_renet::transport::NetcodeServerPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy::input::common_conditions::input_toggle_active;
use bevy_renet::RenetServerPlugin;
use local_ip_address::local_ip;
use monsters::*;
use pathing::*;

use roguelike::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Duration;
use std::{
    net::{SocketAddr, UdpSocket},
    time::SystemTime,
};
use bevy_flycam::prelude::*;
use std::ops::Div;
use std::ops::Mul;
use bevy_spatial::{kdtree::KDTree3, AutomaticUpdate, SpatialAccess};
use renet_visualizer::{RenetServerVisualizer, RenetVisualizerStyle};
use bevy_egui::{EguiContexts, EguiPlugin};
use bevy_rapier3d::prelude::*;
use shared::components::*;
use shared::channels::*;
use shared::constants::*;
use shared::messages::*;
use shared::states::ServerState;

#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<ClientId, Entity>,
}

fn main() {

    let mut app =  App::new();     
       
    app.add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "info,wgpu_core=warn,wgpu_hal=off,rechannel=warn".into(),
            level: bevy::log::Level::DEBUG,
            ..Default::default()
        }))
        //.add_plugins(EguiPlugin)
        .add_plugins(  
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
        )
        .add_plugins(PathingPlugin)
        .add_plugins(AutomaticUpdate::<NearestNeighbourComponent>::new())
        .add_plugins(ObjPlugin) 
        //.add_plugins(PhysicsPlugins::default())
       
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().with_default_system_setup(false))
        .add_plugins(RapierDebugRenderPlugin {           
            ..default()
        })       
        .add_plugins(NoCameraPlayerPlugin)
       // .add_plugins(MinimalPlugins)
        //.add_plugins(LogPlugin::default())
        .add_systems(
            Startup, (
                setup_level,
                setup_simple_camera,
                // setup_prohibited_areas.after(setup_level),
            )
        )
        .init_state::<ServerState>()
        .add_plugins((
            //server_plugins::physics::ServerPhysicsPlugin, 
            MonstersPlugin, 
            server_plugins::server_clock_sync::ServerClockSyncPlugin,
            // server_plugins::clock_server::ClockServerPlugin, // new
            server_plugins::combat::CombatPlugin
        ))
        .add_plugins(RenetServerPlugin)
        .insert_resource(RenetServerVisualizer::<200>::new(
            RenetVisualizerStyle::default(),
        ))
        .insert_resource(ServerLobby::default())
        .insert_resource(Map::default())
        .insert_resource(create_renet_server())
        .add_plugins(NetcodeServerPlugin)
        .insert_resource(create_renet_transport())
        //.add_systems(FixedUpdate, sync_client_time)
        .add_systems(
            Update, 
            (
                server_events, 
                // update_projectiles_system,          
                // update_visualizer_system      
            )
        )
        .add_systems(
            FixedUpdate, (           
                //network_send_delta_position_system.after(roguelike::pathing::apply_rapier3d_velocity_system),      
                //network_send_delta_position_system.after(TransformSystem::TransformPropagate),     
                //network_send_delta_position_system.after(PhysicsSet::Writeback),    
                // network_send_delta_position_system.after(apply_velocity_system),
                //network_send_delta_position_system,  
                //click_move_players_system,
                line_of_sight
                //monster_test
            )
        )
     
        .insert_resource(TimestepMode::Fixed {
            dt: 1.0 / 60.0,  // 60 FPS physics update
            substeps: 1,
        })
        .add_systems(
            PhysicsSchedule,
            (
                RapierPhysicsPlugin::<NoUserData>::get_systems(PhysicsSet::SyncBackend)
                    .in_set(PhysicsSet::SyncBackend),
                RapierPhysicsPlugin::<NoUserData>::get_systems(PhysicsSet::StepSimulation)
                    .in_set(PhysicsSet::StepSimulation),
                RapierPhysicsPlugin::<NoUserData>::get_systems(PhysicsSet::Writeback)
                    .in_set(PhysicsSet::Writeback),
            ),
        )
        .init_schedule(PhysicsSchedule)
        .edit_schedule(PhysicsSchedule, |schedule| {
            schedule.configure_sets((
                PhysicsSet::SyncBackend,
                PhysicsSet::StepSimulation,
                PhysicsSet::Writeback,
            ).chain());
        })
        .add_systems(FixedUpdate, run_physics_schedule.before(roguelike::pathing::get_velocity));
       
        #[cfg(not(feature = "absolute_interpolation"))]
        app.add_systems(FixedPostUpdate,    (network_send_delta_position_system));
        
        #[cfg(feature = "absolute_interpolation")]
        app.add_systems(FixedPostUpdate, (network_send_absolute_position_system));

        app.run();
    
}


#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PhysicsSchedule;


// -------
pub fn run_physics_schedule(world: &mut World) {

    world.run_schedule(PhysicsSchedule);
    /*fn simulate(world: &mut World, dt: f32) {
        let mut pxtime = world.resource_mut::<Time<Fixed>>();
        pxtime.update(Duration::from_secs_f32(dt));

      
        world.run_schedule(PhysicsSchedule);
    }

    let time_delta_f32 = world.resource::<Time>().delta_seconds();
    simulate(world, time_delta_f32);*/
}


fn create_renet_server() -> RenetServer {
    RenetServer::new(connection_config())
}

fn create_renet_transport() -> NetcodeServerTransport {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    /* Public hosting, requires port forwarding
    let rt = tokio::runtime::Runtime::new().unwrap();
    let public_ip = rt.block_on(public_ip::addr()).unwrap();
    let server_addr = SocketAddr::new(public_ip, 42069);
    */

    let server_addr = SocketAddr::new(local_ip().unwrap(), 42069);
 
    info!("Creating Server! {:?}", server_addr);

    let server_config: ServerConfig = ServerConfig {
        current_time: current_time,
        max_clients: 64,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Unsecure
    };


    let inbound_server_addr = SocketAddr::new(local_ip().unwrap(), 42069);

    let socket = UdpSocket::bind(inbound_server_addr).unwrap();

    NetcodeServerTransport::new(server_config, socket).unwrap()

}

fn update_visualizer_system(mut egui_contexts: EguiContexts, mut visualizer: ResMut<RenetServerVisualizer<200>>, server: Res<RenetServer>) {
    visualizer.update(&server);
    //visualizer.show_window(egui_contexts.ctx_mut());
}


fn server_events(
    mut server_events: EventReader<ServerEvent>, 
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    players: Query<(Entity, &Player, &Transform)>,
    monsters: Query<(Entity, &Monster, &Transform), With<Monster>>,
    treeaccess: Res<NNTree>,
    mut server_visualizer: ResMut<RenetServerVisualizer<200>>,
    time: Res<Time>,
    map: Res<Map>
) {
    
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!("Client {client_id} connected");
                server_visualizer.add_client(*client_id);
                // Get player spawning poing.
                let transform = Transform::from_xyz((fastrand::f32() - 0.5) * 40., 1.0, (fastrand::f32() - 0.5) * 40.);
                info!("entity  transform {:?}", transform);
                
                // Find all entities within 12 cells of translation.
                for (_, entity) in treeaccess.within_distance(transform.translation.into(), LINE_OF_SIGHT) {
                    // info!("entity {:?}", entity);

                    // Initialize monsters for this new client
                    /* 
                    if let Ok( (entity, monster,  monster_transform)) = monsters.get(entity.expect("No entity")) {

                        let message = bincode::serialize(&ServerMessages::SpawnMonster {
                            entity,
                            kind: monster.kind.clone(),
                            translation: monster_transform.translation.into(),
                            server_time: time.elapsed().as_millis()
                        })
                        .unwrap();
                        server.send_message(*client_id, ServerChannel::ServerMessages, message);
                    }
                    */
                    // Initialize Players for this new client
                    if let Ok( (entity, player,  player_transform)) = players.get(entity.expect("No entity")) {

                        let message = bincode::serialize(&ServerMessages::PlayerCreate {
                            id: player.id,
                            entity,
                            translation: player_transform.translation.into(),
                            server_time: time.elapsed().as_millis()
                        })
                        .unwrap();
                        server.send_message(*client_id, ServerChannel::ServerMessages, message);
                    }
                    
                   
                }
               

                // Initialize other players for this new client
                /*for (entity, player, transform) in players.iter() {
                    let translation: [f32; 3] = transform.translation.into();
                    let message = bincode::serialize(&ServerMessages::PlayerCreate {
                        id: player.id,
                        entity,
                        translation,
                    })
                    .unwrap();
                    server.send_message(*client_id, ServerChannel::ServerMessages, message);
                }*/

                // Spawn new player              
                let player_entity = commands
                    .spawn((
                        Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.5, 1.)))),
                        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
                        transform,
                        /*PbrBundle {
                            mesh: meshes.add(Mesh::from(Capsule3d::new(0.5, 1.))),
                            material: materials.add(Color::srgb(0.8, 0.7, 0.6)),
                            transform,
                            ..Default::default()
                        },  */
                        LockedAxes::ROTATION_LOCKED,
                        //Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
                        //Restitution::ZERO.with_combine_rule(CoefficientCombine::Min), 
                        Collider::capsule_y(0.5, 0.5),
                        /*CollisionGroups::new(
                            Group::GROUP_1,
                            Group::GROUP_2,
                        ),*/
                        //Mass(5.0),                    
                        ActiveCollisionTypes::KINEMATIC_STATIC,
                        RigidBody::KinematicPositionBased,
                        TransformInterpolation::default(),
                        //TranslationInterpolation,
                        KinematicCharacterController {
                            offset: CharacterLength::Absolute(0.3),
                            filter_flags: QueryFilterFlags::EXCLUDE_KINEMATIC,  
                            //snap_to_ground: Some(CharacterLength::Absolute(1.)),
                            ..KinematicCharacterController::default()
                        },
                        GravityScale(1.0),
                        //RigidBody::Kinematic     
                        //Collider::capsule(0.5, 1.0),
                    ))
                    .insert(AttackSpeed(0.5))
                    .insert(PlayerInput::default())
                    .insert(GameVelocity::default())
                    .insert(Facing(0) )                        
                    .insert(PrevState { translation: transform.translation, rotation: Facing(0)})
                    .insert(NearestNeighbourComponent)
                    .insert(TargetPos { position: transform.translation})
                    .insert(Player { id: *client_id })
                    .insert(LineOfSight::default())
                 
                    .id();

                lobby.players.insert(*client_id, player_entity);

                // Esto se puede mejorar... no debería ser necesario loopear por todas las cosas cercanas al jugador. 
                // Solo comparar el Vec3 del jugador existente con el Vec3 del nuevo jugador
                // Si están a menos de 12, spawnear.
                for (_entity, player, _player_transform) in players.iter() {

                    for (_, entity) in treeaccess.within_distance(transform.translation.into(), LINE_OF_SIGHT) {
                        // info!("entity {:?}", entity);

                        if let Some(entity) = entity {
                            if(entity == player_entity) {
                                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                                    id: *client_id,
                                    entity: player_entity,
                                    translation: transform.translation.into(),
                                    server_time: time.elapsed().as_millis()
                                })
                                .unwrap();
                
                                // Send message to only one client
                                server.send_message(player.id, ServerChannel::ServerMessages, message);
                                //*handle = colors.black.clone();
                            }
                        }                   
                       
                    }
                }

                // Spawn self.
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: *client_id,
                    entity: player_entity,
                    translation: transform.translation.into(),
                    server_time: time.elapsed().as_millis()
                })
                .unwrap();

                // Send message to only one client
                server.send_message(*client_id, ServerChannel::ServerMessages, message);

                /*let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: *client_id,
                    entity: player_entity,
                    translation: transform.translation.into(),
                })
                .unwrap();
                server.broadcast_message(ServerChannel::ServerMessages, message);*/
 
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Player {} disconnected: {}", client_id, reason);

                server_visualizer.remove_client(*client_id);
                //visualizer.remove_client(*client_id);
                if let Some(player_entity) = lobby.players.remove(client_id) {
                    commands.entity(player_entity).despawn();
                }

                let message = bincode::serialize(&ServerMessages::PlayerRemove { id: *client_id }).unwrap();
                server.broadcast_message(ServerChannel::ServerMessages, message);
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, ClientChannel::Command) {
            let command: PlayerCommand = bincode::deserialize(&message).unwrap();
            match command {
                PlayerCommand::Cast { mut cast_at } => {
                    println!("Received cast from client {}: {:?}", client_id, cast_at);

                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        if let Ok((_, _, player_transform)) = players.get(*player_entity) {
                            cast_at[1] = player_transform.translation[1];

                            let direction = (cast_at - player_transform.translation).normalize_or_zero();
                            let mut translation = player_transform.translation + (direction * 0.7);
                            translation[1] = 1.0;

                            let fireball_entity = spawn_fireball(&mut commands, &mut meshes, &mut materials, translation, direction);
                            let message = ServerMessages::SpawnProjectile {
                                entity: fireball_entity,
                                translation: translation.into(),
                            };
                            let message = bincode::serialize(&message).unwrap();
                            server.broadcast_message(ServerChannel::ServerMessages, message);
                        }
                    }
                },
                PlayerCommand::BasicAttack { entity } => {
                    println!("Received basic attack from client {}: {:?}", client_id, entity);
                    
                    if let (Some(player_entity)) = lobby.players.get_mut(&client_id) {

                        if let (Ok((_entity, _player, _player_transform)), Ok((monster_entity, _monster,  monster_transform))) = (players.get(*player_entity), monsters.get(entity)) {

                            println!("Player entity {:?} attacking monster_entity {:?}", player_entity, monster_entity);

                            let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
                            timer.pause(); // Timer pausado hasta que este en rango de ataque                      
                            
                            commands.entity(*player_entity).insert(Aggro {
                                enemy: monster_entity,
                                auto_attack: true,
                                enemy_translation: monster_transform.translation
                                //path: get_path_between_translations(player_transform.translation, monster_transform.translation, &map),
                               // timer: timer // El timer se debe definir al momento en que ya está en rango. Ya que el aspd puede variar mientras te acercas.
                           });
                        }
                    }              
                   
                },
                PlayerCommand::Move { destination_at } => {
                    println!("Received move action from client {}: {:?}", client_id, destination_at);
                
                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        println!("Existe jugador");
                        if let Ok((_entity, _player, player_transform)) = players.get(*player_entity) {
                            println!("Existe transform: {:?}", player_transform);
                            commands.entity(*player_entity).insert(Walking {
                                target_translation: destination_at,
                                path: get_path_between_translations(player_transform.translation, destination_at, &map),                               
                            })
                            .remove::<Aggro>()
                            .remove::<Attacking>()
                            .remove::<AttackingTimer>(); 
                        }   
                       
                    }

               
                }
            }
        }
        while let Some(message) = server.receive_message(client_id, ClientChannel::Input) {
            let input: PlayerInput = bincode::deserialize(&message).unwrap();
           
            if let Some(player_entity) = lobby.players.get(&client_id) {
                commands.entity(*player_entity).insert(input);
            }
        }
    }

}


pub fn setup_simple_camera(mut commands: Commands) {
    // camera
    commands.spawn((Camera3dBundle {
        transform: Transform::from_xyz(-20.5, 30.0, 20.5).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    },
    FlyCam)
    );
}




#[cfg(feature = "absolute_interpolation")]
pub fn network_send_absolute_position_system(
    mut server: ResMut<RenetServer>, 
    players: Query<(&Player, &LineOfSight)>,
    mut entities: Query<(Entity, &Transform, &mut PrevState), Changed<Transform>>,
    time: Res<Time>,
    fixed_time: Res<Time<Fixed>>,
) {
    for (player, line_of_sight) in players.iter() {
      
        for entity in line_of_sight.0.iter() {           

            if let Ok( (entity, transform, mut prev_state)) = entities.get_mut(*entity) {
                
                let quantized_position = transform.translation.div(TRANSLATION_PRECISION).as_ivec3(); // TRANSLATION_PRECISION == 0.001

                let delta_translation = quantized_position - prev_state.translation.div(TRANSLATION_PRECISION).as_ivec3();  
                          
                // println!("translation {:?} . servertie  {:?}",delta_translation, time.elapsed().as_millis());   
                //delta_translation != IVec3::ZERO
                if //&prev_state.rotation != rotation ||
                    delta_translation.x != 0 
                    || delta_translation.z != 0 
                    || delta_translation.y.abs() > 7 // La gravedad hace que se mueva poquito y no queremos madnar 100000 de packets
                {       
           
                    let message= ServerMessages::MoveAbsolute {
                        entity,
                        x: quantized_position.x,
                        y: quantized_position.y,
                        z: quantized_position.z,                      
                        server_time: time.elapsed().as_millis()
                    };

                    let sync_message = bincode::serialize(&message).unwrap();
                    // Send message to only one client
                    println!("Sent quantized_position {:?} .", quantized_position);   
                    server.send_message(player.id, ServerChannel::ServerMessages, sync_message);                    
       
                }  
            }         
        }
    }
    
    // posteriormente, se actualiza las ubicaciones antiguas de las entidades.
    for (_entity, transform, mut prev_state) in entities.iter_mut() {
        
        //println!("Se actualiza prev state  {:?}",transform.translation);   
        prev_state.translation = transform.translation;
        //prev_state.rotation = rotation.clone();
    }
}


 

#[cfg(not(feature = "absolute_interpolation"))]
pub fn network_send_delta_position_system(
    mut server: ResMut<RenetServer>, 
    players: Query<(&Player, &LineOfSight)>,
    mut entities: Query<(Entity, &Transform, &mut PrevState), Changed<Transform>>,
    time: Res<Time>,
    fixed_time: Res<Time<Fixed>>,
) {
    for (player, line_of_sight) in players.iter() {
      
        for entity in line_of_sight.0.iter() {           

            if let Ok( (entity, transform, mut prev_state)) = entities.get_mut(*entity) {
                
                let quantized_position = transform.translation.div(TRANSLATION_PRECISION).as_ivec3(); // TRANSLATION_PRECISION == 0.001
                let delta_translation = quantized_position - prev_state.translation.div(TRANSLATION_PRECISION).as_ivec3();     
                

                let a = fixed_time.overstep_fraction();
                let real_translation = prev_state.translation.lerp(transform.translation, a);

                
                // println!("translation {:?} . servertie  {:?}",delta_translation, time.elapsed().as_millis());   
                //delta_translation != IVec3::ZERO
                if //&prev_state.rotation != rotation ||
                 delta_translation.x != 0 
                || delta_translation.z != 0 
                || delta_translation.y.abs() > 7 // La gravedad hace que se mueva poquito y no queremos madnar 100000 de packets
                {       
                    //println!("translation Y {:?} . servertie  {:?}",delta_translation.y , time.elapsed().as_millis());   
                    //if &prev_state.rotation != rotation || delta_translation != IVec3::ZERO  {                                
                    //println!("real_translation {:?} . overstep_fraction  {:?}",real_translation,fixed_time.overstep_fraction());     
                    //println!("translation {:?} . servertie  {:?}",delta_translation, time.elapsed().as_millis());   
                    let message= ServerMessages::MoveDelta {
                        entity,
                        x: delta_translation.x,
                        y: delta_translation.y,
                        z: delta_translation.z,                      
                        server_time: time.elapsed().as_millis()
                    };

                    let sync_message = bincode::serialize(&message).unwrap();
                    // Send message to only one client
                    //println!("Sent message to client_id {:?} .", player.id);   
                    server.send_message(player.id, ServerChannel::ServerMessages, sync_message);                    
       
                }  
            }         
        }
    }
    
    // posteriormente, se actualiza las ubicaciones antiguas de las entidades.
    for (_entity, transform, mut prev_state) in entities.iter_mut() {
        
        //println!("Se actualiza prev state  {:?}",transform.translation);   
        prev_state.translation = transform.translation;
        //prev_state.rotation = rotation.clone();
    }
}



pub fn line_of_sight(
    mut server: ResMut<RenetServer>, 
    mut players: Query<(&Player, &Transform, &mut LineOfSight), With<Player>>,
    treeaccess: Res<NNTree>, 
    entities: Query<(Entity, &Transform, &SpriteId, &Facing)>,
) {
    for (player, transform, mut line_of_sight) in players.iter_mut() {

        let within_distance = treeaccess.within_distance(transform.translation.into(), LINE_OF_SIGHT);

        let entities_within_distance: Vec<Entity> = within_distance.iter().filter_map(|z| z.1).collect();

        if(entities_within_distance == line_of_sight.0) {
           // info!("No ha cambiado line of sight {:?}", entities_within_distance);
            continue;
        }      

        let old_set: HashSet<Entity> = line_of_sight.0.iter().cloned().collect();
        let new_set: HashSet<Entity> = entities_within_distance.iter().cloned().collect();

        let added: Vec<Entity> = new_set.difference(&old_set).cloned().collect();
        let removed: Vec<Entity> = old_set.difference(&new_set).cloned().collect();        
        
        //println!("Entered line of sight: {:?}", added);     // Output: Added: ["date"]
        //println!("Left line of sight: {:?}", removed);

       // Spawn all added entities into line of sight
        for (spawned_entity) in added.iter() {

            if let Ok( (entity, transform, sprite_id, facing)) = entities.get(*spawned_entity) {

                let message = bincode::serialize(&ServerMessages::SpawnEntity {
                    entity: entity,
                    sprite_id: sprite_id.clone(),
                    translation: transform.translation.into(),
                    facing: facing.clone()
                })
                .unwrap();
                server.send_message(player.id, ServerChannel::ServerMessages, message);
        
            }
        }

        // Despawn all removed entities from line of sight
        for (despawned_entity) in removed.iter() {
            let message = bincode::serialize(&ServerMessages::DespawnEntity { entity: *despawned_entity }).unwrap();
            server.send_message(player.id, ServerChannel::ServerMessages, message);              
        }        

        line_of_sight.0 = entities_within_distance;              
      
    }    

}

