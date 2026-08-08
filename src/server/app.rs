// use avian3d::prelude::{Collider, GravityScale, LockedAxes, RigidBody};
use bevy::ecs::system::SystemParam;
use bevy::log::LogPlugin;

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
use crate::server::gameplay::equipment::{RequestEquipItem, RequestUnequipItem};
use crate::server::gameplay::items::{PendingItemPickup, RequestItemPickup, RequestItemUse};
use crate::server::gameplay::monsters::*;
use crate::server::gameplay::pathing::*;
use crate::server::gameplay::regeneration::{PassiveRegeneration, RegenerationPlugin};
use crate::server::gameplay::spawn_protection::{SpawnProtection, SpawnProtectionPlugin};
use crate::server::state::*;
use bevy_renet::{RenetServer, RenetServerEvent, RenetServerPlugin};

use crate::server::gameplay::spatial::{
    AutomaticUpdate, NNTree, NearestNeighbourComponent, SpatialAccess,
};
use crate::server::gameplay::spells::{AuthoritativeCast, RequestSpellCast};
use crate::server::network::replication::{should_receive_player_action, LineOfSight, PrevState};
use crate::server::persistence::{
    AccountId, CharacterRecord, CharacterSnapshot, PersistenceClient, PersistenceInbox,
    PersistenceRequest, PersistenceResponse, PersistenceStatus, PersistentCharacter,
};
use crate::shared::constants::*;
use crate::shared::gameplay::action_bar::{
    ActionBarBinding, ActionBarLayout, ACTION_BAR_SLOT_COUNT,
};
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::entities::{AttackSpeed, Player};
use crate::shared::gameplay::items::{
    equipment_derived_stats, item_definition, GroundItem, Inventory,
};
use crate::shared::gameplay::maps::{
    canonical_map_name, map_spawn_position, map_to_local_position, map_to_server_position,
    next_map_name, CurrentMap, STARTING_MAP_NAME,
};
use crate::shared::gameplay::progression::{BaseProgression, JobProgression};
use crate::shared::gameplay::skills::{LearnedSkill, SkillTree};
use crate::shared::gameplay::spells::spell_definition;
use crate::shared::network::config::{
    private_key_from_env, socket_addr_from_env, usize_from_env, DEFAULT_GAME_SERVER_ADDR,
    DEFAULT_MAX_CLIENTS, DEFAULT_SERVER_BIND_ADDR, DEFAULT_TOKEN_BIND_ADDR,
};
use crate::shared::network::{channels::*, messages::*};
use crate::shared::states::ServerState;
use crate::world::setup_server_level;
use bevy::{
    app::ScheduleRunnerPlugin,
    asset::AssetPlugin,
    gltf::GltfPlugin,
    image::{CompressedImageFormatSupport, CompressedImageFormats, ImagePlugin},
    mesh::MeshPlugin,
    scene::ScenePlugin,
    state::app::StatesPlugin,
    transform::TransformPlugin,
    world_serialization::WorldSerializationPlugin,
};
use bevy_rapier3d::prelude::*;
use std::collections::HashSet;
use std::ops::Div;
use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
};

#[derive(SystemParam)]
struct ServerMovementParams<'w, 's> {
    time: Res<'w, Time>,
    map: Res<'w, Map>,
    damage_walk_delays: Query<'w, 's, &'static mut DamageWalkDelay>,
    player_map_states: Query<'w, 's, (&'static CurrentMap, Option<&'static PersistentCharacter>)>,
}

#[derive(SystemParam)]
struct ServerAnimationReplicationParams<'w, 's> {
    sitting_players: Query<'w, 's, (), With<Sitting>>,
    viewers: Query<'w, 's, (Entity, &'static Player, &'static LineOfSight)>,
}

