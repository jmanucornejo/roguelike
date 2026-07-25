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
use bevy_renet::renet::{ClientId, ServerEvent};
//use bevy_renet::transport::NetcodeServerPlugin;
use crate::server::gameplay::monsters::*;
use crate::server::gameplay::pathing::*;
use crate::server::state::*;
use bevy::input::common_conditions::input_toggle_active;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_renet::{RenetServer, RenetServerEvent, RenetServerPlugin};
use local_ip_address::local_ip;

use crate::server::gameplay::spatial::{
    AutomaticUpdate, NNTree, NearestNeighbourComponent, SpatialAccess,
};
use crate::server::gameplay::spells::{AuthoritativeCast, RequestSpellCast};
use crate::server::network::replication::{LineOfSight, PrevState};
use crate::server::persistence::{
    AccountId, CharacterRecord, CharacterSnapshot, PersistenceClient, PersistenceInbox,
    PersistenceRequest, PersistenceResponse, PersistenceStatus, PersistentCharacter,
};
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
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
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
        crate::server::gameplay::spells::SpellsPlugin,
        crate::server::persistence::PersistencePlugin,
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
    .init_resource::<CharacterPersistenceQueue>()
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
    .add_systems(FixedUpdate, line_of_sight.after(PhysicsSet::Writeback))
    .insert_resource(TimestepMode::Fixed {
        dt: 1.0 / 60.0, // 60 FPS physics update
        substeps: 1,
    });

    app.add_systems(FixedPostUpdate, network_send_position_snapshots);

    app.run();
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

struct PlayerSpawn {
    character_id: CharacterId,
    transform: Transform,
    facing: Facing,
    health: Health,
    mana: Mana,
    persistent: Option<PersistentCharacter>,
}

impl PlayerSpawn {
    fn ephemeral() -> Self {
        Self {
            // Zero is reserved for an in-memory character when persistence is
            // disabled or unavailable.
            character_id: CharacterId(0),
            transform: Transform::from_xyz(
                (fastrand::f32() - 0.5) * 40.,
                1.0,
                (fastrand::f32() - 0.5) * 40.,
            ),
            facing: Facing(0),
            health: Health {
                current: 40,
                max: 40,
            },
            mana: Mana {
                current: 10,
                max: 10,
            },
            persistent: None,
        }
    }

    fn from_record(record: CharacterRecord, map: &Map) -> Self {
        let persistent = PersistentCharacter::from_record(&record);
        let saved_translation = Vec3::new(record.position_x, record.position_y, record.position_z);
        let spawn_translation = resolve_persistent_spawn(saved_translation, map);
        if spawn_translation != saved_translation {
            warn!(
                "Persistent character {} was saved on blocked tile {:?}; relocating it to {:?}",
                record.id,
                world_cell(saved_translation),
                spawn_translation
            );
        }
        Self {
            character_id: CharacterId(record.id),
            transform: Transform::from_translation(spawn_translation),
            facing: Facing(record.facing),
            health: Health {
                current: record.hp,
                max: record.max_hp,
            },
            mana: Mana {
                current: record.sp,
                max: record.max_sp,
            },
            persistent: Some(persistent),
        }
    }
}

fn world_cell(translation: Vec3) -> Pos {
    Pos(translation.x.round() as i32, translation.z.round() as i32)
}

fn default_character_spawn() -> Vec3 {
    Vec3::new(
        DEFAULT_CHARACTER_SPAWN[0],
        DEFAULT_CHARACTER_SPAWN[1],
        DEFAULT_CHARACTER_SPAWN[2],
    )
}

fn resolve_persistent_spawn(saved_translation: Vec3, map: &Map) -> Vec3 {
    let saved_cell = world_cell(saved_translation);

    // The original database default was the map origin, which is inside the
    // fixed wall. Check it explicitly as well as the navigation mask so an old
    // record is safe even if it loads before all navigation cells are built.
    if matches!(saved_cell, Pos(0, 0) | Pos(-12, 16)) || map.blocked_paths.contains(&saved_cell) {
        default_character_spawn()
    } else {
        saved_translation
    }
}

