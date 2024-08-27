

use bevy::log::{LogPlugin};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_renet::renet::transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use bevy_renet::transport::NetcodeServerPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy::input::common_conditions::input_toggle_active;
use bevy_renet::RenetServerPlugin;
use local_ip_address::local_ip;
use monsters::*;
use pathing::*;
use roguelike::*;
use std::collections::HashMap;
use std::{
    net::{SocketAddr, UdpSocket},
    time::SystemTime,
};

use std::ops::Div;
use std::ops::Mul;
use bevy_spatial::{kdtree::KDTree3, AutomaticUpdate, SpatialAccess};
use renet_visualizer::{RenetServerVisualizer, RenetVisualizerStyle};
use bevy_egui::{EguiContexts, EguiPlugin};

#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<ClientId, Entity>,
}

fn main() {
    App::new()   
       
        .add_plugins(DefaultPlugins.set(LogPlugin {
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
       // .add_plugins(MinimalPlugins)
        //.add_plugins(LogPlugin::default())
        .add_systems(
            Startup, (
                setup_level,
                setup_simple_camera,
                // setup_prohibited_areas.after(setup_level),
            )
        )
        .init_state::<AppState>()
     
        .add_plugins(MonstersPlugin)
        .add_plugins(RenetServerPlugin)
        .insert_resource(RenetServerVisualizer::<200>::new(
            RenetVisualizerStyle::default(),
        ))
        .insert_resource(ServerLobby::default())
        .insert_resource(Map::default())
        .insert_resource(create_renet_server())
        .add_plugins(NetcodeServerPlugin)
        .insert_resource(create_renet_transport())
        .add_systems(Update, server_ping)
        .add_systems(
            Update, 
            (
                server_events, 
                update_projectiles_system,          
                // update_visualizer_system      
            )
        )
        .add_systems(
            FixedUpdate, (
                // server_network_sync,      
                //server_network_sync_player_out,    
                network_send_delta_position_system,      
                click_move_players_system,
                //monster_test
            )
        )
        .add_systems(PostUpdate, projectile_on_removal_system)
        .run();
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
    visualizer.show_window(egui_contexts.ctx_mut());
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
                /*
                // Antiguo, cuando se mostraba todo a todo el mundo.
                for (entity, monster,  transform) in monsters.iter() {
                    let translation: [f32; 3] = transform.translation.into();
                    let message = bincode::serialize(&ServerMessages::SpawnMonster {
                        entity,
                        kind: monster.kind.clone(),
                        translation,
                    })
                    .unwrap();
                    server.send_message(*client_id, ServerChannel::ServerMessages, message);
                }*/

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
                    .spawn(PbrBundle {
                        mesh: meshes.add(Mesh::from(Capsule3d::default())),
                        material: materials.add(Color::srgb(0.8, 0.7, 0.6)),
                        transform,
                        ..Default::default()
                    })
                    .insert(PlayerInput::default())
                    .insert(Velocity::default())
                    .insert(Rotation(0) )                        
                    .insert(PrevState { translation: transform.translation, rotation: Rotation(0)})
                    .insert(NearestNeighbourComponent)
                    .insert(TargetPos { position: transform.translation})
                    .insert(Player { id: *client_id })
                 
                    .id();

                lobby.players.insert(*client_id, player_entity);

                // Esto se puede mejorar... no debería ser necesario loopear por todas las cosas cercanas al jugador. 
                // Solo comparar el Vec3 del jugador existente con el Vec3 del nuevo jugador
                // Si están a menos de 12, spawnear.
                for (_entity, player, player_transform) in players.iter() {

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
                PlayerCommand::BasicAttack { mut cast_at } => {
                    println!("Received basic attack from client {}: {:?}", client_id, cast_at);

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
                PlayerCommand::Move { mut destination_at } => {
                    println!("Received move action from client {}: {:?}", client_id, destination_at);

                    if let Some(mut player_entity) = lobby.players.get_mut(&client_id) {

                   
                        /*if let Ok((_, _, player_transform)) = players.get_mut(*player_entity) {
                            player_transform.translation = destination_at;
                        }*/
                        commands.entity(*player_entity).insert(command);
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

fn server_ping(mut server: ResMut<RenetServer>) {
    //let reliable_channel_id = ReliableChannelConfig::default().channel_id;

    // Receive message from channel
    for client_id in server.clients_id() {
        // The enum DefaultChannel describe the channels used by the default configuration
        while let Some(message) = server.receive_message(client_id, ClientChannel::Ping) {
            let client_message: ClientMessage = bincode::deserialize(&message).unwrap();
            match client_message {
                ClientMessage::Ping => {
                    info!("Got ping from {}!", client_id);
                    let pong = bincode::serialize(&ServerMessage::Pong).unwrap();
                    server.send_message(client_id, DefaultChannel::ReliableOrdered, pong);
                },
                
            }
        }

    }

}



pub fn setup_simple_camera(mut commands: Commands) {
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-20.5, 30.0, 20.5).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}



fn click_move_players_system(
    mut commands: Commands,
    mut query: Query<(&mut Velocity, &PlayerCommand, &mut Transform, Entity)>,
    map: Res<Map>
) {
    for (mut velocity, command, mut transform, entity) in query.iter_mut() {
        match command {
            PlayerCommand::Move { destination_at } => {  


              
                /*let start: Pos = Pos(
                    transform.translation.x.round() as i32, 
                    transform.translation.z.round() as i32
                );*/
                let goal: Pos = Pos(
                    destination_at.x as i32, 
                    destination_at.z as i32
                );    

                let target = get_next_step(transform.translation.into(), goal, &map); 

                if let Some(final_pos) = target {    

                    //info!("Final Pos: {:?}!", final_pos);    
                    // Se cambia el punto objetivo.
                    commands.entity(entity).insert(TargetPos {
                        position: final_pos,
                    });
             
                }

                /*if((destination_at.x != transform.translation.x || destination_at.z != transform.translation.z) && !map.blocked_paths.contains(&goal)) {                     
    
                    // info!("Start   {:?}!  Goal  {:?}!", start,goal);

                    //let succesors = get_succesors(&start, &map);                        
                    let astar_result = astar(
                        &start,
                        |p|  get_astar_successors(p, &map),
                        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
                        |p| *p==goal);


                    //info!("*Star Result {:?}! ",astar_result);    

               
                    if let Some(result) = astar_result{
                        let steps_vec = result.0;
                        let steps_left =  result.1;
                        let mut index = 1;
                        if(steps_left == 0) {
                            index = 0;
                        }
                  
  
                        if let Some(final_pos) = steps_vec.get(index) {
                     
                            let &Pos(x, z) = final_pos;

                            //info!("Final Pos: {:?}!", final_pos);    
                            // Se cambia el punto objetivo.
                            commands.entity(entity).insert(TargetPos {
                                position: Vec3 { x: x as f32, y: 2.0, z: z as f32},
                            });
                     
                       }
                      
                    }        
                  
                }*/
            },
            _  =>{}
        }
     

    }
}



fn update_projectiles_system(mut commands: Commands, mut projectiles: Query<(Entity, &mut Projectile)>, time: Res<Time>) {
    for (entity, mut projectile) in projectiles.iter_mut() {
        projectile.duration.tick(time.delta());
        if projectile.duration.finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn projectile_on_removal_system(mut server: ResMut<RenetServer>, mut removed_projectiles: RemovedComponents<Projectile>) {
    for entity in removed_projectiles.read() {
        let message = ServerMessages::DespawnProjectile { entity };
        let message = bincode::serialize(&message).unwrap();

        server.broadcast_message(ServerChannel::ServerMessages, message);
    }
}


#[allow(clippy::type_complexity)]
fn server_network_sync(mut server: ResMut<RenetServer>, query: Query<(Entity, &Transform), Or<(With<Player>, With<Projectile>, With<Monster>)>>) {
    let mut networked_entities = NetworkedEntities::default();
    for (entity, transform) in query.iter() {
        networked_entities.entities.push(entity);
        networked_entities.translations.push(transform.translation.into());
    }

    let sync_message = bincode::serialize(&networked_entities).unwrap();
    server.broadcast_message(ServerChannel::NetworkedEntities, sync_message);
}


#[allow(clippy::type_complexity)]
fn server_network_sync_player_out(
    mut server: ResMut<RenetServer>, 
    players: Query<(&Player, &Transform)>,
    mut query: Query<(Entity, &Transform), ( Or<(With<Player>, With<Projectile>, With<Monster>, With<NearestNeighbourComponent>)>)>,
    treeaccess: Res<NNTree>
  
) {
    for (player, transform) in players.iter() {

        let mut networked_entities = NetworkedEntities::default();

        for (_, entity) in treeaccess.within_distance(transform.translation.into(), LINE_OF_SIGHT) {
            // info!("entity {:?}", entity);
            if let Ok( (entity, mut transform)) = query.get_mut(entity.expect("No entity")) {

                networked_entities.entities.push(entity);
                networked_entities.translations.push(transform.translation.into());

            }          
           
        }
      
        let sync_message = bincode::serialize(&networked_entities).unwrap();
        // Send message to only one client
        server.send_message(player.id, ServerChannel::NetworkedEntities, sync_message);
        //*handle = colors.black.clone();

    }
 


    /*let mut networked_entities = NetworkedEntities::default();
    for (entity, transform) in query.iter() {
        networked_entities.entities.push(entity);
        networked_entities.translations.push(transform.translation.into());
    }

    let sync_message = bincode::serialize(&networked_entities).unwrap();
    // Send message to only one client
    server.send_message(client_id, ServerChannel::NetworkedEntities, sync_message);

    
    server.broadcast_message(ServerChannel::NetworkedEntities, sync_message);*/
}


pub fn network_send_delta_position_system(
    mut server: ResMut<RenetServer>, 
    players: Query<(&Player, &Transform)>,
    mut query: Query<(Entity, &Transform, &Rotation,  &mut PrevState), Changed<Transform>>,
    treeaccess: Res<NNTree>,
    time: Res<Time>,
) {
    for (player, transform) in players.iter() {

        for (_, entity) in treeaccess.within_distance(transform.translation.into(), LINE_OF_SIGHT) {


            if let Ok( (entity, mut transform, rotation, mut prev_state)) = query.get_mut(entity.expect("No entity")) {
                
                let quantized_position = transform.translation.div(TRANSLATION_PRECISION).as_ivec3(); // TRANSLATION_PRECISION == 0.01

                let delta_translation = quantized_position - prev_state.translation.div(TRANSLATION_PRECISION).as_ivec3();     

                if prev_state.rotation != *rotation || delta_translation != IVec3::ZERO {
                    println!("message_sent {:?} .", time.elapsed().as_millis());
                    println!("prev_translation {:?} .", prev_state.translation);                   
                    println!("delta_translation {:?} .", delta_translation);
                    println!("real_translation {:?} .", transform.translation);
                    
                    let message= ServerMessages::MoveDelta {
                        entity,
                        x: delta_translation.x,
                        y: delta_translation.y,
                        z: delta_translation.z,
                        rotation: rotation.clone(),
                        server_time: time.elapsed().as_millis(),
                        real_translation: transform.translation.into(),
                    };

                    let sync_message = bincode::serialize(&message).unwrap();
                    // Send message to only one client
                    server.send_message(player.id, ServerChannel::ServerMessages, sync_message);                    
       
                }
    
                prev_state.translation = transform.translation;
                prev_state.rotation = rotation.clone();


            }
         
        }
    }
}