pub fn run() {
    let _ = dotenvy::dotenv();
    let asset_root = server_asset_root();
    let network = ServerNetworkConfig::from_env()
        .unwrap_or_else(|error| panic!("Invalid server network configuration: {error}"));
    if let Some(private_key) = network.private_key {
        crate::server::network::token_service::start(
            network.token_bind_addr,
            network.public_addr,
            private_key,
        )
        .unwrap_or_else(|error| panic!("Could not start the network token service: {error}"));
    }
    let transport = create_renet_transport(&network);

    let mut app = App::new();

    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        ))),
    )
    .add_plugins(LogPlugin {
        filter: "info,rechannel=warn".into(),
        level: bevy::log::Level::INFO,
        ..Default::default()
    })
    .add_plugins((
        TransformPlugin,
        AssetPlugin {
            file_path: asset_root,
            ..default()
        },
        WorldSerializationPlugin,
        ScenePlugin,
        ImagePlugin::default(),
        MeshPlugin,
        GltfPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(CompressedImageFormatSupport(CompressedImageFormats::NONE))
    .init_state::<ServerState>()
    .add_plugins(PathingPlugin)
    .add_plugins(AutomaticUpdate::<NearestNeighbourComponent>::new())
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
    .add_systems(Startup, setup_server_level)
    .add_plugins((
        // crate::server::gameplay::physics::ServerPhysicsPlugin,
        MonstersPlugin,
        crate::server::gameplay::spells::SpellsPlugin,
        crate::server::persistence::PersistencePlugin,
        crate::server::network::clock_sync::ServerClockSyncPlugin,
        // crate::server::network::clock_server::ClockServerPlugin, // prototype
        crate::server::gameplay::combat::CombatPlugin,
        crate::server::gameplay::equipment::EquipmentPlugin,
        crate::server::gameplay::items::ItemsPlugin,
        RegenerationPlugin,
        SpawnProtectionPlugin,
    ))
    .add_plugins(RenetServerPlugin)
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
    .insert_resource(transport)
    //.add_systems(FixedUpdate, sync_client_time)
    .add_systems(
        Update,
        (
            server_events,
            send_changed_skill_trees.after(server_events),
            send_changed_character_stats.after(server_events),
            save_changed_progression.after(server_events),
            // update_projectiles_system,
            // update_visualizer_system
        ),
    )
    .add_observer(queue_server_event)
    .add_observer(on_cycle_placeholder_class)
    .add_observer(on_spend_attribute_point)
    .add_observer(on_respawn_at_save_point)
    .add_systems(FixedUpdate, line_of_sight.after(PhysicsSet::Writeback))
    .insert_resource(TimestepMode::Fixed {
        dt: 1.0 / 60.0, // 60 FPS physics update
        substeps: 1,
    });

    app.add_systems(FixedPostUpdate, network_send_position_snapshots);

    app.run();
}

fn server_asset_root() -> String {
    if let Ok(configured) = std::env::var("ASSET_ROOT") {
        if !configured.trim().is_empty() {
            return configured;
        }
    }

    if let Ok(current_directory) = std::env::current_dir() {
        let local_assets = current_directory.join("assets");
        if local_assets.is_dir() {
            return local_assets.to_string_lossy().into_owned();
        }
    }

    "assets".to_string()
}

#[derive(Event)]
struct CyclePlaceholderClass {
    player: Entity,
}

#[derive(Event)]
struct RespawnAtSavePoint {
    player: Entity,
}

fn respawn_destination(save_point: Option<&SavePoint>) -> (&'static str, Vec3) {
    if let Some(save_point) = save_point {
        let local_position = Vec3::from(save_point.position);
        if local_position.is_finite() {
            let map_name = canonical_map_name(&save_point.map_name);
            return (map_name, map_to_server_position(map_name, local_position));
        }
    }
    (STARTING_MAP_NAME, default_character_spawn())
}

fn on_respawn_at_save_point(
    trigger: On<RespawnAtSavePoint>,
    mut players: Query<
        (
            &LineOfSight,
            &mut Transform,
            &mut Health,
            &mut Mana,
            &mut GameVelocity,
            &mut CurrentMap,
            Option<&SavePoint>,
            Option<&mut PersistentCharacter>,
            Option<&mut KinematicCharacterController>,
        ),
        (With<Player>, With<Dead>),
    >,
    viewers: Query<(Entity, &Player, &LineOfSight)>,
    mut server: ResMut<RenetServer>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let player_entity = trigger.event().player;
    let Ok((
        player_line_of_sight,
        mut transform,
        mut health,
        mut mana,
        mut velocity,
        mut current_map,
        save_point,
        persistent,
        controller,
    )) = players.get_mut(player_entity)
    else {
        return;
    };

    let (map_name, destination) = respawn_destination(save_point);
    current_map.0 = map_name.to_string();
    if let Some(mut persistent) = persistent {
        persistent.map_name = map_name.to_string();
    }
    transform.translation = destination;
    health.current = health.max;
    mana.current = mana.max;
    velocity.0 = Vec3::ZERO;
    if let Some(mut controller) = controller {
        controller.translation = None;
    }

    commands
        .entity(player_entity)
        .try_remove::<Dead>()
        .try_remove::<Sitting>()
        .try_remove::<Aggro>()
        .try_remove::<Attacking>()
        .try_remove::<AttackingTimer>()
        .try_remove::<Walking>()
        .try_remove::<TargetPos>()
        .try_insert(SpawnProtection::default());

    let message = bincode::serialize(&ServerMessages::PlayerRespawned {
        entity: player_entity,
        map_name: map_name.to_string(),
        translation: destination.into(),
        health: health.clone(),
        mana: mana.clone(),
        server_time: time.elapsed().as_millis(),
    })
    .expect("player respawn message should serialize");
    for (viewer_entity, viewer, line_of_sight) in &viewers {
        if should_receive_player_action(
            viewer_entity,
            player_entity,
            line_of_sight,
            player_line_of_sight,
        ) {
            server.send_message(viewer.id, ServerChannel::ServerMessages, message.clone());
        }
    }
}

fn on_cycle_placeholder_class(
    trigger: On<CyclePlaceholderClass>,
    mut players: Query<
        (
            &mut JobProgression,
            &mut SkillTree,
            Option<&mut ActionBarLayout>,
        ),
        With<Player>,
    >,
) {
    let request = trigger.event();
    let Ok((mut progression, mut skill_tree, action_bar)) = players.get_mut(request.player) else {
        return;
    };
    let previous = progression.class;
    let next = previous.next_placeholder();
    progression.change_class(next);
    skill_tree.reset();
    if let Some(mut action_bar) = action_bar {
        for slot_index in 0..ACTION_BAR_SLOT_COUNT {
            if matches!(
                action_bar.binding(slot_index),
                Some(ActionBarBinding::Skill(_))
            ) {
                action_bar.set(slot_index, None);
            }
        }
    }
    info!(
        "Player {:?} changed placeholder class {} -> {} and reset to Job Lv. 1",
        request.player,
        previous.name(),
        next.name()
    );
}

#[derive(Event)]
struct SpendAttributePoint {
    player: Entity,
    attribute: CharacterAttribute,
}

fn on_spend_attribute_point(
    trigger: On<SpendAttributePoint>,
    mut players: Query<
        (
            &Player,
            &mut CharacterStats,
            &BaseProgression,
            &Equipment,
            &mut Health,
            &mut Mana,
        ),
        With<Player>,
    >,
) {
    let request = trigger.event();
    let Ok((player, mut stats, progression, equipment, mut health, mut mana)) =
        players.get_mut(request.player)
    else {
        return;
    };
    match stats.spend_point(request.attribute) {
        Ok(value) => {
            let derived = equipment_derived_stats(&stats, progression.level, equipment);
            health.max = derived.max_health;
            health.current = health.current.min(health.max);
            mana.max = derived.max_mana;
            mana.current = mana.current.min(mana.max);
            info!(
                "Player {} raised {} to {}",
                player.id,
                request.attribute.name(),
                value
            );
        }
        Err(reason) => warn!(
            "Rejected {} allocation from client {}: {:?}",
            request.attribute.name(),
            player.id,
            reason
        ),
    }
}

fn create_renet_server() -> RenetServer {
    RenetServer::new(connection_config())
}

#[derive(Clone, Copy, Debug)]
struct ServerNetworkConfig {
    bind_addr: SocketAddr,
    public_addr: SocketAddr,
    token_bind_addr: SocketAddr,
    max_clients: usize,
    private_key: Option<[u8; 32]>,
}

impl ServerNetworkConfig {
    fn from_env() -> Result<Self, String> {
        let bind_addr = socket_addr_from_env("SERVER_BIND_ADDR", DEFAULT_SERVER_BIND_ADDR)?;
        let private_key = private_key_from_env("NETCODE_PRIVATE_KEY")?;
        if private_key.is_none() && !bind_addr.ip().is_loopback() {
            return Err(
                "unsecure mode may bind only to loopback; set NETCODE_PRIVATE_KEY for a public server"
                    .to_string(),
            );
        }

        Ok(Self {
            bind_addr,
            public_addr: socket_addr_from_env("SERVER_PUBLIC_ADDR", DEFAULT_GAME_SERVER_ADDR)?,
            token_bind_addr: socket_addr_from_env("TOKEN_BIND_ADDR", DEFAULT_TOKEN_BIND_ADDR)?,
            max_clients: usize_from_env("SERVER_MAX_CLIENTS", DEFAULT_MAX_CLIENTS)?,
            private_key,
        })
    }
}

fn create_renet_transport(network: &ServerNetworkConfig) -> NetcodeServerTransport {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch");

    let authentication = match network.private_key {
        Some(private_key) => {
            info!(
                "Starting secure server on {} (public address {}, max {} clients)",
                network.bind_addr, network.public_addr, network.max_clients
            );
            ServerAuthentication::Secure { private_key }
        }
        None => {
            warn!(
                "Starting loopback-only unsecure development server on {}",
                network.bind_addr
            );
            ServerAuthentication::Unsecure
        }
    };

    let server_config: ServerConfig = ServerConfig {
        current_time,
        max_clients: network.max_clients,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![network.public_addr],
        authentication,
    };

    let socket = UdpSocket::bind(network.bind_addr).unwrap_or_else(|error| {
        panic!(
            "Could not bind game UDP socket on {}: {error}",
            network.bind_addr
        )
    });

    NetcodeServerTransport::new(server_config, socket)
        .expect("valid server transport configuration")
}

struct PlayerSpawn {
    character_id: CharacterId,
    map_name: String,
    transform: Transform,
    facing: Facing,
    health: Health,
    mana: Mana,
    gold: Gold,
    stats: CharacterStats,
    equipment: Equipment,
    save_point: Option<SavePoint>,
    progression: BaseProgression,
    job_progression: JobProgression,
    skill_tree: SkillTree,
    inventory: Inventory,
    action_bar: ActionBarLayout,
    persistent: Option<PersistentCharacter>,
}

impl PlayerSpawn {
    fn ephemeral() -> Self {
        Self {
            // Zero is reserved for an in-memory character when persistence is
            // disabled or unavailable.
            character_id: CharacterId(0),
            map_name: STARTING_MAP_NAME.to_string(),
            transform: Transform::from_translation(
                map_spawn_position(STARTING_MAP_NAME)
                    + Vec3::new(
                        (fastrand::f32() - 0.5) * 40.0,
                        0.0,
                        (fastrand::f32() - 0.5) * 40.0,
                    ),
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
            gold: Gold::default(),
            stats: CharacterStats::default(),
            equipment: Equipment::default(),
            save_point: None,
            progression: BaseProgression::default(),
            job_progression: JobProgression::default(),
            skill_tree: SkillTree::default(),
            inventory: Inventory::default(),
            action_bar: ActionBarLayout::default(),
            persistent: None,
        }
    }

    fn from_record(
        record: CharacterRecord,
        map: &Map,
        inventory: Inventory,
        equipment: Equipment,
        action_bar: ActionBarLayout,
        learned_skills: Vec<LearnedSkill>,
    ) -> Self {
        let map_name = canonical_map_name(&record.map_name).to_string();
        let mut persistent = PersistentCharacter::from_record(&record);
        persistent.map_name.clone_from(&map_name);
        let saved_translation = Vec3::new(record.position_x, record.position_y, record.position_z);
        let spawn_translation = resolve_persistent_spawn(&map_name, saved_translation, map);
        let saved_server_translation = map_to_server_position(&map_name, saved_translation);
        if spawn_translation != saved_server_translation {
            warn!(
                "Persistent character {} was saved on blocked tile {:?}; relocating it to {:?}",
                record.id,
                world_cell(saved_server_translation),
                spawn_translation
            );
        }
        let job_progression = JobProgression::from_persisted(
            record.class_id,
            record.job_level,
            record.job_experience,
        );
        let skill_tree =
            SkillTree::from_persisted(job_progression.class, job_progression.level, learned_skills);
        let stats = CharacterStats {
            might: record.might,
            finesse: record.finesse,
            agility: record.agility,
            vitality: record.vitality,
            intellect: record.intellect,
            spirit: record.spirit,
            available_points: record.attribute_points,
        };
        let progression = BaseProgression {
            level: record.base_level,
            experience: record.base_experience,
        };
        let derived = equipment_derived_stats(&stats, progression.level, &equipment);
        let save_point = match (
            record.save_map_name.clone(),
            record.save_position_x,
            record.save_position_y,
            record.save_position_z,
        ) {
            (Some(map_name), Some(x), Some(y), Some(z)) => Some(SavePoint {
                map_name,
                position: [x, y, z],
            }),
            _ => None,
        };
        Self {
            character_id: CharacterId(record.id),
            map_name,
            transform: Transform::from_translation(spawn_translation),
            facing: Facing(record.facing),
            health: Health {
                current: record.hp.min(derived.max_health),
                max: derived.max_health,
            },
            mana: Mana {
                current: record.sp.min(derived.max_mana),
                max: derived.max_mana,
            },
            gold: Gold(record.gold),
            stats,
            equipment,
            save_point,
            progression,
            job_progression,
            skill_tree,
            inventory,
            action_bar,
            persistent: Some(persistent),
        }
    }
}

fn world_cell(translation: Vec3) -> Pos {
    Pos(translation.x.round() as i32, translation.z.round() as i32)
}

fn default_character_spawn() -> Vec3 {
    map_spawn_position(STARTING_MAP_NAME)
}

fn resolve_persistent_spawn(map_name: &str, saved_translation: Vec3, map: &Map) -> Vec3 {
    let saved_server_translation = map_to_server_position(map_name, saved_translation);
    let saved_cell = world_cell(saved_server_translation);

    // The original database default was the map origin, which is inside the
    // fixed wall. Check it explicitly as well as the navigation mask so an old
    // record is safe even if it loads before all navigation cells are built.
    if (map_name == STARTING_MAP_NAME && matches!(saved_cell, Pos(0, 0) | Pos(-12, 16)))
        || map.blocked_paths.contains(&saved_cell)
    {
        map_spawn_position(map_name)
    } else {
        saved_server_translation
    }
}

fn spawn_player(commands: &mut Commands, client_id: ClientId, spawn: PlayerSpawn) -> Entity {
    let attack_speed = 0.5;
    let map_name = spawn.map_name.clone();
    let transform = spawn.transform;
    let facing = spawn.facing.clone();
    let is_dead = spawn.health.current == 0;
    let mut player = commands.spawn((
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
        PassiveRegeneration::default(),
    ));
    player.insert((
        GameVelocity::default(),
        facing.clone(),
        spawn.health,
        spawn.mana,
        spawn.progression,
        spawn.job_progression,
        spawn.skill_tree,
        spawn.inventory,
        spawn.action_bar,
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
    player.insert(CurrentMap(map_name));
    player.insert(spawn.gold);
    player.insert((spawn.stats, spawn.equipment));
    if is_dead {
        player.insert(Dead);
    } else {
        player.insert(SpawnProtection::default());
    }
    if let Some(save_point) = spawn.save_point {
        player.insert(save_point);
    }

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
    map_name: &str,
    facing: &Facing,
    health: &Health,
    mana: &Mana,
    progression: BaseProgression,
    job_progression: JobProgression,
    attack_speed: f32,
    server_time: u128,
) {
    let message = bincode::serialize(&ServerMessages::PlayerCreate {
        id: client_id,
        entity: player_entity,
        character_id,
        map_name: map_name.to_string(),
        translation: transform.translation.into(),
        facing: facing.clone(),
        health: health.clone(),
        mana: mana.clone(),
        progression,
        job_progression,
        attack_speed,
        sitting: false,
        server_time,
    })
    .expect("player create message should serialize");
    server.send_message(recipient, ServerChannel::ServerMessages, message);
}

fn spawn_and_announce_player(
    commands: &mut Commands,
    lobby: &mut ServerLobby,
    server: &mut RenetServer,
    client_id: ClientId,
    spawn: PlayerSpawn,
    server_time: u128,
) -> Entity {
    let character_id = spawn.character_id;
    let map_name = spawn.map_name.clone();
    let transform = spawn.transform;
    let facing = spawn.facing.clone();
    let health = spawn.health.clone();
    let mana = spawn.mana.clone();
    let progression = spawn.progression;
    let job_progression = spawn.job_progression;
    let skill_tree = spawn.skill_tree.clone();
    let inventory = spawn.inventory.clone();
    let equipment = spawn.equipment.clone();
    let action_bar = spawn.action_bar.clone();
    let stats = spawn.stats;
    let player_entity = spawn_player(commands, client_id, spawn);

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
        &map_name,
        &facing,
        &health,
        &mana,
        progression,
        job_progression,
        0.5,
        server_time,
    );
    let inventory_message = bincode::serialize(&ServerMessages::InventoryUpdated {
        entity: player_entity,
        inventory,
    })
    .expect("inventory update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, inventory_message);
    let equipment_message = bincode::serialize(&ServerMessages::EquipmentUpdated {
        entity: player_entity,
        equipment,
    })
    .expect("equipment update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, equipment_message);
    let action_bar_message = bincode::serialize(&ServerMessages::ActionBarUpdated { action_bar })
        .expect("action bar update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, action_bar_message);
    let skill_tree_message = bincode::serialize(&ServerMessages::SkillTreeUpdated { skill_tree })
        .expect("skill tree update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, skill_tree_message);
    let stats_message = bincode::serialize(&ServerMessages::CharacterStatsUpdated {
        entity: player_entity,
        stats,
    })
    .expect("character stats update should serialize");
    server.send_message(client_id, ServerChannel::ServerMessages, stats_message);
    player_entity
}

fn send_changed_skill_trees(
    mut server: ResMut<RenetServer>,
    persistence: Option<Res<PersistenceClient>>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
    players: Query<(&Player, &CharacterId, &SkillTree), Changed<SkillTree>>,
) {
    for (player, character_id, skill_tree) in &players {
        let message = bincode::serialize(&ServerMessages::SkillTreeUpdated {
            skill_tree: skill_tree.clone(),
        })
        .expect("skill tree update should serialize");
        server.send_message(player.id, ServerChannel::ServerMessages, message);

        if character_id.0 != 0 {
            if let Some(persistence) = persistence.as_deref() {
                let request_id = persistence_queue.next_request_id();
                if let Err(error) = persistence.send(PersistenceRequest::SaveSkillPoints {
                    request_id,
                    character_id: *character_id,
                    available_points: skill_tree.available_points(),
                }) {
                    error!(
                        "Could not persist skill-point balance for character {}: {}",
                        character_id.0, error
                    );
                }
            }
        }
    }
}

fn send_changed_character_stats(
    mut server: ResMut<RenetServer>,
    players: Query<(Entity, &Player, &CharacterStats), Changed<CharacterStats>>,
) {
    for (entity, player, stats) in &players {
        let message = bincode::serialize(&ServerMessages::CharacterStatsUpdated {
            entity,
            stats: *stats,
        })
        .expect("character stats update should serialize");
        server.send_message(player.id, ServerChannel::ServerMessages, message);
    }
}

fn disconnect_snapshot(
    character_id: CharacterId,
    transform: Option<&Transform>,
    persistent: Option<&PersistentCharacter>,
    health: Option<&Health>,
    mana: Option<&Mana>,
    gold: Option<&Gold>,
    stats: Option<&CharacterStats>,
    equipment: Option<&Equipment>,
    save_point: Option<&SavePoint>,
    facing: Option<&Facing>,
    progression: Option<&BaseProgression>,
    job_progression: Option<&JobProgression>,
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
    if gold.is_none() {
        missing.push("Gold");
    }
    if stats.is_none() {
        missing.push("CharacterStats");
    }
    if equipment.is_none() {
        missing.push("Equipment");
    }
    if facing.is_none() {
        missing.push("Facing");
    }
    if progression.is_none() {
        missing.push("BaseProgression");
    }
    if job_progression.is_none() {
        missing.push("JobProgression");
    }

    if let (
        Some(transform),
        Some(persistent),
        Some(health),
        Some(mana),
        Some(gold),
        Some(stats),
        Some(equipment),
        Some(facing),
        Some(progression),
        Some(job_progression),
    ) = (
        transform,
        persistent,
        health,
        mana,
        gold,
        stats,
        equipment,
        facing,
        progression,
        job_progression,
    ) {
        return Ok((
            persistent.snapshot(
                character_id,
                transform,
                facing,
                health,
                mana,
                gold,
                stats,
                equipment,
                save_point,
                progression,
                job_progression,
            ),
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

    let snapshot_map_name = persistent
        .map(|persistent| persistent.map_name.as_str())
        .unwrap_or(&snapshot.map_name);
    if let Some(transform) = transform {
        snapshot.position = map_to_local_position(snapshot_map_name, transform.translation).into();
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
    if let Some(gold) = gold {
        snapshot.gold = gold.0;
    }
    if let Some(stats) = stats {
        snapshot.stats = *stats;
    }
    if let Some(save_point) = save_point {
        snapshot.save_point = Some(save_point.clone());
    }
    if let Some(persistent) = persistent {
        snapshot.map_name.clone_from(&persistent.map_name);
    }
    if let Some(progression) = progression {
        snapshot.base_level = progression.level;
        snapshot.base_experience = progression.experience;
    }
    if let Some(job_progression) = job_progression {
        snapshot.class_id = job_progression.class.id();
        snapshot.job_level = job_progression.level;
        snapshot.job_experience = job_progression.experience;
    }

    Ok((snapshot, missing))
}

fn request_selected_character_load(
    client_id: ClientId,
    account_id: AccountId,
    character_id: CharacterId,
    persistence: &PersistenceClient,
    queue: &mut CharacterPersistenceQueue,
) -> bool {
    let request_id = queue.next_request_id();
    match persistence.send(PersistenceRequest::LoadCharacter {
        request_id,
        account_id,
        character_id,
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

fn send_account_message(
    server: &mut RenetServer,
    client_id: ClientId,
    message: AccountServerMessage,
) {
    match bincode::serialize(&message) {
        Ok(message) => server.send_message(client_id, ServerChannel::Account, message),
        Err(error) => error!("Could not serialize account response for {client_id}: {error}"),
    }
}

fn request_character_list(
    client_id: ClientId,
    account_id: AccountId,
    persistence: &PersistenceClient,
    queue: &mut CharacterPersistenceQueue,
) -> bool {
    let request_id = queue.next_request_id();
    match persistence.send(PersistenceRequest::ListCharacters {
        request_id,
        account_id,
    }) {
        Ok(()) => {
            queue.account_requests.insert(
                request_id,
                PendingAccountRequest::ListCharacters { client_id },
            );
            true
        }
        Err(error) => {
            error!("Could not request character list for client {client_id}: {error}");
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

fn save_changed_progression(
    persistence: Option<Res<PersistenceClient>>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
    players: Query<
        (
            &Transform,
            &CharacterId,
            &PersistentCharacter,
            &Health,
            &Mana,
            &Gold,
            &CharacterStats,
            &Equipment,
            Option<&SavePoint>,
            &Facing,
            &BaseProgression,
            &JobProgression,
        ),
        (
            With<Player>,
            Or<(
                Changed<BaseProgression>,
                Changed<JobProgression>,
                Changed<Gold>,
                Changed<CharacterStats>,
                Changed<Equipment>,
                Changed<SavePoint>,
                Changed<PersistentCharacter>,
            )>,
        ),
    >,
) {
    let Some(persistence) = persistence.as_deref() else {
        return;
    };

    for (
        transform,
        character_id,
        persistent,
        health,
        mana,
        gold,
        stats,
        equipment,
        save_point,
        facing,
        progression,
        job_progression,
    ) in &players
    {
        let snapshot = persistent.snapshot(
            *character_id,
            transform,
            facing,
            health,
            mana,
            gold,
            stats,
            equipment,
            save_point,
            progression,
            job_progression,
        );
        request_character_save(
            persistence,
            &mut persistence_queue,
            snapshot,
            "progression save",
        );
    }
}

fn server_events(
    mut server_events: ResMut<PendingServerEvents>,
    mut commands: Commands,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    mut players: Query<(
        Entity,
        &Player,
        &Transform,
        Option<&AuthoritativeCast>,
        &CharacterId,
        &Inventory,
        &mut ActionBarLayout,
        &JobProgression,
        &mut SkillTree,
        Option<&Dead>,
    )>,
    player_persistence_states: Query<
        (
            Entity,
            Option<&Transform>,
            Option<&CharacterId>,
            Option<&PersistentCharacter>,
            Option<&Health>,
            Option<&Mana>,
            Option<&Gold>,
            Option<&CharacterStats>,
            Option<&Equipment>,
            Option<&SavePoint>,
            Option<&Facing>,
            Option<&BaseProgression>,
            Option<&JobProgression>,
        ),
        With<Player>,
    >,
    monsters: Query<(Entity, &Monster, &Transform), With<Monster>>,
    animation_replication: ServerAnimationReplicationParams,
    mut movement: ServerMovementParams,
    persistence: Option<Res<PersistenceClient>>,
    persistence_status: Res<PersistenceStatus>,
    mut persistence_inbox: ResMut<PersistenceInbox>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
) {
    let time = movement.time.as_ref();
    let map = movement.map.as_ref();
    let connected_clients = server.clients_id();

    persistence_queue.waiting_clients.clear();

    for client_id in connected_clients.iter().copied() {
        while let Some(message) = server.receive_message(client_id, ClientChannel::Account) {
            let request = match bincode::deserialize::<AccountClientMessage>(&message) {
                Ok(request) => request,
                Err(error) => {
                    warn!("Rejected malformed account request from {client_id}: {error}");
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "The account request was invalid.".into(),
                        },
                    );
                    continue;
                }
            };

            let Some(persistence) = persistence.as_deref() else {
                send_account_message(
                    &mut server,
                    client_id,
                    AccountServerMessage::Error {
                        message: "Account login requires DATABASE_URL on the server.".into(),
                    },
                );
                continue;
            };
            match &*persistence_status {
                PersistenceStatus::Ready => {}
                PersistenceStatus::Connecting => {
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "The account database is still starting. Please try again."
                                .into(),
                        },
                    );
                    continue;
                }
                PersistenceStatus::Disabled => {
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "Account login is disabled because DATABASE_URL is not set."
                                .into(),
                        },
                    );
                    continue;
                }
                PersistenceStatus::Failed(failure) => {
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: format!("The account database is unavailable: {failure}"),
                        },
                    );
                    continue;
                }
            }

            match request {
                AccountClientMessage::Login { username, password } => {
                    if lobby.players.contains_key(&client_id) {
                        continue;
                    }
                    let request_id = persistence_queue.next_request_id();
                    if persistence
                        .send(PersistenceRequest::AuthenticateAccount {
                            request_id,
                            username: username.clone(),
                            password,
                        })
                        .is_ok()
                    {
                        persistence_queue.account_requests.insert(
                            request_id,
                            PendingAccountRequest::Login {
                                client_id,
                                username: username.trim().to_string(),
                            },
                        );
                    } else {
                        send_account_message(
                            &mut server,
                            client_id,
                            AccountServerMessage::Error {
                                message: "Could not contact the account database.".into(),
                            },
                        );
                    }
                }
                AccountClientMessage::CreateAccount { username, password } => {
                    if lobby.players.contains_key(&client_id) {
                        continue;
                    }
                    let request_id = persistence_queue.next_request_id();
                    if persistence
                        .send(PersistenceRequest::CreateAccount {
                            request_id,
                            username: username.clone(),
                            password,
                        })
                        .is_ok()
                    {
                        persistence_queue.account_requests.insert(
                            request_id,
                            PendingAccountRequest::Register {
                                client_id,
                                username: username.trim().to_string(),
                            },
                        );
                    } else {
                        send_account_message(
                            &mut server,
                            client_id,
                            AccountServerMessage::Error {
                                message: "Could not contact the account database.".into(),
                            },
                        );
                    }
                }
                AccountClientMessage::CreateCharacter { slot, name } => {
                    let Some(session) = persistence_queue
                        .authenticated_accounts
                        .get(&client_id)
                        .cloned()
                    else {
                        send_account_message(
                            &mut server,
                            client_id,
                            AccountServerMessage::Error {
                                message: "Log in before creating a character.".into(),
                            },
                        );
                        continue;
                    };
                    if lobby.players.contains_key(&client_id) {
                        continue;
                    }
                    let request_id = persistence_queue.next_request_id();
                    if persistence
                        .send(PersistenceRequest::CreateCharacter {
                            request_id,
                            character: crate::server::persistence::NewCharacter {
                                account_id: session.account_id,
                                slot,
                                name,
                            },
                        })
                        .is_ok()
                    {
                        persistence_queue.account_requests.insert(
                            request_id,
                            PendingAccountRequest::CreateCharacter { client_id },
                        );
                    }
                }
                AccountClientMessage::SelectCharacter { character_id } => {
                    let Some(session) = persistence_queue.authenticated_accounts.get(&client_id)
                    else {
                        send_account_message(
                            &mut server,
                            client_id,
                            AccountServerMessage::Error {
                                message: "Log in before selecting a character.".into(),
                            },
                        );
                        continue;
                    };
                    if lobby.players.contains_key(&client_id)
                        || persistence_queue
                            .load_requests
                            .values()
                            .any(|pending_client| *pending_client == client_id)
                    {
                        continue;
                    }
                    if !request_selected_character_load(
                        client_id,
                        session.account_id,
                        CharacterId(character_id),
                        persistence,
                        &mut persistence_queue,
                    ) {
                        send_account_message(
                            &mut server,
                            client_id,
                            AccountServerMessage::Error {
                                message: "Could not load that character.".into(),
                            },
                        );
                    }
                }
            }
        }
    }

    while let Some(response) = persistence_inbox.0.pop_front() {
        match response {
            PersistenceResponse::AccountAuthenticated {
                request_id,
                account_id,
            } => {
                let Some(PendingAccountRequest::Login {
                    client_id,
                    username,
                }) = persistence_queue.account_requests.remove(&request_id)
                else {
                    warn!("Received account login for unknown request {request_id}");
                    continue;
                };
                if !server.clients_id().contains(&client_id) {
                    continue;
                }
                let Some(account_id) = account_id else {
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "Incorrect username or password.".into(),
                        },
                    );
                    continue;
                };
                if persistence_queue
                    .authenticated_accounts
                    .iter()
                    .any(|(other_client, session)| {
                        *other_client != client_id && session.account_id == account_id
                    })
                {
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "That account is already logged in.".into(),
                        },
                    );
                    continue;
                }
                persistence_queue.authenticated_accounts.insert(
                    client_id,
                    AccountSession {
                        account_id,
                        username,
                    },
                );
                if let Some(persistence) = persistence.as_deref() {
                    request_character_list(
                        client_id,
                        account_id,
                        persistence,
                        &mut persistence_queue,
                    );
                }
            }
            PersistenceResponse::AccountCreated {
                request_id,
                account_id,
            } => {
                let Some(PendingAccountRequest::Register {
                    client_id,
                    username,
                }) = persistence_queue.account_requests.remove(&request_id)
                else {
                    warn!("Received account creation for unknown request {request_id}");
                    continue;
                };
                if !server.clients_id().contains(&client_id) {
                    continue;
                }
                persistence_queue.authenticated_accounts.insert(
                    client_id,
                    AccountSession {
                        account_id,
                        username,
                    },
                );
                if let Some(persistence) = persistence.as_deref() {
                    request_character_list(
                        client_id,
                        account_id,
                        persistence,
                        &mut persistence_queue,
                    );
                }
            }
            PersistenceResponse::CharactersListed {
                request_id,
                characters,
            } => {
                let Some(PendingAccountRequest::ListCharacters { client_id }) =
                    persistence_queue.account_requests.remove(&request_id)
                else {
                    warn!("Received character list for unknown request {request_id}");
                    continue;
                };
                let Some(session) = persistence_queue.authenticated_accounts.get(&client_id) else {
                    continue;
                };
                send_account_message(
                    &mut server,
                    client_id,
                    AccountServerMessage::CharacterList {
                        username: session.username.clone(),
                        characters: characters
                            .into_iter()
                            .map(|character| CharacterSelectionSummary {
                                id: character.id,
                                slot: character.slot,
                                name: character.name,
                                class_id: character.class_id,
                                base_level: character.base_level,
                                job_level: character.job_level,
                            })
                            .collect(),
                    },
                );
            }
            PersistenceResponse::CharacterCreated { request_id, .. } => {
                let Some(PendingAccountRequest::CreateCharacter { client_id }) =
                    persistence_queue.account_requests.remove(&request_id)
                else {
                    warn!("Received character creation for unknown request {request_id}");
                    continue;
                };
                if let (Some(session), Some(persistence)) = (
                    persistence_queue.authenticated_accounts.get(&client_id),
                    persistence.as_deref(),
                ) {
                    request_character_list(
                        client_id,
                        session.account_id,
                        persistence,
                        &mut persistence_queue,
                    );
                }
            }
            PersistenceResponse::CharacterLoaded {
                request_id,
                character,
                inventory,
                equipment,
                action_bar,
                learned_skills,
            } => {
                let Some(client_id) = persistence_queue.load_requests.remove(&request_id) else {
                    warn!("Received character load for unknown request {request_id}");
                    continue;
                };
                if !server.clients_id().contains(&client_id) {
                    continue;
                }

                let Some(character) = character else {
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "That character does not belong to this account.".into(),
                        },
                    );
                    continue;
                };

                let character_id = CharacterId(character.id);
                if player_persistence_states.iter().any(
                    |(_, _, active_id, _, _, _, _, _, _, _, _, _, _)| {
                        active_id.is_some_and(|active_id| *active_id == character_id)
                    },
                ) {
                    warn!(
                        "Rejecting duplicate login for persistent character {}",
                        character_id.0
                    );
                    send_account_message(
                        &mut server,
                        client_id,
                        AccountServerMessage::Error {
                            message: "That character is already in the world.".into(),
                        },
                    );
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
                send_account_message(&mut server, client_id, AccountServerMessage::EnteringWorld);
                spawn_and_announce_player(
                    &mut commands,
                    &mut lobby,
                    &mut server,
                    client_id,
                    PlayerSpawn::from_record(
                        character,
                        &map,
                        inventory,
                        equipment,
                        action_bar,
                        learned_skills,
                    ),
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
            PersistenceResponse::InventoryItemAdded {
                request_id,
                character_id,
                item_id,
                quantity,
            } => {
                info!(
                    "Persisted inventory item {} x{} for character {} (request {})",
                    item_id.0, quantity, character_id.0, request_id
                );
            }
            PersistenceResponse::InventoryItemRemoved {
                request_id,
                character_id,
                item_id,
                quantity,
            } => {
                info!(
                    "Persisted inventory consumption for item {} (remaining x{}) for character {} \
                     (request {})",
                    item_id.0, quantity, character_id.0, request_id
                );
            }
            PersistenceResponse::InventoryItemEquipped {
                request_id,
                character_id,
                item_id,
                slot,
            } => {
                info!(
                    "Persisted item {} in {} for character {} (request {})",
                    item_id.0,
                    slot.name(),
                    character_id.0,
                    request_id
                );
            }
            PersistenceResponse::InventoryItemUnequipped {
                request_id,
                character_id,
                item_id,
                slot,
            } => {
                info!(
                    "Persisted item {} unequipped from {} for character {} (request {})",
                    item_id.0,
                    slot.name(),
                    character_id.0,
                    request_id
                );
            }
            PersistenceResponse::ActionBarSlotSaved {
                request_id,
                character_id,
                slot_index,
            } => {
                info!(
                    "Persisted action bar slot {} for character {} (request {})",
                    slot_index + 1,
                    character_id.0,
                    request_id
                );
            }
            PersistenceResponse::ActionBarSwapSaved {
                request_id,
                character_id,
                first_slot,
                second_slot,
            } => {
                info!(
                    "Persisted action bar swap F{} <-> F{} for character {} (request {})",
                    first_slot + 1,
                    second_slot + 1,
                    character_id.0,
                    request_id
                );
            }
            PersistenceResponse::SkillRankSaved {
                request_id,
                character_id,
                skill_id,
                rank,
            } => {
                info!(
                    "Persisted skill {} at rank {} for character {} (request {})",
                    skill_id.0, rank, character_id.0, request_id
                );
            }
            PersistenceResponse::SkillPointsSaved {
                request_id,
                character_id,
                available_points,
            } => {
                info!(
                    "Persisted {} available skill point(s) for character {} (request {})",
                    available_points, character_id.0, request_id
                );
            }
            PersistenceResponse::SkillsCleared {
                request_id,
                character_id,
            } => {
                info!(
                    "Cleared learned skills for character {} (request {})",
                    character_id.0, request_id
                );
            }
            PersistenceResponse::RequestFailed {
                request_id,
                operation,
                message,
            } => {
                error!("Persistence failed to {operation}: {message}");
                if let Some(request_id) = request_id {
                    if let Some(pending) = persistence_queue.account_requests.remove(&request_id) {
                        let client_id = match pending {
                            PendingAccountRequest::Login { client_id, .. }
                            | PendingAccountRequest::Register { client_id, .. }
                            | PendingAccountRequest::ListCharacters { client_id }
                            | PendingAccountRequest::CreateCharacter { client_id } => client_id,
                        };
                        if server.clients_id().contains(&client_id) {
                            send_account_message(
                                &mut server,
                                client_id,
                                AccountServerMessage::Error { message },
                            );
                        }
                    } else if let Some(character_id) =
                        persistence_queue.save_requests.remove(&request_id)
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
            for (
                _entity,
                transform,
                character_id,
                persistent,
                health,
                mana,
                gold,
                stats,
                equipment,
                save_point,
                facing,
                progression,
                job_progression,
            ) in &player_persistence_states
            {
                let (
                    Some(transform),
                    Some(character_id),
                    Some(persistent),
                    Some(health),
                    Some(mana),
                    Some(gold),
                    Some(stats),
                    Some(equipment),
                    Some(facing),
                    Some(progression),
                    Some(job_progression),
                ) = (
                    transform,
                    character_id,
                    persistent,
                    health,
                    mana,
                    gold,
                    stats,
                    equipment,
                    facing,
                    progression,
                    job_progression,
                )
                else {
                    continue;
                };
                let snapshot = persistent.snapshot(
                    *character_id,
                    transform,
                    facing,
                    health,
                    mana,
                    gold,
                    stats,
                    equipment,
                    save_point,
                    progression,
                    job_progression,
                );
                request_character_save(persistence, &mut persistence_queue, snapshot, "autosave");
            }
        }
    }

    for event in server_events.0.drain(..) {
        match event {
            PendingServerEvent::Connected(client_id) => {
                println!("Client {client_id} connected");
            }
            PendingServerEvent::Disconnected(client_id, reason) => {
                println!("Player {} disconnected: {}", client_id, reason);

                persistence_queue.remove_client(client_id);
                //visualizer.remove_client(*client_id);
                if let Some(player_entity) = lobby.players.remove(&client_id) {
                    let tracked_character_id = lobby.characters.remove(&client_id);
                    let current_state = player_persistence_states.get(player_entity).ok();
                    let component_character_id = current_state.and_then(
                        |(_, _, character_id, _, _, _, _, _, _, _, _, _, _)| character_id.copied(),
                    );
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
                        let (
                            _,
                            transform,
                            _,
                            persistent,
                            health,
                            mana,
                            gold,
                            stats,
                            equipment,
                            save_point,
                            facing,
                            progression,
                            job_progression,
                        ) = current_state.unwrap_or((
                            player_entity,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        ));
                        let snapshot = disconnect_snapshot(
                            character_id,
                            transform,
                            persistent,
                            health,
                            mana,
                            gold,
                            stats,
                            equipment,
                            save_point,
                            facing,
                            progression,
                            job_progression,
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
        let mut sitting_after_commands = lobby
            .players
            .get(&client_id)
            .is_some_and(|player| animation_replication.sitting_players.contains(*player));
        while let Some(message) = server.receive_message(client_id, ClientChannel::Command) {
            let command: PlayerCommand = bincode::deserialize(&message).unwrap();
            if !matches!(&command, PlayerCommand::RespawnAtSavePoint)
                && lobby
                    .players
                    .get(&client_id)
                    .and_then(|player| players.get(*player).ok())
                    .is_some_and(|(_, _, _, _, _, _, _, _, _, dead)| dead.is_some())
            {
                warn!("Rejected command from dead player {client_id}: {command:?}");
                continue;
            }
            if sitting_after_commands && sitting_blocks_player_command(&command) {
                warn!("Rejected command from sitting player {client_id}: {command:?}");
                continue;
            }
            match command {
                PlayerCommand::RespawnAtSavePoint => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    if players
                        .get(player_entity)
                        .is_ok_and(|(_, _, _, _, _, _, _, _, _, dead)| dead.is_some())
                    {
                        commands.trigger(RespawnAtSavePoint {
                            player: player_entity,
                        });
                    }
                }
                PlayerCommand::CyclePlaceholderClass => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        if let Ok((_, _, _, _, character_id, _, mut action_bar, _, _, _)) =
                            players.get_mut(*player_entity)
                        {
                            let skill_slots = (0..ACTION_BAR_SLOT_COUNT)
                                .filter(|slot_index| {
                                    matches!(
                                        action_bar.binding(*slot_index),
                                        Some(ActionBarBinding::Skill(_))
                                    )
                                })
                                .collect::<Vec<_>>();

                            for slot_index in &skill_slots {
                                action_bar.set(*slot_index, None);
                            }

                            if character_id.0 != 0 {
                                if let Some(persistence) = persistence.as_deref() {
                                    let request_id = persistence_queue.next_request_id();
                                    if let Err(error) =
                                        persistence.send(PersistenceRequest::ClearSkills {
                                            request_id,
                                            character_id: *character_id,
                                        })
                                    {
                                        error!(
                                            "Could not clear skills for character {}: {}",
                                            character_id.0, error
                                        );
                                    }
                                    for slot_index in skill_slots {
                                        let request_id = persistence_queue.next_request_id();
                                        if let Err(error) = persistence.send(
                                            PersistenceRequest::SaveActionBarSlot {
                                                request_id,
                                                character_id: *character_id,
                                                slot_index: slot_index as u8,
                                                binding: None,
                                            },
                                        ) {
                                            error!(
                                                "Could not clear skill action bar slot {} for \
                                                 character {}: {}",
                                                slot_index + 1,
                                                character_id.0,
                                                error
                                            );
                                        }
                                    }
                                }
                            }

                            let message = bincode::serialize(&ServerMessages::ActionBarUpdated {
                                action_bar: action_bar.clone(),
                            })
                            .expect("action bar update should serialize");
                            server.send_message(client_id, ServerChannel::ServerMessages, message);
                        }
                        commands.trigger(CyclePlaceholderClass {
                            player: *player_entity,
                        });
                    }
                }
                PlayerCommand::SpendSkillPoint { skill_id } => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    let Ok((_, _, _, _, character_id, _, _, job_progression, mut skill_tree, _)) =
                        players.get_mut(player_entity)
                    else {
                        continue;
                    };
                    match skill_tree.spend_point(job_progression.class, skill_id) {
                        Ok(rank) => {
                            info!(
                                "Player {client_id} raised skill {} to rank {}",
                                skill_id.0, rank
                            );
                            if character_id.0 != 0 {
                                if let Some(persistence) = persistence.as_deref() {
                                    let request_id = persistence_queue.next_request_id();
                                    if let Err(error) =
                                        persistence.send(PersistenceRequest::SaveSkillRank {
                                            request_id,
                                            character_id: *character_id,
                                            skill_id,
                                            rank,
                                            available_points: skill_tree.available_points(),
                                        })
                                    {
                                        error!(
                                            "Could not persist skill {} for character {}: {}",
                                            skill_id.0, character_id.0, error
                                        );
                                    }
                                }
                            }
                        }
                        Err(reason) => warn!(
                            "Rejected skill {} allocation from client {}: {:?}",
                            skill_id.0, client_id, reason
                        ),
                    }
                }
                PlayerCommand::SpendAttributePoint { attribute } => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    commands.trigger(SpendAttributePoint {
                        player: player_entity,
                        attribute,
                    });
                }
                PlayerCommand::CycleMap => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    let Ok((_, _, _, _, _, _, _, _, _, dead)) = players.get(player_entity) else {
                        continue;
                    };
                    if dead.is_some() {
                        warn!("Rejected map change from dead player {client_id}");
                        continue;
                    }
                    let Ok((current_map, persistent)) =
                        movement.player_map_states.get(player_entity)
                    else {
                        continue;
                    };
                    let destination_map = next_map_name(&current_map.0);
                    let destination = map_spawn_position(destination_map);
                    let mut entity_commands = commands.entity(player_entity);
                    entity_commands
                        .insert((
                            Transform::from_translation(destination),
                            CurrentMap(destination_map.to_string()),
                            PlayerInput::default(),
                            GameVelocity::default(),
                            TargetPos {
                                position: destination,
                            },
                            LineOfSight::default(),
                            player_character_controller(),
                            SpawnProtection::default(),
                        ))
                        .remove::<Aggro>()
                        .remove::<Attacking>()
                        .remove::<AttackingTimer>()
                        .remove::<AuthoritativeCast>()
                        .remove::<PendingItemPickup>()
                        .remove::<Sitting>()
                        .remove::<Walking>();
                    if let Some(persistent) = persistent {
                        let mut updated = persistent.clone();
                        updated.map_name = destination_map.to_string();
                        entity_commands.insert(updated);
                    }

                    let message = bincode::serialize(&ServerMessages::MapChanged {
                        entity: player_entity,
                        map_name: destination_map.to_string(),
                        translation: destination.into(),
                        server_time: time.elapsed().as_millis(),
                    })
                    .expect("map change should serialize");
                    server.send_message(client_id, ServerChannel::ServerMessages, message);
                    info!(
                        "Moved client {} from '{}' to '{}'",
                        client_id, current_map.0, destination_map
                    );
                }
                PlayerCommand::Cast {
                    spell_id,
                    cast_at,
                    target_entity,
                } => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands
                            .entity(*player_entity)
                            .remove::<PendingItemPickup>();
                        commands.trigger(RequestSpellCast {
                            caster: *player_entity,
                            spell_id,
                            target: cast_at,
                            target_entity,
                        });
                    }
                }
                PlayerCommand::BasicAttack {
                    entity,
                    auto_attack,
                } => {
                    println!(
                        "Received {}basic attack from client {}: {:?}",
                        if auto_attack { "auto " } else { "" },
                        client_id,
                        entity
                    );

                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        if let Ok(mut walk_delay) =
                            movement.damage_walk_delays.get_mut(*player_entity)
                        {
                            walk_delay.cancel_pending_destination();
                        }
                        if let (
                            Ok((
                                _entity,
                                _player,
                                _player_transform,
                                active_cast,
                                _character_id,
                                _inventory,
                                _action_bar,
                                _job_progression,
                                _skill_tree,
                                _dead,
                            )),
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

                            commands
                                .entity(*player_entity)
                                .remove::<PendingItemPickup>()
                                .remove::<SpawnProtection>();
                            commands.entity(*player_entity).insert(Aggro {
                                enemy: monster_entity,
                                auto_attack,
                                enemy_translation: monster_transform.translation, //path: get_path_between_translations(player_transform.translation, monster_transform.translation, &map),
                                                                                  // timer: timer // El timer se debe definir al momento en que ya está en rango. Ya que el aspd puede variar mientras te acercas.
                            });
                        }
                    }
                }
                PlayerCommand::StopBasicAttack => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        if let Ok(mut walk_delay) =
                            movement.damage_walk_delays.get_mut(*player_entity)
                        {
                            walk_delay.cancel_pending_destination();
                        }
                        commands
                            .entity(*player_entity)
                            .remove::<Aggro>()
                            .remove::<Attacking>()
                            .remove::<AttackingTimer>()
                            .remove::<Walking>()
                            .remove::<TargetPos>();
                    }
                }
                PlayerCommand::Face { target } => {
                    if !sitting_after_commands {
                        warn!("Rejected facing command from standing player {client_id}");
                        continue;
                    }
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    let Ok((_, _, player_transform, _, _, _, _, _, _, _)) =
                        players.get(player_entity)
                    else {
                        continue;
                    };
                    let Some(facing) = facing_from_direction(target - player_transform.translation)
                    else {
                        continue;
                    };

                    commands.entity(player_entity).insert(facing.clone());
                    let facing_message = bincode::serialize(&ServerMessages::FacingChanged {
                        entity: player_entity,
                        facing,
                    })
                    .expect("facing state should serialize");
                    if let Ok((_, _, player_line_of_sight)) =
                        animation_replication.viewers.get(player_entity)
                    {
                        for (viewer_entity, viewer, line_of_sight) in &animation_replication.viewers
                        {
                            if should_receive_player_action(
                                viewer_entity,
                                player_entity,
                                line_of_sight,
                                player_line_of_sight,
                            ) {
                                server.send_message(
                                    viewer.id,
                                    ServerChannel::ServerMessages,
                                    facing_message.clone(),
                                );
                            }
                        }
                    }
                }
                PlayerCommand::ToggleSitting => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    let Ok((_, _, player_transform, active_cast, _, _, _, _, _, _)) =
                        players.get(player_entity)
                    else {
                        continue;
                    };
                    if active_cast.is_some() {
                        info!("Ignoring sit toggle from client {client_id} while casting");
                        continue;
                    }

                    let sitting = !sitting_after_commands;
                    sitting_after_commands = sitting;
                    if let Ok(mut walk_delay) = movement.damage_walk_delays.get_mut(player_entity) {
                        walk_delay.cancel_pending_destination();
                    }

                    let mut player_commands = commands.entity(player_entity);
                    player_commands
                        .insert(PlayerInput::default())
                        .remove::<Aggro>()
                        .remove::<Attacking>()
                        .remove::<AttackingTimer>()
                        .remove::<PendingItemPickup>()
                        .remove::<Walking>()
                        .remove::<TargetPos>();
                    if sitting {
                        player_commands.insert(Sitting);
                    } else {
                        player_commands.remove::<Sitting>();
                    }

                    let movement_message =
                        bincode::serialize(&ServerMessages::MovementInterrupted {
                            entity: player_entity,
                            translation: player_transform.translation.into(),
                            server_time: time.elapsed().as_millis(),
                        })
                        .expect("sitting movement interruption should serialize");
                    server.send_message(client_id, ServerChannel::ServerMessages, movement_message);

                    let sitting_message = bincode::serialize(&ServerMessages::SittingChanged {
                        entity: player_entity,
                        sitting,
                    })
                    .expect("sitting state should serialize");
                    if let Ok((_, _, player_line_of_sight)) =
                        animation_replication.viewers.get(player_entity)
                    {
                        for (viewer_entity, viewer, line_of_sight) in &animation_replication.viewers
                        {
                            if should_receive_player_action(
                                viewer_entity,
                                player_entity,
                                line_of_sight,
                                player_line_of_sight,
                            ) {
                                server.send_message(
                                    viewer.id,
                                    ServerChannel::ServerMessages,
                                    sitting_message.clone(),
                                );
                            }
                        }
                    }
                }
                PlayerCommand::PickupItem { entity } => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands.trigger(RequestItemPickup {
                            player: *player_entity,
                            ground_item: entity,
                        });
                    }
                }
                PlayerCommand::UseItem { item_id } => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands.trigger(RequestItemUse {
                            player: *player_entity,
                            item_id,
                        });
                    }
                }
                PlayerCommand::EquipItem { item_id } => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands.trigger(RequestEquipItem {
                            player: *player_entity,
                            item_id,
                        });
                    }
                }
                PlayerCommand::UnequipItem { slot } => {
                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        commands.trigger(RequestUnequipItem {
                            player: *player_entity,
                            slot,
                        });
                    }
                }
                PlayerCommand::SetActionBarSlot {
                    slot_index,
                    binding,
                } => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    let Ok((_, _, _, _, character_id, inventory, mut action_bar, _, skill_tree, _)) =
                        players.get_mut(player_entity)
                    else {
                        continue;
                    };

                    if (slot_index as usize) < ACTION_BAR_SLOT_COUNT
                        && valid_action_bar_binding(binding, inventory, &skill_tree)
                    {
                        action_bar.set(slot_index as usize, binding);
                        if character_id.0 != 0 {
                            if let Some(persistence) = persistence.as_deref() {
                                let request_id = persistence_queue.next_request_id();
                                if let Err(error) =
                                    persistence.send(PersistenceRequest::SaveActionBarSlot {
                                        request_id,
                                        character_id: *character_id,
                                        slot_index,
                                        binding,
                                    })
                                {
                                    error!(
                                        "Could not persist action bar slot {} for character {}: {}",
                                        slot_index + 1,
                                        character_id.0,
                                        error
                                    );
                                }
                            }
                        }
                    } else {
                        warn!(
                            "Rejected invalid action bar binding {:?} in slot {} from client {}",
                            binding,
                            slot_index + 1,
                            client_id
                        );
                    }

                    let message = bincode::serialize(&ServerMessages::ActionBarUpdated {
                        action_bar: action_bar.clone(),
                    })
                    .expect("action bar update should serialize");
                    server.send_message(client_id, ServerChannel::ServerMessages, message);
                }
                PlayerCommand::SwapActionBarSlots {
                    first_slot,
                    second_slot,
                } => {
                    let Some(player_entity) = lobby.players.get(&client_id).copied() else {
                        continue;
                    };
                    let Ok((_, _, _, _, character_id, _, mut action_bar, _, _, _)) =
                        players.get_mut(player_entity)
                    else {
                        continue;
                    };

                    let valid_slots = (first_slot as usize) < ACTION_BAR_SLOT_COUNT
                        && (second_slot as usize) < ACTION_BAR_SLOT_COUNT;
                    if valid_slots && first_slot != second_slot {
                        action_bar.swap(first_slot as usize, second_slot as usize);
                        let first_binding = action_bar.binding(first_slot as usize);
                        let second_binding = action_bar.binding(second_slot as usize);

                        if character_id.0 != 0 {
                            if let Some(persistence) = persistence.as_deref() {
                                let request_id = persistence_queue.next_request_id();
                                if let Err(error) =
                                    persistence.send(PersistenceRequest::SaveActionBarSwap {
                                        request_id,
                                        character_id: *character_id,
                                        first_slot,
                                        first_binding,
                                        second_slot,
                                        second_binding,
                                    })
                                {
                                    error!(
                                        "Could not persist action bar swap F{} <-> F{} for \
                                         character {}: {}",
                                        first_slot + 1,
                                        second_slot + 1,
                                        character_id.0,
                                        error
                                    );
                                }
                            }
                        }
                    } else {
                        warn!(
                            "Rejected invalid action bar swap F{} <-> F{} from client {}",
                            first_slot + 1,
                            second_slot + 1,
                            client_id
                        );
                    }

                    let message = bincode::serialize(&ServerMessages::ActionBarUpdated {
                        action_bar: action_bar.clone(),
                    })
                    .expect("action bar update should serialize");
                    server.send_message(client_id, ServerChannel::ServerMessages, message);
                }
                PlayerCommand::Move { destination_at } => {
                    if let Some(player_entity) = lobby.players.get_mut(&client_id) {
                        if let Ok((
                            _entity,
                            _player,
                            player_transform,
                            active_cast,
                            _character_id,
                            _inventory,
                            _action_bar,
                            _job_progression,
                            _skill_tree,
                            _dead,
                        )) = players.get(*player_entity)
                        {
                            if active_cast.is_some() {
                                info!("Ignoring movement from client {client_id} while casting");
                                continue;
                            }
                            commands.entity(*player_entity).remove::<SpawnProtection>();
                            if let Ok(mut walk_delay) =
                                movement.damage_walk_delays.get_mut(*player_entity)
                            {
                                walk_delay.queue_destination(destination_at);
                                commands
                                    .entity(*player_entity)
                                    .remove::<Aggro>()
                                    .remove::<Attacking>()
                                    .remove::<AttackingTimer>()
                                    .remove::<PendingItemPickup>()
                                    .remove::<Walking>()
                                    .remove::<TargetPos>();
                                let message =
                                    bincode::serialize(&ServerMessages::MovementInterrupted {
                                        entity: *player_entity,
                                        translation: player_transform.translation.into(),
                                        server_time: time.elapsed().as_millis(),
                                    })
                                    .expect("movement interruption should serialize");
                                server.send_message(
                                    client_id,
                                    ServerChannel::ServerMessages,
                                    message,
                                );
                                info!(
                                    "Queued movement from client {client_id} until hit-stun ends"
                                );
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
                                .remove::<AttackingTimer>()
                                .remove::<PendingItemPickup>();

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
                        if let Ok(mut walk_delay) =
                            movement.damage_walk_delays.get_mut(*player_entity)
                        {
                            walk_delay.cancel_pending_destination();
                        }
                        commands
                            .entity(*player_entity)
                            .remove::<Walking>()
                            .remove::<Aggro>()
                            .remove::<Attacking>()
                            .remove::<AttackingTimer>()
                            .remove::<PendingItemPickup>();
                    }
                }
            }
        }
        while let Some(message) = server.receive_message(client_id, ClientChannel::Input) {
            let input: PlayerInput = bincode::deserialize(&message).unwrap();

            if let Some(player_entity) = lobby.players.get(&client_id) {
                let alive = players
                    .get(*player_entity)
                    .is_ok_and(|(_, _, _, _, _, _, _, _, _, dead)| dead.is_none());
                if alive && !sitting_after_commands {
                    commands.entity(*player_entity).insert(input);
                }
            }
        }
    }
}