fn spawn_player(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    client_id: ClientId,
    spawn: PlayerSpawn,
) -> Entity {
    let attack_speed = 0.5;
    let transform = spawn.transform;
    let facing = spawn.facing.clone();
    let mut player = commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.5, 1.)))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        transform,
        LockedAxes::ROTATION_LOCKED,
        Collider::capsule_y(0.5, 0.5),
        ActiveCollisionTypes::KINEMATIC_STATIC,
        RigidBody::KinematicPositionBased,
        TransformInterpolation::default(),
        player_character_controller(),
        GravityScale(1.0),
        AttackSpeed(attack_speed),
        PlayerInput::default(),
    ));
    player.insert((
        GameVelocity::default(),
        facing.clone(),
        spawn.health,
        spawn.mana,
        spawn.character_id,
        PrevState {
            translation: transform.translation,
            rotation: facing,
        },
        NearestNeighbourComponent,
        TargetPos {
            position: transform.translation,
        },
        Player { id: client_id },
        LineOfSight::default(),
    ));

    if let Some(persistent) = spawn.persistent {
        player.insert(persistent);
    }

    player.id()
}

fn send_player_create(
    server: &mut RenetServer,
    recipient: ClientId,
    client_id: ClientId,
    player_entity: Entity,
    transform: &Transform,
    character_id: CharacterId,
    facing: &Facing,
    health: &Health,
    mana: &Mana,
    attack_speed: f32,
    server_time: u128,
) {
    let message = bincode::serialize(&ServerMessages::PlayerCreate {
        id: client_id,
        entity: player_entity,
        character_id,
        translation: transform.translation.into(),
        facing: facing.clone(),
        health: health.clone(),
        mana: mana.clone(),
        attack_speed,
        server_time,
    })
    .expect("player create message should serialize");
    server.send_message(recipient, ServerChannel::ServerMessages, message);
}

fn spawn_and_announce_player(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    lobby: &mut ServerLobby,
    server: &mut RenetServer,
    client_id: ClientId,
    spawn: PlayerSpawn,
    server_time: u128,
) -> Entity {
    let character_id = spawn.character_id;
    let transform = spawn.transform;
    let facing = spawn.facing.clone();
    let health = spawn.health.clone();
    let mana = spawn.mana.clone();
    let player_entity = spawn_player(commands, meshes, materials, client_id, spawn);

    lobby.players.insert(client_id, player_entity);
    if character_id.0 == 0 {
        lobby.characters.remove(&client_id);
    } else {
        lobby.characters.insert(client_id, character_id);
    }
    send_player_create(
        server,
        client_id,
        client_id,
        player_entity,
        &transform,
        character_id,
        &facing,
        &health,
        &mana,
        0.5,
        server_time,
    );
    player_entity
}

