use bevy::log::{LogPlugin};
use bevy::prelude::*;
use bevy_renet::renet::transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use bevy_renet::renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use bevy_renet::transport::NetcodeServerPlugin;
use bevy_renet::RenetServerPlugin;
use local_ip_address::local_ip;
use roguelike::*;
use std::collections::HashMap;
use std::{
    net::{SocketAddr, UdpSocket},
    time::SystemTime,
};
use pathfinding::prelude::{astar, bfs};



#[derive(Debug, Default, Resource)]
pub struct Map {
    pub blocked_paths: Vec<Pos>
}

/* 
impl Pos {
    fn successors(&self) -> Vec<Pos> {
      let &Pos(x, z) = self;
      vec![Pos(x+1,z+1), Pos(x+1,z), Pos(x+1,z-1), Pos(x,z+1),
           Pos(x,z-1), Pos(x-1,z-1), Pos(x-1,z+1), Pos(x-1,z)]  
    }

    fn astar_successors(&self) -> Vec<(Pos, u32)> {
        let &Pos(x, z) = self;
        vec![Pos(x+1,z+1), Pos(x+1,z), Pos(x+1,z-1), Pos(x,z+1),
             Pos(x,z-1), Pos(x-1,z-1), Pos(x-1,z+1), Pos(x-1,z)]  
             .into_iter().map(|p| (p, 1)).collect()  // Le pone peso de 1 a todo
    }
}*/
  


const PLAYER_MOVE_SPEED: f32 = 5.0;

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
       // .add_plugins(MinimalPlugins)
        //.add_plugins(LogPlugin::default())
        .add_systems(
            Startup, (
                setup_level,
                setup_simple_camera,
                setup_prohibited_areas.after(setup_level),
            )
        )
        .add_plugins(RenetServerPlugin)
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
                server_network_sync,
                move_players_system,
                update_projectiles_system,                
            )
        )
        .add_systems(
            FixedUpdate, (
                apply_velocity_system,
                click_move_players_system
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


fn server_events(
    mut server_events: EventReader<ServerEvent>, 
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    players: Query<(Entity, &Player, &Transform)>,
) {

    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!("Client {client_id} connected");

                // Initialize other players for this new client
                for (entity, player, transform) in players.iter() {
                    let translation: [f32; 3] = transform.translation.into();
                    let message = bincode::serialize(&ServerMessages::PlayerCreate {
                        id: player.id,
                        entity,
                        translation,
                    })
                    .unwrap();
                    server.send_message(*client_id, ServerChannel::ServerMessages, message);
                }

                // Spawn new player
                let transform = Transform::from_xyz((fastrand::f32() - 0.5) * 40., 1.0, (fastrand::f32() - 0.5) * 40.);
                let player_entity = commands
                    .spawn(PbrBundle {
                        mesh: meshes.add(Mesh::from(Capsule3d::default())),
                        material: materials.add(Color::srgb(0.8, 0.7, 0.6)),
                        transform,
                        ..Default::default()
                    })
                    .insert(PlayerInput::default())
                    .insert(Velocity::default())
                    .insert(Player { id: *client_id })
                    .id();

                lobby.players.insert(*client_id, player_entity);

                let translation: [f32; 3] = transform.translation.into();
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: *client_id,
                    entity: player_entity,
                    translation,
                })
                .unwrap();
                server.broadcast_message(ServerChannel::ServerMessages, message);
 
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Player {} disconnected: {}", client_id, reason);
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

pub fn setup_prohibited_areas(mut map: ResMut<Map>, mut buildings: Query<(Entity, &mut Building)>) {
   
    for (_entity, mut building) in buildings.iter_mut() {
        info!("Building {:?}!", building.blocked_paths);
        map.blocked_paths.append(&mut building.blocked_paths);
        info!("blocked_paths {:?}!", map);
    }

}

pub fn setup_simple_camera(mut commands: Commands) {
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-20.5, 30.0, 20.5).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}