fn sitting_blocks_player_command(command: &PlayerCommand) -> bool {
    matches!(
        command,
        PlayerCommand::Move { .. }
            | PlayerCommand::BasicAttack { .. }
            | PlayerCommand::PickupItem { .. }
            | PlayerCommand::Cast { .. }
    )
}

fn valid_action_bar_binding(
    binding: Option<ActionBarBinding>,
    inventory: &Inventory,
    skill_tree: &SkillTree,
) -> bool {
    match binding {
        None => true,
        Some(ActionBarBinding::Spell(spell_id)) => spell_definition(spell_id).is_some(),
        Some(ActionBarBinding::Item(item_id)) => {
            item_definition(item_id).is_some() && inventory.quantity(item_id) > 0
        }
        Some(ActionBarBinding::Skill(skill_id)) => skill_tree.rank(skill_id) > 0,
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

    #[cfg(not(feature = "batched_position_snapshots"))]
    {
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

    #[cfg(feature = "batched_position_snapshots")]
    for (player, line_of_sight) in &players {
        for batch in visible_position_snapshot_batches(&snapshots, line_of_sight, server_time) {
            let message =
                bincode::serialize(&batch).expect("position snapshot batch should serialize");
            server.send_message(player.id, ServerChannel::NetworkedEntities, message);
        }
    }
}

#[cfg(feature = "batched_position_snapshots")]
fn visible_position_snapshot_batches(
    snapshots: &[EntitySnapshot],
    line_of_sight: &LineOfSight,
    server_time: u128,
) -> Vec<EntitySnapshotBatch> {
    let mut batches = Vec::new();
    let mut current = Vec::with_capacity(MAX_POSITION_SNAPSHOTS_PER_BATCH);

    for snapshot in snapshots {
        // `line_of_sight` is sorted whenever the interest set is refreshed.
        if line_of_sight.0.binary_search(&snapshot.entity).is_err() {
            continue;
        }
        current.push(QuantizedEntityPosition::from(snapshot));
        if current.len() == MAX_POSITION_SNAPSHOTS_PER_BATCH {
            batches.push(EntitySnapshotBatch {
                server_time,
                snapshots: std::mem::take(&mut current),
            });
            current = Vec::with_capacity(MAX_POSITION_SNAPSHOTS_PER_BATCH);
        }
    }

    if !current.is_empty() {
        batches.push(EntitySnapshotBatch {
            server_time,
            snapshots: current,
        });
    }

    batches
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
            &BaseProgression,
            &JobProgression,
            &CurrentMap,
            Option<&Sitting>,
        ),
        With<Player>,
    >,
    treeaccess: Res<NNTree>,
    ground_items: Query<(Entity, &GroundItem, &Transform)>,
    entities: Query<(Entity, &Transform, &SpriteId, &Facing, Option<&Health>)>,
    transforms: Query<&Transform>,
    time: Res<Time>,
) {
    for (viewer_entity, player, transform, mut line_of_sight) in viewers.iter_mut() {
        let old_set: HashSet<Entity> = line_of_sight.0.iter().copied().collect();
        let within_distance = treeaccess.within_distance(transform.translation, LINE_OF_SIGHT);

        let mut new_set: HashSet<Entity> =
            within_distance.iter().filter_map(|entry| entry.1).collect();

        // Keep an already-visible entity for a small margin beyond the entry
        // radius. Without this hysteresis, tiny interpolation differences at
        // exactly 14 units can alternate spawn/despawn messages every update.
        for old_entity in &old_set {
            if new_set.contains(old_entity) {
                continue;
            }
            if transforms.get(*old_entity).is_ok_and(|old_transform| {
                remains_in_line_of_sight(transform.translation, old_transform.translation)
            }) {
                new_set.insert(*old_entity);
            }
        }

        // Player replication must not depend solely on the periodically rebuilt
        // spatial tree. A player can connect between tree rebuilds, and every
        // nearby client still needs an immediate, symmetric PlayerCreate.
        include_nearby_players(
            &mut new_set,
            transform.translation,
            players
                .iter()
                .map(|(entity, _, transform, _, _, _, _, _, _, _, _, _)| {
                    (entity, transform.translation)
                }),
        );

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
                progression,
                job_progression,
                current_map,
                sitting,
            )) = players.get(*spawned_entity)
            {
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: spawned_player.id,
                    entity: *spawned_entity,
                    character_id: *character_id,
                    map_name: current_map.0.clone(),
                    translation: spawned_transform.translation.into(),
                    facing: facing.clone(),
                    health: health.clone(),
                    mana: mana.clone(),
                    progression: *progression,
                    job_progression: *job_progression,
                    attack_speed: attack_speed.0,
                    sitting: sitting.is_some(),
                    server_time: time.elapsed().as_millis(),
                })
                .unwrap();
                server.send_message(player.id, ServerChannel::ServerMessages, message);
                continue;
            }

            if let Ok((entity, item, transform)) = ground_items.get(*spawned_entity) {
                let message = bincode::serialize(&ServerMessages::SpawnGroundItem {
                    entity,
                    item: item.clone(),
                    translation: transform.translation.into(),
                })
                .expect("ground item spawn should serialize");
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

const LINE_OF_SIGHT_EXIT_MARGIN: f32 = 1.0;

fn remains_in_line_of_sight(viewer: Vec3, entity: Vec3) -> bool {
    let exit_distance = LINE_OF_SIGHT + LINE_OF_SIGHT_EXIT_MARGIN;
    viewer.distance_squared(entity) <= exit_distance * exit_distance
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
    fn visible_entities_get_a_hysteresis_margin_before_despawning() {
        assert!(remains_in_line_of_sight(
            Vec3::ZERO,
            Vec3::X * (LINE_OF_SIGHT + 0.5)
        ));
        assert!(!remains_in_line_of_sight(
            Vec3::ZERO,
            Vec3::X * (LINE_OF_SIGHT + LINE_OF_SIGHT_EXIT_MARGIN + 0.01)
        ));
    }

    #[test]
    fn attribute_spending_is_server_authoritative_and_updates_resource_maxima() {
        let mut app = App::new();
        app.add_observer(on_spend_attribute_point);
        let player = app
            .world_mut()
            .spawn((
                Player { id: 7 },
                CharacterStats {
                    available_points: 4,
                    ..default()
                },
                BaseProgression::default(),
                Equipment::default(),
                Health {
                    current: 35,
                    max: 40,
                },
                Mana {
                    current: 8,
                    max: 10,
                },
            ))
            .id();

        app.world_mut().trigger(SpendAttributePoint {
            player,
            attribute: CharacterAttribute::Vitality,
        });
        app.world_mut().flush();

        let stats = app.world().get::<CharacterStats>(player).unwrap();
        assert_eq!(stats.vitality, 2);
        assert_eq!(stats.available_points, 2);
        assert_eq!(app.world().get::<Health>(player).unwrap().max, 45);

        app.world_mut().trigger(SpendAttributePoint {
            player,
            attribute: CharacterAttribute::Intellect,
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .get::<CharacterStats>(player)
                .unwrap()
                .available_points,
            0
        );
        assert_eq!(app.world().get::<Mana>(player).unwrap().max, 12);
    }

    #[test]
    fn action_bar_accepts_owned_items_and_known_spells_only() {
        let mut inventory = Inventory::default();
        inventory.add(crate::shared::gameplay::items::RED_HERB, 1);
        let skill_tree = SkillTree::from_persisted(
            crate::shared::gameplay::progression::CharacterClass::Mage,
            5,
            [LearnedSkill {
                id: crate::shared::gameplay::skills::SkillId(300),
                rank: 2,
            }],
        );

        assert!(valid_action_bar_binding(
            Some(ActionBarBinding::Item(
                crate::shared::gameplay::items::RED_HERB
            )),
            &inventory,
            &skill_tree
        ));
        assert!(!valid_action_bar_binding(
            Some(ActionBarBinding::Item(
                crate::shared::gameplay::items::PIG_MEAT
            )),
            &inventory,
            &skill_tree
        ));
        assert!(valid_action_bar_binding(
            Some(ActionBarBinding::Spell(4)),
            &inventory,
            &skill_tree
        ));
        assert!(!valid_action_bar_binding(
            Some(ActionBarBinding::Spell(999)),
            &inventory,
            &skill_tree
        ));
        assert!(valid_action_bar_binding(
            Some(ActionBarBinding::Skill(
                crate::shared::gameplay::skills::SkillId(300)
            )),
            &inventory,
            &skill_tree
        ));
        assert!(!valid_action_bar_binding(
            Some(ActionBarBinding::Skill(
                crate::shared::gameplay::skills::SkillId(301)
            )),
            &inventory,
            &skill_tree
        ));
    }

    #[test]
    fn placeholder_class_cycle_resets_only_job_progression() {
        let mut app = App::new();
        app.add_observer(on_cycle_placeholder_class);
        let player = app
            .world_mut()
            .spawn((
                Player { id: 1 },
                BaseProgression {
                    level: 12,
                    experience: 34,
                },
                JobProgression {
                    class: crate::shared::gameplay::progression::CharacterClass::Novice,
                    level: 8,
                    experience: 20,
                },
                SkillTree::from_persisted(
                    crate::shared::gameplay::progression::CharacterClass::Novice,
                    8,
                    [LearnedSkill {
                        id: crate::shared::gameplay::skills::SkillId(100),
                        rank: 2,
                    }],
                ),
                ActionBarLayout {
                    slots: [
                        Some(ActionBarBinding::Skill(
                            crate::shared::gameplay::skills::SkillId(100),
                        )),
                        Some(ActionBarBinding::Spell(2)),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                },
            ))
            .id();

        app.world_mut().trigger(CyclePlaceholderClass { player });

        assert_eq!(
            app.world().get::<BaseProgression>(player),
            Some(&BaseProgression {
                level: 12,
                experience: 34,
            })
        );
        assert_eq!(
            app.world().get::<JobProgression>(player),
            Some(&JobProgression {
                class: crate::shared::gameplay::progression::CharacterClass::Swordsman,
                level: 1,
                experience: 0,
            })
        );
        assert_eq!(
            app.world().get::<SkillTree>(player),
            Some(&SkillTree::default())
        );
        assert_eq!(
            app.world()
                .get::<ActionBarLayout>(player)
                .and_then(|action_bar| action_bar.binding(0)),
            None
        );
        assert_eq!(
            app.world()
                .get::<ActionBarLayout>(player)
                .and_then(|action_bar| action_bar.binding(1)),
            Some(ActionBarBinding::Spell(2))
        );
    }

    #[test]
    fn legacy_blocked_spawns_are_relocated_even_before_navigation_is_ready() {
        let map = Map::default();

        assert_eq!(
            resolve_persistent_spawn(STARTING_MAP_NAME, Vec3::new(0.0, 1.0, 0.0), &map),
            default_character_spawn()
        );
        assert_eq!(
            resolve_persistent_spawn(STARTING_MAP_NAME, Vec3::new(-12.0, 1.0, 16.0), &map,),
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
            resolve_persistent_spawn(STARTING_MAP_NAME, blocked_spawn, &map),
            default_character_spawn()
        );
        assert_eq!(
            resolve_persistent_spawn(STARTING_MAP_NAME, open_spawn, &map),
            open_spawn
        );
    }

    #[test]
    fn respawn_uses_the_saved_point_or_the_game_start_fallback() {
        let save_point = SavePoint {
            map_name: STARTING_MAP_NAME.into(),
            position: [7.0, 1.0, -4.0],
        };

        assert_eq!(
            respawn_destination(Some(&save_point)),
            (STARTING_MAP_NAME, Vec3::new(7.0, 1.0, -4.0))
        );
        assert_eq!(
            respawn_destination(None),
            (STARTING_MAP_NAME, default_character_spawn())
        );
    }

    fn snapshot_at(character_id: CharacterId, position: [f32; 3]) -> CharacterSnapshot {
        CharacterSnapshot {
            character_id,
            class_id: 0,
            base_level: 1,
            base_experience: 0,
            job_level: 1,
            job_experience: 0,
            hp: 40,
            max_hp: 40,
            sp: 10,
            max_sp: 10,
            gold: 0,
            stats: CharacterStats::default(),
            map_name: "starting_map".into(),
            save_point: None,
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
            None,
            None,
            None,
            None,
            Some(&facing),
            None,
            None,
            Some(&cached),
        )
        .expect("the cached database state should complete the snapshot");

        assert_eq!(snapshot.position, [8.0, 3.0, -4.0]);
        assert_eq!(snapshot.hp, 31);
        assert_eq!(snapshot.sp, cached.sp);
        assert_eq!(snapshot.facing, 6);
        assert_eq!(
            missing,
            vec![
                "PersistentCharacter",
                "Mana",
                "Gold",
                "CharacterStats",
                "Equipment",
                "BaseProgression",
                "JobProgression"
            ]
        );
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
        world.insert_resource(RenetServer::new(connection_config()));
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
    fn progression_change_queues_an_immediate_persistent_save() {
        let character_id = CharacterId(1);
        let (persistence, mut requests) = PersistenceClient::test_channel();
        let mut world = World::new();
        world.insert_resource(persistence);

        let mut queue = CharacterPersistenceQueue::default();
        queue.revisions.insert(character_id, 4);
        world.insert_resource(queue);
        world.spawn((
            Player { id: 1 },
            Transform::from_xyz(2.0, 1.0, -3.0),
            character_id,
            PersistentCharacter {
                account_id: AccountId(1),
                revision: 4,
                map_name: "starting_map".into(),
            },
            Health {
                current: 40,
                max: 40,
            },
            Mana {
                current: 10,
                max: 10,
            },
            Gold(25),
            CharacterStats {
                might: 8,
                available_points: 3,
                ..default()
            },
            Equipment::default(),
            Facing(2),
            BaseProgression {
                level: 3,
                experience: 25,
            },
            JobProgression {
                class: crate::shared::gameplay::progression::CharacterClass::Mage,
                level: 4,
                experience: 30,
            },
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(save_changed_progression);
        schedule.run(&mut world);

        let PersistenceRequest::SaveCharacter { snapshot, .. } = requests
            .try_recv()
            .expect("progression change should queue a save")
        else {
            panic!("expected a character save request");
        };
        assert_eq!(snapshot.base_level, 3);
        assert_eq!(snapshot.base_experience, 25);
        assert_eq!(snapshot.class_id, 2);
        assert_eq!(snapshot.job_level, 4);
        assert_eq!(snapshot.job_experience, 30);
        assert_eq!(snapshot.gold, 25);
        assert_eq!(snapshot.stats.might, 8);
        assert_eq!(snapshot.stats.available_points, 3);
        assert_eq!(snapshot.expected_revision, 4);
    }

    #[test]
    fn configured_persistence_failure_does_not_spawn_an_unsavable_player() {
        let client_id = 1;
        let mut world = World::new();
        world.insert_resource(PendingServerEvents(vec![PendingServerEvent::Connected(
            client_id,
        )]));
        world.insert_resource(ServerLobby::default());
        world.insert_resource(RenetServer::new(connection_config()));
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
    fn sitting_blocks_world_actions_but_allows_standing_and_stop_commands() {
        for command in [
            PlayerCommand::Move {
                destination_at: Vec3::X,
            },
            PlayerCommand::BasicAttack {
                entity: Entity::PLACEHOLDER,
                auto_attack: false,
            },
            PlayerCommand::PickupItem {
                entity: Entity::PLACEHOLDER,
            },
            PlayerCommand::Cast {
                spell_id: 2,
                cast_at: Vec3::ZERO,
                target_entity: None,
            },
        ] {
            assert!(sitting_blocks_player_command(&command));
        }

        assert!(!sitting_blocks_player_command(
            &PlayerCommand::ToggleSitting
        ));
        assert!(!sitting_blocks_player_command(&PlayerCommand::Face {
            target: Vec3::X,
        }));
        assert!(!sitting_blocks_player_command(&PlayerCommand::StopMoving));
        assert!(!sitting_blocks_player_command(
            &PlayerCommand::StopBasicAttack
        ));
    }

    #[cfg(feature = "batched_position_snapshots")]
    #[test]
    fn position_batches_are_visibility_filtered_chunked_and_mtu_safe() {
        let mut world = World::new();
        let entities: Vec<_> = (0..100).map(|_| world.spawn_empty().id()).collect();
        let snapshots: Vec<_> = entities
            .iter()
            .enumerate()
            .map(|(index, entity)| EntitySnapshot {
                entity: *entity,
                x: index as i32,
                y: 1_000,
                z: -(index as i32),
                server_time: 111,
            })
            .collect();
        let mut visible_entities = entities[..97].to_vec();
        visible_entities.sort_unstable();

        let batches = visible_position_snapshot_batches(
            &snapshots,
            &LineOfSight(visible_entities.clone()),
            222,
        );

        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.snapshots.len())
                .collect::<Vec<_>>(),
            vec![48, 48, 1]
        );
        assert!(batches.iter().all(|batch| batch.server_time == 222));
        assert!(batches.iter().all(|batch| {
            bincode::serialize(batch).is_ok_and(|encoded| encoded.len() <= 1_200)
        }));
        let mut received: Vec<_> = batches
            .iter()
            .flat_map(|batch| batch.snapshots.iter().map(|snapshot| snapshot.entity))
            .collect();
        received.sort_unstable();
        assert_eq!(received, visible_entities);
    }

    #[test]
    fn server_events_system_has_valid_bevy_parameters() {
        let mut world = World::new();
        world.insert_resource(PendingServerEvents::default());
        world.insert_resource(ServerLobby::default());
        world.insert_resource(RenetServer::new(connection_config()));
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