fn disconnect_snapshot(
    character_id: CharacterId,
    transform: Option<&Transform>,
    persistent: Option<&PersistentCharacter>,
    health: Option<&Health>,
    mana: Option<&Mana>,
    facing: Option<&Facing>,
    last_saved: Option<&CharacterSnapshot>,
) -> Result<(CharacterSnapshot, Vec<&'static str>), Vec<&'static str>> {
    let mut missing = Vec::new();
    if transform.is_none() {
        missing.push("Transform");
    }
    if persistent.is_none() {
        missing.push("PersistentCharacter");
    }
    if health.is_none() {
        missing.push("Health");
    }
    if mana.is_none() {
        missing.push("Mana");
    }
    if facing.is_none() {
        missing.push("Facing");
    }

    if let (Some(transform), Some(persistent), Some(health), Some(mana), Some(facing)) =
        (transform, persistent, health, mana, facing)
    {
        return Ok((
            persistent.snapshot(character_id, transform, facing, health, mana),
            missing,
        ));
    }

    // A disconnect can arrive while other deferred ECS commands are removing
    // components. Start with the last complete database snapshot, then overlay
    // every current authoritative value that remains on the entity.
    let Some(mut snapshot) = last_saved.cloned() else {
        return Err(missing);
    };
    snapshot.character_id = character_id;

    if let Some(transform) = transform {
        snapshot.position = transform.translation.into();
    }
    if let Some(facing) = facing {
        snapshot.facing = facing.0;
    }
    if let Some(health) = health {
        snapshot.hp = health.current;
        snapshot.max_hp = health.max;
    }
    if let Some(mana) = mana {
        snapshot.sp = mana.current;
        snapshot.max_sp = mana.max;
    }
    if let Some(persistent) = persistent {
        snapshot.base_level = persistent.base_level;
        snapshot.base_experience = persistent.base_experience;
        snapshot.job_level = persistent.job_level;
        snapshot.job_experience = persistent.job_experience;
        snapshot.zeny = persistent.zeny;
        snapshot.map_name.clone_from(&persistent.map_name);
    }

    Ok((snapshot, missing))
}

fn request_character_load(
    client_id: ClientId,
    persistence: &PersistenceClient,
    queue: &mut CharacterPersistenceQueue,
) -> bool {
    let request_id = queue.next_request_id();
    match persistence.send(PersistenceRequest::LoadOrCreateDefaultCharacter {
        request_id,
        account_id: AccountId(client_id),
    }) {
        Ok(()) => {
            queue.load_requests.insert(request_id, client_id);
            true
        }
        Err(error) => {
            error!("Could not request character load for client {client_id}: {error}");
            false
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CharacterSaveRequest {
    Queued,
    Deferred,
    AlreadyCovered,
    Failed,
}

fn request_character_save(
    persistence: &PersistenceClient,
    queue: &mut CharacterPersistenceQueue,
    mut snapshot: CharacterSnapshot,
    reason: &'static str,
) -> CharacterSaveRequest {
    let character_id = snapshot.character_id;
    let comparable_snapshot = snapshot.clone().without_revision();

    if let Some(in_flight) = queue.saves_in_flight.get(&character_id) {
        if in_flight == &comparable_snapshot {
            // This is the newest authoritative state again, so an older deferred
            // snapshot must no longer be written after the current request.
            queue.deferred_saves.remove(&character_id);
            return CharacterSaveRequest::AlreadyCovered;
        }

        queue
            .deferred_saves
            .insert(character_id, DeferredCharacterSave { snapshot, reason });
        info!(
            "Deferring {reason} for persistent character {} at {:?} until its current save finishes",
            character_id.0, comparable_snapshot.position
        );
        return CharacterSaveRequest::Deferred;
    }

    // With no request in flight, this snapshot is the newest authority and
    // supersedes anything retained from a previous failed request.
    queue.deferred_saves.remove(&character_id);

    snapshot.expected_revision = queue
        .revisions
        .get(&character_id)
        .copied()
        .unwrap_or(snapshot.expected_revision);

    if queue.last_saved.get(&character_id) == Some(&comparable_snapshot) {
        return CharacterSaveRequest::AlreadyCovered;
    }

    let request_id = queue.next_request_id();
    match persistence.send(PersistenceRequest::SaveCharacter {
        request_id,
        snapshot,
    }) {
        Ok(()) => {
            info!(
                "Queuing {reason} for persistent character {} at {:?}",
                character_id.0, comparable_snapshot.position
            );
            queue.save_requests.insert(request_id, character_id);
            queue
                .saves_in_flight
                .insert(character_id, comparable_snapshot);
            CharacterSaveRequest::Queued
        }
        Err(error) => {
            error!(
                "Could not queue {reason} for character {}: {}",
                character_id.0, error
            );
            CharacterSaveRequest::Failed
        }
    }
}

fn finish_character_save(
    persistence: Option<&PersistenceClient>,
    queue: &mut CharacterPersistenceQueue,
    request_id: u64,
    character_id: CharacterId,
    revision: u64,
) {
    let requested_character = queue.save_requests.remove(&request_id);
    if requested_character != Some(character_id) {
        warn!(
            "Received save confirmation for unexpected request {} and character {}",
            request_id, character_id.0
        );
    }
    if let Some(saved_snapshot) = queue.saves_in_flight.remove(&character_id) {
        queue.last_saved.insert(character_id, saved_snapshot);
    }
    queue.revisions.insert(character_id, revision);
    info!(
        "Saved persistent character {} at revision {} (request {})",
        character_id.0, revision, request_id
    );

    if let (Some(persistence), Some(deferred)) =
        (persistence, queue.deferred_saves.remove(&character_id))
    {
        request_character_save(persistence, queue, deferred.snapshot, deferred.reason);
    }
}

fn server_events(
    mut server_events: ResMut<PendingServerEvents>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    players: Query<(Entity, &Player, &Transform, Option<&AuthoritativeCast>)>,
    player_persistence_states: Query<
        (
            Entity,
            Option<&Transform>,
            Option<&CharacterId>,
            Option<&PersistentCharacter>,
            Option<&Health>,
            Option<&Mana>,
            Option<&Facing>,
        ),
        With<Player>,
    >,
    monsters: Query<(Entity, &Monster, &Transform), With<Monster>>,
    mut server_visualizer: ResMut<ServerVisualizer>,
    time: Res<Time>,
    map: Res<Map>,
    persistence: Option<Res<PersistenceClient>>,
    persistence_status: Res<PersistenceStatus>,
    mut persistence_inbox: ResMut<PersistenceInbox>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
) {
    let connected_clients = server.clients_id();

    if matches!(*persistence_status, PersistenceStatus::Ready) {
        if let Some(persistence) = persistence.as_deref() {
            let waiting_clients: Vec<_> = persistence_queue.waiting_clients.drain().collect();
            for client_id in waiting_clients {
                if connected_clients.contains(&client_id)
                    && !request_character_load(client_id, persistence, &mut persistence_queue)
                {
                    error!(
                        "Disconnecting client {client_id}: the persistent character load could not \
                         be queued"
                    );
                    server.disconnect(client_id);
                }
            }
        }
    } else if matches!(*persistence_status, PersistenceStatus::Disabled) {
        let waiting_clients: Vec<_> = persistence_queue.waiting_clients.drain().collect();
        for client_id in waiting_clients {
            if connected_clients.contains(&client_id) {
                spawn_and_announce_player(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &mut lobby,
                    &mut server,
                    client_id,
                    PlayerSpawn::ephemeral(),
                    time.elapsed().as_millis(),
                );
            }
        }
    } else if let PersistenceStatus::Failed(failure) = &*persistence_status {
        let waiting_clients: Vec<_> = persistence_queue.waiting_clients.drain().collect();
        for client_id in waiting_clients {
            if connected_clients.contains(&client_id) {
                error!(
                    "Disconnecting client {client_id}: persistent character loading is unavailable: \
                     {failure}"
                );
                server.disconnect(client_id);
            }
        }
    }

    while let Some(response) = persistence_inbox.0.pop_front() {
        match response {
            PersistenceResponse::CharacterLoaded {
                request_id,
                character,
            } => {
                let Some(client_id) = persistence_queue.load_requests.remove(&request_id) else {
                    warn!("Received character load for unknown request {request_id}");
                    continue;
                };
                if !server.clients_id().contains(&client_id) {
                    continue;
                }

                let Some(character) = character else {
                    error!("No character was returned for connected client {client_id}");
                    server.disconnect(client_id);
                    continue;
                };

                let character_id = CharacterId(character.id);
                if player_persistence_states
                    .iter()
                    .any(|(_, _, active_id, _, _, _, _)| {
                        active_id.is_some_and(|active_id| *active_id == character_id)
                    })
                {
                    warn!(
                        "Rejecting duplicate login for persistent character {}",
                        character_id.0
                    );
                    server.disconnect(client_id);
                    continue;
                }

                info!(
                    "Loaded persistent character {} for client {}",
                    character_id.0, client_id
                );
                persistence_queue.last_saved.insert(
                    character_id,
                    CharacterSnapshot::from_record(&character).without_revision(),
                );
                persistence_queue
                    .revisions
                    .insert(character_id, character.revision);
                spawn_and_announce_player(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &mut lobby,
                    &mut server,
                    client_id,
                    PlayerSpawn::from_record(character, &map),
                    time.elapsed().as_millis(),
                );
            }
            PersistenceResponse::CharacterSaved {
                request_id,
                character_id,
                revision,
            } => {
                finish_character_save(
                    persistence.as_deref(),
                    &mut persistence_queue,
                    request_id,
                    character_id,
                    revision,
                );
            }
            PersistenceResponse::RequestFailed {
                request_id,
                operation,
                message,
            } => {
                error!("Persistence failed to {operation}: {message}");
                if let Some(request_id) = request_id {
                    if let Some(character_id) = persistence_queue.save_requests.remove(&request_id)
                    {
                        persistence_queue.saves_in_flight.remove(&character_id);
                    } else if let Some(client_id) =
                        persistence_queue.load_requests.remove(&request_id)
                    {
                        if server.clients_id().contains(&client_id) {
                            error!(
                                "Disconnecting client {client_id}: persistent character loading \
                                 failed"
                            );
                            server.disconnect(client_id);
                        }
                    }
                }
            }
            other => {
                debug!("Ignoring unused persistence response: {other:?}");
            }
        }
    }

    if persistence_queue
        .autosave_timer
        .tick(time.delta())
        .just_finished()
    {
        if let Some(persistence) = persistence.as_deref() {
            for (_entity, transform, character_id, persistent, health, mana, facing) in
                &player_persistence_states
            {
                let (
                    Some(transform),
                    Some(character_id),
                    Some(persistent),
                    Some(health),
                    Some(mana),
                    Some(facing),
                ) = (transform, character_id, persistent, health, mana, facing)
                else {
                    continue;
                };
                let snapshot = persistent.snapshot(*character_id, transform, facing, health, mana);
                request_character_save(persistence, &mut persistence_queue, snapshot, "autosave");
            }
        }
    }

    for event in server_events.0.drain(..) {
        match event {
            PendingServerEvent::Connected(client_id) => {
                println!("Client {client_id} connected");
                server_visualizer.add_client(client_id);
                match &*persistence_status {
                    PersistenceStatus::Ready => {
                        let requested = persistence.as_deref().is_some_and(|persistence| {
                            request_character_load(client_id, persistence, &mut persistence_queue)
                        });
                        if !requested {
                            error!(
                                "Disconnecting client {client_id}: the persistent character load \
                                 could not be queued"
                            );
                            server.disconnect(client_id);
                        }
                    }
                    PersistenceStatus::Connecting => {
                        persistence_queue.waiting_clients.insert(client_id);
                    }
                    PersistenceStatus::Disabled => {
                        spawn_and_announce_player(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            &mut lobby,
                            &mut server,
                            client_id,
                            PlayerSpawn::ephemeral(),
                            time.elapsed().as_millis(),
                        );
                    }
                    PersistenceStatus::Failed(failure) => {
                        error!(
                            "Disconnecting client {client_id}: persistence is configured but \
                             unavailable: {failure}"
                        );
                        server.disconnect(client_id);
                    }
                }
            }
            PendingServerEvent::Disconnected(client_id, reason) => {
                println!("Player {} disconnected: {}", client_id, reason);

                server_visualizer.remove_client(client_id);
                persistence_queue.remove_client(client_id);
                //visualizer.remove_client(*client_id);
                if let Some(player_entity) = lobby.players.remove(&client_id) {
                    let tracked_character_id = lobby.characters.remove(&client_id);
                    let current_state = player_persistence_states.get(player_entity).ok();
                    let component_character_id = current_state
                        .and_then(|(_, _, character_id, _, _, _, _)| character_id.copied());
                    let character_id = component_character_id.or(tracked_character_id);

                    if persistence.is_none() {
                        warn!(
                            "Cannot save disconnected client {client_id}: persistence is unavailable"
                        );
                    } else if character_id.is_none_or(|character_id| character_id.0 == 0) {
                        warn!(
                            "Cannot save disconnected client {client_id}: the player was started as \
                             an in-memory character rather than a database-backed character"
                        );
                    } else {
                        let character_id =
                            character_id.expect("nonzero character id was checked above");
                        let (_, transform, _, persistent, health, mana, facing) = current_state
                            .unwrap_or((player_entity, None, None, None, None, None, None));
                        let snapshot = disconnect_snapshot(
                            character_id,
                            transform,
                            persistent,
                            health,
                            mana,
                            facing,
                            persistence_queue.last_saved.get(&character_id),
                        );

                        match snapshot {
                            Ok((snapshot, missing)) => {
                                if !missing.is_empty() {
                                    warn!(
                                        "Recovered disconnect snapshot for persistent character {} \
                                         despite missing ECS components: {}",
                                        character_id.0,
                                        missing.join(", ")
                                    );
                                }
                                info!(
                                    "Disconnect snapshot for persistent character {} is at {:?}",
                                    character_id.0, snapshot.position
                                );
                                match request_character_save(
                                    persistence
                                        .as_deref()
                                        .expect("persistence availability was checked above"),
                                    &mut persistence_queue,
                                    snapshot,
                                    "disconnect save",
                                ) {
                                    CharacterSaveRequest::Queued => {}
                                    CharacterSaveRequest::Deferred => info!(
                                        "Disconnect save for persistent character {} will run after \
                                         the current database write",
                                        character_id.0
                                    ),
                                    CharacterSaveRequest::AlreadyCovered => info!(
                                        "Disconnect save for persistent character {} needs no new \
                                         database write because this exact snapshot is already saved \
                                         or in flight",
                                        character_id.0
                                    ),
                                    CharacterSaveRequest::Failed => {}
                                }
                            }
                            Err(missing) => warn!(
                                "Cannot save disconnected client {client_id}: player entity \
                                 {player_entity:?} is missing {} and no cached database snapshot is \
                                 available",
                                missing.join(", ")
                            ),
                        }
                    }

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
                PlayerCommand::Cast { spell_id, cast_at } => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands.trigger(RequestSpellCast {
                            caster: *player_entity,
                            spell_id,
                            target: cast_at,
                        });
                    }
                }
                PlayerCommand::BasicAttack { entity } => {
                    println!(
                        "Received basic attack from client {}: {:?}",
                        client_id, entity
                    );

                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        if let (
                            Ok((_entity, _player, _player_transform, active_cast)),
                            Ok((monster_entity, _monster, monster_transform)),
                        ) = (players.get(*player_entity), monsters.get(entity))
                        {
                            if active_cast.is_some() {
                                info!("Ignoring attack from client {client_id} while casting");
                                continue;
                            }
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
                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        if let Ok((_entity, _player, player_transform, active_cast)) =
                            players.get(*player_entity)
                        {
                            if active_cast.is_some() {
                                info!("Ignoring movement from client {client_id} while casting");
                                continue;
                            }
                            let path = get_path_between_translations(
                                player_transform.translation,
                                destination_at,
                                &map,
                            );
                            let mut player_commands = commands.entity(*player_entity);
                            player_commands
                                .remove::<Aggro>()
                                .remove::<Attacking>()
                                .remove::<AttackingTimer>();

                            match path {
                                Some(path) => {
                                    info!(
                                        "Accepted move from client {}: {:?} -> {:?} ({} cells)",
                                        client_id,
                                        player_transform.translation,
                                        destination_at,
                                        path.0.len()
                                    );
                                    player_commands.insert(Walking {
                                        target_translation: destination_at,
                                        path: Some(path),
                                    });
                                }
                                None => {
                                    warn!(
                                        "Rejected move from client {}: {:?} -> {:?}",
                                        client_id, player_transform.translation, destination_at
                                    );
                                    player_commands.remove::<Walking>();
                                    let message =
                                        bincode::serialize(&ServerMessages::MovementRejected {
                                            entity: *player_entity,
                                            translation: player_transform.translation.into(),
                                            server_time: time.elapsed().as_millis(),
                                        })
                                        .expect("movement rejection should serialize");
                                    server.send_message(
                                        client_id,
                                        ServerChannel::ServerMessages,
                                        message,
                                    );
                                }
                            }
                        }
                    }
                }
                PlayerCommand::StopMoving => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands
                            .entity(*player_entity)
                            .remove::<Walking>()
                            .remove::<Aggro>()
                            .remove::<Attacking>()
                            .remove::<AttackingTimer>();
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
    players: Query<
        (
            Entity,
            &Player,
            &Transform,
            &AttackSpeed,
            &CharacterId,
            &Facing,
            &Health,
            &Mana,
        ),
        With<Player>,
    >,
    treeaccess: Res<NNTree>,
    entities: Query<(Entity, &Transform, &SpriteId, &Facing, Option<&Health>)>,
    time: Res<Time>,
) {
    for (viewer_entity, player, transform, mut line_of_sight) in viewers.iter_mut() {
        let within_distance = treeaccess.within_distance(transform.translation, LINE_OF_SIGHT);

        let mut new_set: HashSet<Entity> =
            within_distance.iter().filter_map(|entry| entry.1).collect();

        // Player replication must not depend solely on the periodically rebuilt
        // spatial tree. A player can connect between tree rebuilds, and every
        // nearby client still needs an immediate, symmetric PlayerCreate.
        include_nearby_players(
            &mut new_set,
            transform.translation,
            players
                .iter()
                .map(|(entity, _, transform, _, _, _, _, _)| (entity, transform.translation)),
        );

        let old_set: HashSet<Entity> = line_of_sight.0.iter().cloned().collect();
        if old_set == new_set {
            continue;
        }

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

            if let Ok((
                _,
                spawned_player,
                spawned_transform,
                attack_speed,
                character_id,
                facing,
                health,
                mana,
            )) = players.get(*spawned_entity)
            {
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: spawned_player.id,
                    entity: *spawned_entity,
                    character_id: *character_id,
                    translation: spawned_transform.translation.into(),
                    facing: facing.clone(),
                    health: health.clone(),
                    mana: mana.clone(),
                    attack_speed: attack_speed.0,
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

        line_of_sight.0 = new_set.into_iter().collect();
        line_of_sight.0.sort_unstable();
    }
}

fn include_nearby_players(
    visible: &mut HashSet<Entity>,
    viewer_translation: Vec3,
    players: impl IntoIterator<Item = (Entity, Vec3)>,
) {
    for (entity, translation) in players {
        if viewer_translation.distance_squared(translation) <= LINE_OF_SIGHT * LINE_OF_SIGHT {
            visible.insert(entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_blocked_spawns_are_relocated_even_before_navigation_is_ready() {
        let map = Map::default();

        assert_eq!(
            resolve_persistent_spawn(Vec3::new(0.0, 1.0, 0.0), &map),
            default_character_spawn()
        );
        assert_eq!(
            resolve_persistent_spawn(Vec3::new(-12.0, 1.0, 16.0), &map),
            default_character_spawn()
        );
    }

    #[test]
    fn blocked_persistent_spawn_is_relocated_but_open_spawn_is_preserved() {
        let open_spawn = Vec3::new(7.25, 2.0, -3.75);
        let blocked_spawn = Vec3::new(8.0, 2.0, -4.0);
        let mut map = Map::default();
        map.blocked_paths.insert(world_cell(blocked_spawn));

        assert_eq!(
            resolve_persistent_spawn(blocked_spawn, &map),
            default_character_spawn()
        );
        assert_eq!(resolve_persistent_spawn(open_spawn, &map), open_spawn);
    }

    fn snapshot_at(character_id: CharacterId, position: [f32; 3]) -> CharacterSnapshot {
        CharacterSnapshot {
            character_id,
            base_level: 1,
            base_experience: 0,
            job_level: 1,
            job_experience: 0,
            hp: 40,
            max_hp: 40,
            sp: 10,
            max_sp: 10,
            zeny: 0,
            map_name: "starting_map".into(),
            position,
            facing: 0,
            expected_revision: 0,
        }
    }

    #[test]
    fn disconnect_snapshot_uses_current_position_when_an_ecs_component_is_missing() {
        let character_id = CharacterId(1);
        let cached = snapshot_at(character_id, [1.0, 1.0, 2.0]);
        let transform = Transform::from_xyz(8.0, 3.0, -4.0);
        let health = Health {
            current: 31,
            max: 40,
        };
        let facing = Facing(6);

        let (snapshot, missing) = disconnect_snapshot(
            character_id,
            Some(&transform),
            None,
            Some(&health),
            None,
            Some(&facing),
            Some(&cached),
        )
        .expect("the cached database state should complete the snapshot");

        assert_eq!(snapshot.position, [8.0, 3.0, -4.0]);
        assert_eq!(snapshot.hp, 31);
        assert_eq!(snapshot.sp, cached.sp);
        assert_eq!(snapshot.facing, 6);
        assert_eq!(missing, vec!["PersistentCharacter", "Mana"]);
    }

    #[test]
    fn disconnect_event_saves_current_position_with_partial_persistent_components() {
        let client_id = 1;
        let character_id = CharacterId(1);
        let current_position = Vec3::new(9.0, 2.0, -6.0);
        let (persistence, mut requests) = PersistenceClient::test_channel();

        let mut world = World::new();
        let player_entity = world
            .spawn((
                Player { id: client_id },
                Transform::from_translation(current_position),
                character_id,
                Health {
                    current: 35,
                    max: 40,
                },
                Mana {
                    current: 8,
                    max: 10,
                },
                Facing(6),
            ))
            .id();

        let mut lobby = ServerLobby::default();
        lobby.players.insert(client_id, player_entity);
        lobby.characters.insert(client_id, character_id);
        world.insert_resource(lobby);
        world.insert_resource(PendingServerEvents(vec![PendingServerEvent::Disconnected(
            client_id,
            "test disconnect".into(),
        )]));
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<StandardMaterial>::default());
        world.insert_resource(RenetServer::new(connection_config()));
        world.insert_resource(ServerVisualizer(RenetServerVisualizer::<200>::new(
            RenetVisualizerStyle::default(),
        )));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(Map::default());
        world.insert_resource(persistence);
        world.insert_resource(PersistenceStatus::Ready);
        world.insert_resource(PersistenceInbox::default());

        let mut queue = CharacterPersistenceQueue::default();
        queue
            .last_saved
            .insert(character_id, snapshot_at(character_id, [0.0, 1.0, 0.0]));
        queue.revisions.insert(character_id, 2);
        world.insert_resource(queue);

        let mut schedule = Schedule::default();
        schedule.add_systems(server_events);
        schedule.run(&mut world);

        let PersistenceRequest::SaveCharacter { snapshot, .. } = requests
            .try_recv()
            .expect("disconnect should queue a character save")
        else {
            panic!("expected a character save request");
        };
        assert_eq!(snapshot.position, current_position.to_array());
        assert_eq!(snapshot.expected_revision, 2);
    }

    #[test]
    fn configured_persistence_failure_does_not_spawn_an_unsavable_player() {
        let client_id = 1;
        let mut world = World::new();
        world.insert_resource(PendingServerEvents(vec![PendingServerEvent::Connected(
            client_id,
        )]));
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<StandardMaterial>::default());
        world.insert_resource(ServerLobby::default());
        world.insert_resource(RenetServer::new(connection_config()));
        world.insert_resource(ServerVisualizer(RenetServerVisualizer::<200>::new(
            RenetVisualizerStyle::default(),
        )));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(Map::default());
        world.insert_resource(PersistenceStatus::Failed("database is unavailable".into()));
        world.insert_resource(PersistenceInbox::default());
        world.insert_resource(CharacterPersistenceQueue::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(server_events);
        schedule.run(&mut world);

        assert!(world.resource::<ServerLobby>().players.is_empty());
        assert_eq!(
            world
                .query_filtered::<Entity, With<Player>>()
                .iter(&world)
                .count(),
            0
        );
    }

    #[test]
    fn newer_disconnect_snapshot_waits_for_in_flight_autosave() {
        let character_id = CharacterId(1);
        let (persistence, mut requests) = PersistenceClient::test_channel();
        let mut queue = CharacterPersistenceQueue::default();
        queue.revisions.insert(character_id, 4);

        assert_eq!(
            request_character_save(
                &persistence,
                &mut queue,
                snapshot_at(character_id, [0.0, 1.0, 0.0]),
                "autosave",
            ),
            CharacterSaveRequest::Queued
        );
        let PersistenceRequest::SaveCharacter {
            request_id,
            snapshot,
        } = requests.try_recv().expect("first save should be queued")
        else {
            panic!("expected a character save request");
        };
        assert_eq!(snapshot.expected_revision, 4);
        assert!(!queue.last_saved.contains_key(&character_id));

        assert_eq!(
            request_character_save(
                &persistence,
                &mut queue,
                snapshot_at(character_id, [8.0, 1.0, -3.0]),
                "disconnect save",
            ),
            CharacterSaveRequest::Deferred
        );
        assert_eq!(
            queue
                .deferred_saves
                .get(&character_id)
                .map(|save| save.snapshot.position),
            Some([8.0, 1.0, -3.0])
        );

        finish_character_save(Some(&persistence), &mut queue, request_id, character_id, 5);

        let PersistenceRequest::SaveCharacter { snapshot, .. } =
            requests.try_recv().expect("deferred save should be queued")
        else {
            panic!("expected the deferred character save request");
        };
        assert_eq!(snapshot.position, [8.0, 1.0, -3.0]);
        assert_eq!(snapshot.expected_revision, 5);
        assert!(!queue.deferred_saves.contains_key(&character_id));
        assert_eq!(
            queue
                .last_saved
                .get(&character_id)
                .map(|snapshot| snapshot.position),
            Some([0.0, 1.0, 0.0])
        );
    }

    #[test]
    fn nearby_players_are_visible_without_waiting_for_the_spatial_tree() {
        let mut world = World::new();
        let nearby = world.spawn_empty().id();
        let far_away = world.spawn_empty().id();
        let mut visible = HashSet::new();

        include_nearby_players(
            &mut visible,
            Vec3::ZERO,
            [
                (nearby, Vec3::new(LINE_OF_SIGHT - 1.0, 0.0, 0.0)),
                (far_away, Vec3::new(LINE_OF_SIGHT + 1.0, 0.0, 0.0)),
            ],
        );

        assert!(visible.contains(&nearby));
        assert!(!visible.contains(&far_away));
    }

    #[test]
    fn server_events_system_has_valid_bevy_parameters() {
        let mut world = World::new();
        world.insert_resource(PendingServerEvents::default());
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<StandardMaterial>::default());
        world.insert_resource(ServerLobby::default());
        world.insert_resource(RenetServer::new(connection_config()));
        world.insert_resource(ServerVisualizer(RenetServerVisualizer::<200>::new(
            RenetVisualizerStyle::default(),
        )));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(Map::default());
        world.insert_resource(PersistenceStatus::Disabled);
        world.insert_resource(PersistenceInbox::default());
        world.insert_resource(CharacterPersistenceQueue::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(server_events);
        schedule.run(&mut world);
    }

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