fn get_succesors(current_pos: &Pos, mut map: &ResMut<Map>) -> Vec<Pos> {

    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    //info!("blocked_paths   {:?}!", blocked_paths);
    let mut possible_positions =  vec![Pos(x+1,z+1), Pos(x+1,z), Pos(x+1,z-1), Pos(x,z+1),
    Pos(x,z-1), Pos(x-1,z-1), Pos(x-1,z+1), Pos(x-1,z)];

    possible_positions.retain(|pos| !blocked_paths.contains(&pos));


    info!("possible_positions   {:?}!", possible_positions);
    // se le agrega el peso
    possible_positions

}
fn  get_astar_successors(current_pos: &Pos, mut map: &ResMut<Map>) -> Vec<(Pos, u32)> {

    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    //info!("blocked_paths   {:?}!", blocked_paths);

  
     let mut possible_positions =  vec![];

   // Si no hay nada arriba, puede ir hacia arriba
    if(!blocked_paths.contains(&Pos(x,z+1))) {
        possible_positions.push(Pos(x,z+1));
    }
    // Si no hay nada derecha, puede ir hacia derecha
    if(!blocked_paths.contains(&Pos(x+1,z))) {
        possible_positions.push(Pos(x+1,z));
    }
    // Si no hay nada izquierda, puede ir hacia izquierda
    if(!blocked_paths.contains(&Pos(x-1,z))) {
        possible_positions.push(Pos(x-1,z));
    }
    // Si no hay nada abajo, puede ir hacia abajo
    if(!blocked_paths.contains(&Pos(x,z-1))) {
        possible_positions.push(Pos(x,z-1));
    }
    // Si tiene nada arriba ni a la izq, diagonal arriba izq.
    if(!blocked_paths.contains(&Pos(x,z+1)) && !blocked_paths.contains(&Pos(x-1,z)) && !blocked_paths.contains(&Pos(x-1,z+1))) {
        possible_positions.push(Pos(x-1,z+1));
    }
    // Si tiene nada arriba ni a la derecha, diagonal arriba derecha.
    if(!blocked_paths.contains(&Pos(x,z+1)) && !blocked_paths.contains(&Pos(x+1,z)) && !blocked_paths.contains(&Pos(x+1,z+1))) {
        possible_positions.push(Pos(x+1,z+1));
    }
    // Si tiene nada abajo ni a la izq, diagonal abajo izq.
    if(!blocked_paths.contains(&Pos(x,z-1)) && !blocked_paths.contains(&Pos(x-1,z)) && !blocked_paths.contains(&Pos(x-1,z-1))) {
        possible_positions.push(Pos(x-1,z-1));
    }
    // Si tiene nada abajo ni a la derecha, diagonal abajo derecha.
    if(!blocked_paths.contains(&Pos(x,z-1)) && !blocked_paths.contains(&Pos(x+1,z)) && !blocked_paths.contains(&Pos(x+1,z-1))) {
        possible_positions.push(Pos(x+1,z-1));
    }


    /*let mut possible_positions =  vec![
        Pos(x+1,z+1), 
        Pos(x+1,z), 
        Pos(x+1,z-1), 
        Pos(x,z+1), 
        Pos(x,z-1), 
        Pos(x-1,z-1), 
        Pos(x-1,z+1), 
        Pos(x-1,z)
    ];*/


    possible_positions.retain(|pos| !blocked_paths.contains(&pos));


    info!("possible_positions   {:?}!", possible_positions);

    possible_positions.into_iter().map(|p| (p, 1)).collect()

}

