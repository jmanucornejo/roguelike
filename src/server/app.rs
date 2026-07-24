use bevy::ecs::schedule::ScheduleLabel;
// use avian3d::prelude::{Collider, GravityScale, LockedAxes, RigidBody};
use bevy::log::LogPlugin;
use bevy_obj::ObjPlugin;

///use avian3d::math::{AdjustPrecision, Quaternion, Scalar, Vector};
///use avian3d::prelude::{CoefficientCombine, Collider, ColliderParent, Collisions, Friction, GravityScale, LinearVelocity, LockedAxes, Mass, Position, PostProcessCollisions, Restitution, RigidBody, Rotation, Sensor};
// use avian3d::{PhysicsPlugins};
use bevy::prelude::*;

use bevy_renet::netcode::{
    NetcodeServerPlugin, NetcodeServerTransport, ServerAuthentication, ServerConfig,
};
//use bevy_renet::renet::transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use bevy_renet::renet::ServerEvent;
//use bevy_renet::transport::NetcodeServerPlugin;
use crate::server::gameplay::monsters::*;
use crate::server::gameplay::pathing::*;
use crate::server::state::*;
use bevy::input::common_conditions::input_toggle_active;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_renet::{RenetServer, RenetServerEvent, RenetServerPlugin};
use local_ip_address::local_ip;

use crate::server::gameplay::projectiles::spawn_fireball;
use crate::server::gameplay::spatial::{
    AutomaticUpdate, NNTree, NearestNeighbourComponent, SpatialAccess,
};
use crate::server::network::replication::{LineOfSight, PrevState};
use crate::shared::constants::*;
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::entities::{AttackSpeed, Player};
use crate::shared::network::{channels::*, messages::*};
use crate::shared::states::ServerState;
use crate::world::setup_level;
use bevy_egui::{EguiContexts, EguiPlugin};
use bevy_flycam::prelude::*;
use bevy_rapier3d::prelude::*;
use renet_visualizer::{RenetServerVisualizer, RenetVisualizerStyle};
use std::collections::HashSet;
use std::ops::Div;
use std::{
    net::{SocketAddr, UdpSocket},
    time::SystemTime,
};

pub fn run() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(LogPlugin {
        filter: "info,wgpu_core=warn,wgpu_hal=off,rechannel=warn".into(),
        level: bevy::log::Level::DEBUG,
        ..Default::default()
    }))
    .add_plugins(EguiPlugin::default())
    .add_plugins(WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)))
    .add_plugins(PathingPlugin)
    .add_plugins(AutomaticUpdate::<NearestNeighbourComponent>::new())
    .add_plugins(ObjPlugin)
    //.add_plugins(PhysicsPlugins::default())
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().with_default_system_setup(false))
    .add_plugins(RapierDebugRenderPlugin { ..default() })
    .add_plugins(NoCameraPlayerPlugin)
    // .add_plugins(MinimalPlugins)
    //.add_plugins(LogPlugin::default())
    .add_systems(
        Startup,
        (
            setup_level,
            setup_simple_camera,
            // setup_prohibited_areas.after(setup_level),
        ),
    )
    .init_state::<ServerState>()
    .add_plugins((
        // crate::server::gameplay::physics::ServerPhysicsPlugin,
        MonstersPlugin,
        crate::server::network::clock_sync::ServerClockSyncPlugin,
        // crate::server::network::clock_server::ClockServerPlugin, // prototype
        crate::server::gameplay::combat::CombatPlugin,
    ))
    .add_plugins(RenetServerPlugin)
    .insert_resource(ServerVisualizer(RenetServerVisualizer::<200>::new(
        RenetVisualizerStyle::default(),
    )))
    .insert_resource(ServerLobby::default())
    .init_resource::<PendingServerEvents>()
    .insert_resource(SnapshotTimer(Timer::from_seconds(
        1.0 / NETWORK_SNAPSHOT_HZ,
        TimerMode::Repeating,
    )))
    .insert_resource(Time::<Fixed>::from_hz(60.0))
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
        ),
    )
    .add_observer(queue_server_event)
    .add_systems(FixedUpdate, line_of_sight)
    .insert_resource(TimestepMode::Fixed {
        dt: 1.0 / 60.0, // 60 FPS physics update
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
        schedule.configure_sets(
            (
                PhysicsSet::SyncBackend,
                PhysicsSet::StepSimulation,
                PhysicsSet::Writeback,
            )
                .chain(),
        );
    })
    .add_systems(
        FixedUpdate,
        run_physics_schedule.before(crate::server::gameplay::pathing::get_velocity),
    );

    app.add_systems(FixedPostUpdate, network_send_position_snapshots);

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
        current_time,
        max_clients: 64,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let inbound_server_addr = SocketAddr::new(local_ip().unwrap(), 42069);

    let socket = UdpSocket::bind(inbound_server_addr).unwrap();

    NetcodeServerTransport::new(server_config, socket).unwrap()
}

fn update_visualizer_system(
    egui_contexts: EguiContexts,
    mut visualizer: ResMut<ServerVisualizer>,
    server: Res<RenetServer>,
) {
    visualizer.update(&server.0);
    //visualizer.show_window(egui_contexts.ctx_mut());
}

fn server_events(
    mut server_events: ResMut<PendingServerEvents>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    players: Query<(Entity, &Player, &Transform)>,
    monsters: Query<(Entity, &Monster, &Transform), With<Monster>>,
    mut server_visualizer: ResMut<ServerVisualizer>,
    time: Res<Time>,
    map: Res<Map>,
) {
    for event in server_events.0.drain(..) {
        match event {
            PendingServerEvent::Connected(client_id) => {
                println!("Client {client_id} connected");
                server_visualizer.add_client(client_id);
                // Get player spawning poing.
                let transform = Transform::from_xyz(
                    (fastrand::f32() - 0.5) * 40.,
                    1.0,
                    (fastrand::f32() - 0.5) * 40.,
                );
                info!("entity  transform {:?}", transform);

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
                        player_character_controller(),
                        GravityScale(1.0),
                        //RigidBody::Kinematic
                        //Collider::capsule(0.5, 1.0),
                    ))
                    .insert(AttackSpeed(0.5))
                    .insert(PlayerInput::default())
                    .insert(GameVelocity::default())
                    .insert(Facing(0))
                    .insert(PrevState {
                        translation: transform.translation,
                        rotation: Facing(0),
                    })
                    .insert(NearestNeighbourComponent)
                    .insert(TargetPos {
                        position: transform.translation,
                    })
                    .insert(Player { id: client_id })
                    .insert(LineOfSight::default())
                    .id();

                lobby.players.insert(client_id, player_entity);

                // Spawn self.
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: client_id,
                    entity: player_entity,
                    translation: transform.translation.into(),
                    server_time: time.elapsed().as_millis(),
                })
                .unwrap();

                // Send message to only one client
                server.send_message(client_id, ServerChannel::ServerMessages, message);

                /*let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: *client_id,
                    entity: player_entity,
                    translation: transform.translation.into(),
                })
                .unwrap();
                server.broadcast_message(ServerChannel::ServerMessages, message);*/
            }
            PendingServerEvent::Disconnected(client_id, reason) => {
                println!("Player {} disconnected: {}", client_id, reason);

                server_visualizer.remove_client(client_id);
                //visualizer.remove_client(*client_id);
                if let Some(player_entity) = lobby.players.remove(&client_id) {
                    // The player may already have been removed by another gameplay
                    // system (for example, death handling). Disconnect cleanup must
                    // therefore be safe to run against a stale lobby entity.
                    commands.entity(player_entity).try_despawn();
                }

                let message =
                    bincode::serialize(&ServerMessages::PlayerRemove { id: client_id }).unwrap();
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

                            let direction =
                                (cast_at - player_transform.translation).normalize_or_zero();
                            let mut translation = player_transform.translation + (direction * 0.7);
                            translation[1] = 1.0;

                            let fireball_entity = spawn_fireball(
                                &mut commands,
                                &mut meshes,
                                &mut materials,
                                translation,
                                direction,
                            );
                            let message = ServerMessages::SpawnProjectile {
                                entity: fireball_entity,
                                translation: translation.into(),
                            };
                            let message = bincode::serialize(&message).unwrap();
                            server.broadcast_message(ServerChannel::ServerMessages, message);
                        }
                    }
                }
                PlayerCommand::BasicAttack { entity } => {
                    println!(
                        "Received basic attack from client {}: {:?}",
                        client_id, entity
                    );

                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        if let (
                            Ok((_entity, _player, _player_transform)),
                            Ok((monster_entity, _monster, monster_transform)),
                        ) = (players.get(*player_entity), monsters.get(entity))
                        {
                            println!(
                                "Player entity {:?} attacking monster_entity {:?}",
                                player_entity, monster_entity
                            );

                            let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
                            timer.pause(); // Timer pausado hasta que este en rango de ataque

                            commands.entity(*player_entity).insert(Aggro {
                                enemy: monster_entity,
                                auto_attack: true,
                                enemy_translation: monster_transform.translation, //path: get_path_between_translations(player_transform.translation, monster_transform.translation, &map),
                                                                                  // timer: timer // El timer se debe definir al momento en que ya está en rango. Ya que el aspd puede variar mientras te acercas.
                            });
                        }
                    }
                }
                PlayerCommand::Move { destination_at } => {
                    println!(
                        "Received move action from client {}: {:?}",
                        client_id, destination_at
                    );

                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        println!("Existe jugador");
                        if let Ok((_entity, _player, player_transform)) =
                            players.get(*player_entity)
                        {
                            println!("Existe transform: {:?}", player_transform);
                            commands
                                .entity(*player_entity)
                                .insert(Walking {
                                    target_translation: destination_at,
                                    path: get_path_between_translations(
                                        player_transform.translation,
                                        destination_at,
                                        &map,
                                    ),
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

fn queue_server_event(trigger: On<RenetServerEvent>, mut pending: ResMut<PendingServerEvents>) {
    match &trigger.event().0 {
        ServerEvent::ClientConnected { client_id } => {
            pending.0.push(PendingServerEvent::Connected(*client_id));
        }
        ServerEvent::ClientDisconnected { client_id, reason } => {
            pending.0.push(PendingServerEvent::Disconnected(
                *client_id,
                reason.to_string(),
            ));
        }
    }
}

pub fn setup_simple_camera(mut commands: Commands) {
    // camera
    commands.spawn((
        Camera3d {
            ..Default::default()
        },
        Transform::from_xyz(-20.5, 30.0, 20.5).looking_at(Vec3::ZERO, Vec3::Y),
        FlyCam,
    ));
}

fn network_send_position_snapshots(
    mut server: ResMut<RenetServer>,
    players: Query<(&Player, &LineOfSight)>,
    mut entities: Query<(Entity, &Transform, &mut PrevState)>,
    time: Res<Time>,
    mut snapshot_timer: ResMut<SnapshotTimer>,
) {
    if !snapshot_timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let server_time = time.elapsed().as_millis();
    let mut snapshots = Vec::new();

    for (entity, transform, mut previous) in &mut entities {
        let quantized_position = transform.translation.div(TRANSLATION_PRECISION).as_ivec3();
        let previous_quantized = previous.translation.div(TRANSLATION_PRECISION).as_ivec3();
        let delta = quantized_position - previous_quantized;

        if delta.x != 0 || delta.z != 0 || delta.y.abs() > 7 {
            snapshots.push(EntitySnapshot {
                entity,
                x: quantized_position.x,
                y: quantized_position.y,
                z: quantized_position.z,
                server_time,
            });
            previous.translation = transform.translation;
        }
    }

    let encoded_snapshots: Vec<_> = snapshots
        .into_iter()
        .map(|snapshot| {
            (
                snapshot.entity,
                bincode::serialize(&snapshot).expect("position snapshot should serialize"),
            )
        })
        .collect();

    for (player, line_of_sight) in &players {
        for (entity, snapshot) in &encoded_snapshots {
            if line_of_sight.0.contains(entity) {
                server.send_message(
                    player.id,
                    ServerChannel::NetworkedEntities,
                    snapshot.clone(),
                );
            }
        }
    }
}

pub fn line_of_sight(
    mut server: ResMut<RenetServer>,
    mut viewers: Query<(Entity, &Player, &Transform, &mut LineOfSight), With<Player>>,
    players: Query<(&Player, &Transform), With<Player>>,
    treeaccess: Res<NNTree>,
    entities: Query<(Entity, &Transform, &SpriteId, &Facing, Option<&Health>)>,
    time: Res<Time>,
) {
    for (viewer_entity, player, transform, mut line_of_sight) in viewers.iter_mut() {
        let within_distance = treeaccess.within_distance(transform.translation, LINE_OF_SIGHT);

        let entities_within_distance: Vec<Entity> =
            within_distance.iter().filter_map(|z| z.1).collect();

        if entities_within_distance == line_of_sight.0 {
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
        for spawned_entity in added.iter() {
            // The connecting client already received its own PlayerCreate message.
            if *spawned_entity == viewer_entity {
                continue;
            }

            if let Ok((spawned_player, spawned_transform)) = players.get(*spawned_entity) {
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: spawned_player.id,
                    entity: *spawned_entity,
                    translation: spawned_transform.translation.into(),
                    server_time: time.elapsed().as_millis(),
                })
                .unwrap();
                server.send_message(player.id, ServerChannel::ServerMessages, message);
                continue;
            }

            if let Ok((entity, transform, sprite_id, facing, health)) =
                entities.get(*spawned_entity)
            {
                let health_message = match health {
                    None => None,
                    Some(health) => Some(health.clone()),
                };

                let message = bincode::serialize(&ServerMessages::SpawnEntity {
                    entity,
                    sprite_id: sprite_id.clone(),
                    translation: transform.translation.into(),
                    facing: facing.clone(),
                    health: health_message,
                    server_time: time.elapsed().as_millis(),
                })
                .unwrap();
                server.send_message(player.id, ServerChannel::ServerMessages, message);
            }
        }

        // Despawn all removed entities from line of sight
        for despawned_entity in removed.iter() {
            let message = bincode::serialize(&ServerMessages::DespawnEntity {
                entity: *despawned_entity,
            })
            .unwrap();
            server.send_message(player.id, ServerChannel::ServerMessages, message);
        }

        line_of_sight.0 = entities_within_distance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_of_sight_system_has_valid_bevy_parameters() {
        let mut world = World::new();
        world.insert_resource(RenetServer::new(connection_config()));
        world.init_resource::<NNTree>();
        world.init_resource::<Time>();

        let mut schedule = Schedule::default();
        schedule.add_systems(line_of_sight);
        schedule.run(&mut world);
    }
}