fn click_move_players_system(
    mut query: Query<(&mut Velocity, &PlayerCommand, &mut Transform)>,
    mut map: ResMut<Map>
) {
    for (mut velocity, command, mut transform) in query.iter_mut() {
        match command {
            PlayerCommand::Move { mut destination_at } => {  

                let start: Pos = Pos(
                    transform.translation.x.round() as i32, 
                    transform.translation.z.round() as i32
                );
                let goal: Pos = Pos(
                    destination_at.x as i32, 
                    destination_at.z as i32
                );    

                if((destination_at.x != transform.translation.x || destination_at.z != transform.translation.z) && !map.blocked_paths.contains(&goal)) {                     
    
                    info!("Start   {:?}!  Goal  {:?}!", start,goal);

                    //let succesors = get_succesors(&start, &map);                        
                    let astar_result = astar(
                        &start,
                        |p|  get_astar_successors(p, &map),
                        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
                        |p| *p==goal);


                    info!("*Star Result {:?}! ",astar_result);    
                    // let bfs_result = bfs( &start, |p| succesors.clone(), |p| *p == goal);
                    /*let astar_result = astar(
                        &start,
                        |p| p.astar_successors(),
                        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
                        |p| *p==goal);*/

              
                       // info!("bfs_ Result {:?}! ",bfs_result);    

               
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
                            
                            //info!("*Star Result Next step  x: {:?}! z: {:?}!", x, z);    
                            info!("Translation: {:?}!", transform.translation);   
                            let distance_x = x as f32 - transform.translation.x;
                            //info!("*Star Result distance_x  x: {:?}!", distance_x);   

                            if distance_x.abs() < 0.2 && (steps_left == 1 || steps_left == 0)  {
                                velocity.0.x = 0.0;
                                transform.translation.x = destination_at.x;
                            }        
                            else if distance_x > 0.0 {
                                velocity.0.x = PLAYER_MOVE_SPEED;
                            }
                            else if  distance_x < 0.0 {
                                velocity.0.x = -PLAYER_MOVE_SPEED;
                            }
                        
                            let distance_z = z as f32 - transform.translation.z;         

                            if  distance_z.abs() < 0.2  && (steps_left == 1 || steps_left == 0)  {
                                velocity.0.z = 0.0; 
                                transform.translation.z = destination_at.z;
                            }                    
                            else if distance_z > 0.0 {
                                velocity.0.z = PLAYER_MOVE_SPEED;
                            }
                            else if  distance_z < 0.0 {
                                velocity.0.z = -PLAYER_MOVE_SPEED;
                            } 

                           
                       }
                      
                    }
                  
                    //transform.translation.x = destination_at.x;
                    //transform.translation.z = destination_at.z;
             
                    /* 
                    let distance_x = destination_at.x - transform.translation.x;

                    if distance_x.abs() < 0.1  {
                        velocity.0.x = 0.0;
                        transform.translation.x = destination_at.x;
                    }        
                    else if distance_x > 0.0 {
                        velocity.0.x = PLAYER_MOVE_SPEED;
                    }
                    else if  distance_x < 0.0 {
                        velocity.0.x = -PLAYER_MOVE_SPEED;
                    }
                 

                    let distance_z = destination_at.z - transform.translation.z;             

                    if  distance_z.abs() < 0.1  {
                        velocity.0.z = 0.0; 
                        transform.translation.z = destination_at.z;
                    }                    
                    else if distance_z > 0.0 {
                        velocity.0.z = PLAYER_MOVE_SPEED;
                    }
                    else if  distance_z < 0.0 {
                        velocity.0.z = -PLAYER_MOVE_SPEED;
                    }*/

                  
                  
                }
            },
            _  =>{}
        }
     

    }
}

fn move_players_system(mut query: Query<(&mut Velocity, &PlayerInput)>) {
    for (mut velocity, input) in query.iter_mut() {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        let direction = Vec2::new(x, y).normalize_or_zero();
        velocity.0.x = direction.x * PLAYER_MOVE_SPEED;
        velocity.0.z = direction.y * PLAYER_MOVE_SPEED;
    }
}

fn apply_velocity_system(mut query: Query<(&Velocity, &mut Transform)>, time: Res<Time>) {
    for (velocity, mut transform) in query.iter_mut() {
        transform.translation += velocity.0 * time.delta_seconds();
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
fn server_network_sync(mut server: ResMut<RenetServer>, query: Query<(Entity, &Transform), Or<(With<Player>, With<Projectile>)>>) {
    let mut networked_entities = NetworkedEntities::default();
    for (entity, transform) in query.iter() {
        networked_entities.entities.push(entity);
        networked_entities.translations.push(transform.translation.into());
    }

    let sync_message = bincode::serialize(&networked_entities).unwrap();
    server.broadcast_message(ServerChannel::NetworkedEntities, sync_message);
}
