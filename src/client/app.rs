use bevy::core_pipeline::prepass::DepthPrepass;

// use avian3d::math::Scalar;
// use avian3d::prelude::*;
use crate::client::assets::*;
use crate::client::network::clock_sync::*;
use crate::client::network::movement::*;
use crate::client::presentation::animations::{AttackSpriteVisual, WalkingSpriteVisual};
use crate::client::presentation::casting::{
    ConfirmedSpellCastCompleted, ConfirmedSpellCastStarted,
};
use crate::client::presentation::damage_numbers::{DamageNumberEvent, DamageNumbersPlugin};
use crate::client::state::*;
use crate::shared::constants::*;
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::entities::{AttackSpeed, Player};
use crate::shared::network::{channels::*, messages::*};
use crate::shared::states::ClientState;
use crate::world::setup_level;
use bevy_egui::EguiPlugin;
use bevy_obj::ObjPlugin;
use bevy_sprite3d::prelude::*;
use local_ip_address::local_ip;
use std::ops::Mul;

use crate::client::presentation::health_bars::{BarHeight, BarSettings};

use bevy::{
    camera::Exposure,
    core_pipeline::tonemapping::Tonemapping,
    light::{
        atmosphere::ScatteringMedium, light_consts::lux, Atmosphere, AtmosphereEnvironmentMapLight,
        GlobalAmbientLight,
    },
    log::LogPlugin,
    pbr::AtmosphereSettings,
    post_process::bloom::Bloom,
    prelude::*,
    window::{close_when_requested, Window, WindowCloseRequested, WindowResolution},
};
// pub use bevy_renet::renet::transport::ClientAuthentication;
use bevy_asset_loader::prelude::*;
use bevy_renet::netcode::*;
pub use bevy_renet::netcode::{ClientAuthentication, NetcodeClientPlugin};
use bevy_renet::{RenetClient, RenetClientPlugin, RenetReceive};
use std::f32::consts::TAU;
use std::{
    net::{SocketAddr, UdpSocket},
    time::SystemTime,
};

use bevy::input::common_conditions::input_toggle_active;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
// use smooth_bevy_cameras::{LookTransform, LookTransformBundle, LookTransformPlugin, Smoother};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_rapier3d::prelude::*;

#[derive(Component)]
struct Hovered;

#[derive(Component)]
struct AtmosphereSun;

pub fn run() {
    let mut app: App = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(LogPlugin {
                filter: "info,wgpu_core=warn,wgpu_hal=off,rechannel=warn".into(),
                level: bevy::log::Level::DEBUG,
                ..Default::default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(720, 720),
                    title: "Tribute Client".to_string(),
                    resizable: false,
                    ..default()
                }),
                ..default()
            }),
    )
    .init_state::<ClientState>()
    .add_loading_state(
        LoadingState::new(ClientState::Setup)
            .load_collection::<MyAssets>()
            .load_collection::<PigAssets>()
            .load_collection::<SealAssets>()
            .load_collection::<ChaskiAssets>()
            .load_collection::<SkyboxAssets>()
            .continue_to_state(ClientState::InMenu),
    )
    .add_plugins(EguiPlugin::default())
    .add_plugins(WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)))
    .add_plugins(crate::client::presentation::menu::MenuPlugin)
    .add_plugins(ObjPlugin)
    .add_plugins(PanOrbitCameraPlugin)
    // .add_plugins(LookTransformPlugin)
    //.add_plugins(DefaultPlugins)
    .add_plugins(RenetClientPlugin)
    .insert_resource(LocalPlayerInput::default())
    .insert_resource(ClientLobby::default())
    .insert_resource(CameraFacing::default())
    //.insert_resource(avian3d::prelude::SpatialQueryPipeline::default())
    /* .add_plugins((
        PhysicsPlugins::default(),
    ))*/
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
    //.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().with_default_system_setup(false))
    .add_plugins(RapierDebugRenderPlugin { ..default() })
    .insert_resource(Map::default())
    .insert_resource(NetworkMapping::default())
    .add_message::<PlayerCommand>()
    .add_plugins(NetcodeClientPlugin)
    .add_systems(
        Last,
        disconnect_on_window_close.before(close_when_requested),
    )
    .add_systems(OnEnter(ClientState::InGame), (setup_level, setup_camera))
    .add_plugins(Sprite3dPlugin)
    .add_plugins((
        InterpolationPlugin,
        // crate::client::network::clock_sync::ClientClockSyncPlugin,
        crate::client::presentation::action_bar::ActionBarPlugin,
        crate::client::presentation::animations::AnimationsPlugin,
        crate::client::presentation::casting::CastingPlugin,
        DamageNumbersPlugin,
        ClientClockSyncPlugin,
        // crate::client::presentation::music::MusicPlugin,
        crate::client::input::pointer::PointerPlugin,
        crate::client::presentation::health::HealthPlugin,
        crate::client::presentation::spells::SpellAnimationsPlugin,
        crate::client::presentation::water_material::WaterPlugin,
        // crate::client::presentation::water_experiment::WaterPlugin,
    ))
    .add_systems(
        Update,
        (
            client_send_input.run_if(in_state(ClientState::InGame)),
            client_send_player_commands
                .run_if(in_state(ClientState::InGame))
                .after(PredictionInputSet),
        ),
    )
    .add_systems(
        PreUpdate,
        client_sync_players
            .run_if(in_state(ClientState::InGame))
            .after(RenetReceive),
    )
    .add_systems(
        Update,
        (
            configure_atmosphere_sun.run_if(in_state(ClientState::InGame)),
            draw_player_sprites.run_if(in_state(ClientState::InGame)),
            camera_follow
                .run_if(in_state(ClientState::InGame))
                .after(InterpolationSet),
            // sprite_movement.run_if(in_state(ClientState::InGame)),
        ),
    );
    //.add_systems(FixedUpdate, (debug_current_gamemode_state));

    create_renet_transport(&mut app);

    app.run();
}

fn disconnect_on_window_close(
    close_requests: MessageReader<WindowCloseRequested>,
    mut transport: ResMut<NetcodeClientTransport>,
) {
    if !close_requests.is_empty() {
        info!("Window close requested; notifying the server before exiting");
        transport.disconnect();
    }
}

fn _debug_current_gamemode_state(state: Res<State<ClientState>>) {
    eprintln!("Current state: {:?}", state.get());
}

fn create_renet_transport(app: &mut App) {
    // create client
    let client = RenetClient::new(connection_config());
    app.insert_resource(client);

    let _ = dotenvy::dotenv();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    // Temporary development identity until login and character selection are
    // implemented. Keeping this stable lets the server reload the same database
    // character after a restart.
    let client_id = std::env::var("PLAYER_ACCOUNT_ID")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|id| *id > 0)
        .unwrap_or(1);
    info!("Using development player account {client_id}");

    let server_addr = SocketAddr::new(local_ip().unwrap(), 42069);

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();

    let transport: NetcodeClientTransport =
        NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

    app.insert_resource(transport);
    app.insert_resource(CurrentClientId(client_id));
}

fn client_send_input(player_input: Res<LocalPlayerInput>, mut client: ResMut<RenetClient>) {
    let input_message = bincode::serialize(&**player_input).unwrap();

    // info!("Sent input message {:?}!", input_message );
    client.send_message(ClientChannel::Input, input_message);
}

fn client_send_player_commands(
    mut player_commands: MessageReader<PlayerCommand>,
    mut client: ResMut<RenetClient>,
) {
    for command in player_commands.read() {
        let command_message = bincode::serialize(command).unwrap();

        info!("Sent command message {:?}!", command_message);
        client.send_message(ClientChannel::Command, command_message);
    }
}

fn draw_player_sprites(
    mut commands: Commands,
    mut entities: Query<(Entity, &Transform), Or<(Added<Player>, Added<ControlledPlayer>)>>,
    chaski: Res<ChaskiAssets>,
) {
    for (entity, transform) in entities.iter_mut() {
        let texture_atlas = TextureAtlas {
            layout: chaski.layout.clone(),
            index: 32,
        };
        let attack_texture_atlas = TextureAtlas {
            layout: chaski.attack_layout.clone(),
            index: 0,
        };

        /*let sprite_entity = commands.spawn(
        (
            Transform::from_xyz(0., -1.0, 0.),
            Sprite3dBuilder {
                image: chaski.sprite.clone(),
                pixels_per_metre: 48.,
                //pixels_per_metre: 128.,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                // transform: Transform::from_xyz(0., 0., 0.),
                //pivot: Some(Vec2::new(0.5, 0.5)),
                pivot: Some(Vec2::new(0.5, 0.)), // para que gire sobre los pies y no del centro.
                ..default()
            }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()),
            Name::new("PlayerSprite"),
            Billboard
        )).id();*/

        commands.entity(entity).with_children(|children| {
            children.spawn((
                Transform::from_xyz(0., -1.0, 0.),
                Sprite3d {
                    pixels_per_metre: 48.,
                    //pixels_per_metre: 128.,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    pivot: Some(Vec2::new(0.5, 0.0)),
                    // transform: Transform::from_xyz(0., 0., 0.),
                    //pivot: Some(Vec2::new(0.5, 0.5)),
                    ..default()
                },
                Sprite {
                    image: chaski.sprite.clone(),
                    texture_atlas: Some(texture_atlas.clone()),
                    ..default()
                },
                Visibility::Inherited,
                Name::new("PlayerWalkSprite"),
                Billboard,
                WalkingSpriteVisual,
            ));

            // Attack strips have a different aspect ratio than the walking sheet.
            // Keeping a separate Sprite3d lets the plugin build the correct atlas UVs.
            children.spawn((
                Transform::from_xyz(0., -1.0, 0.),
                Sprite3d {
                    pixels_per_metre: 48.,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    pivot: Some(Vec2::new(0.5, 0.0)),
                    ..default()
                },
                Sprite {
                    image: chaski.attack_side_down.clone(),
                    texture_atlas: Some(attack_texture_atlas.clone()),
                    ..default()
                },
                Visibility::Hidden,
                Name::new("PlayerAttackSprite"),
                Billboard,
                AttackSpriteVisual,
            ));
        });

        println!("Draw player sprite {:?}", transform);
    }
}

fn client_sync_players(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    client_id: Res<CurrentClientId>,
    mut lobby: ResMut<ClientLobby>,
    mut network_mapping: ResMut<NetworkMapping>,
    assets: Res<MyAssets>,
    chaski: Res<ChaskiAssets>,
    pig_assets: Res<PigAssets>,
    seal_assets: Res<SealAssets>,
    mut entities: Query<(
        &mut PositionHistory,
        &mut AuthoritativePosition,
        &mut Transform,
        &mut GameVelocity,
        Option<&mut PredictedMovement>,
        Option<&mut KinematicCharacterController>,
    )>,
) {
    let client_id = client_id.0;
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate {
                id,
                character_id,
                translation,
                facing,
                health,
                mana,
                entity,
                attack_speed,
                server_time,
            } => {
                println!("Player {} connected at translation  {:?}", id, translation);

                let texture_atlas = TextureAtlas {
                    layout: chaski.layout.clone(),
                    index: 0,
                };

                let mut client_entity = commands.spawn((
                    Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.5, 1.)))),
                    MeshMaterial3d(materials.add(Color::srgba(0.8, 0.7, 0.6, 0.0))),
                    Transform::from_xyz(translation[0], translation[1], translation[2]),
                    Name::new("Player"),
                    Collider::capsule_y(0.5, 0.5),
                    ActiveCollisionTypes::KINEMATIC_STATIC,
                    RigidBody::KinematicPositionBased,
                    health,
                    mana,
                    Animation::Idle,
                    AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
                    //RigidBody::Kinematic,
                    //Collider::capsule(0.4, 1.0),
                ));

                client_entity.insert((
                    Player { id },
                    character_id,
                    AttackSpeed(attack_speed),
                    GameVelocity::default(),
                    facing,
                    PositionHistory::new(translation.into(), server_time),
                    AuthoritativePosition::new(translation.into(), server_time),
                ));

                if client_id == id {
                    client_entity.insert(ControlledPlayer).insert((
                        BarSettings::<Health> {
                            offset: -1.12,
                            width: 1.05,
                            height: BarHeight::Static(0.06),
                            foreground_color: Some(Color::srgb(0.12, 0.82, 0.18)),
                            screen_anchor_offset: Some(-1.12),
                            ..default()
                        },
                        BarSettings::<Mana> {
                            offset: -1.22,
                            width: 1.05,
                            height: BarHeight::Static(0.06),
                            screen_anchor_offset: Some(-1.12),
                            screen_offset: Vec2::new(0.0, 4.0),
                            ..default()
                        },
                    ));
                    //.insert(Billboard)
                    //.insert(NotShadowCaster)

                    #[cfg(feature = "client_prediction")]
                    client_entity
                        .insert((PredictedMovement::default(), player_character_controller()));

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
                    if let Ok(_entity_exists) = commands.get_entity(client_entity) {
                        commands.entity(client_entity).despawn();
                        network_mapping.0.remove(&server_entity);
                    }
                }
            }
            ServerMessages::MovementRejected {
                entity,
                translation,
                server_time,
            } => {
                let Some(client_entity) = network_mapping.0.get(&entity) else {
                    continue;
                };
                let Ok((
                    mut history,
                    mut authoritative,
                    mut transform,
                    mut velocity,
                    prediction,
                    controller,
                )) = entities.get_mut(*client_entity)
                else {
                    continue;
                };

                let position = Vec3::from_array(translation);
                history.add_absolute_position(position, server_time);
                authoritative.position = position;
                authoritative.timestamp = server_time;
                transform.translation = position;
                velocity.0 = Vec3::ZERO;
                if let Some(mut prediction) = prediction {
                    prediction.destination = None;
                }
                if let Some(mut controller) = controller {
                    controller.translation = None;
                }
                commands.entity(*client_entity).insert(Animation::Idle);
                warn!("Server rejected movement; restored authoritative position");
            }
            ServerMessages::SpawnProjectile {
                entity,
                translation,
            } => {
                //let mut meshes = sprite_params.meshes.clone();

                let projectile_entity = commands.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.1)))),
                    MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                    Transform::from_translation(translation.into()),
                ));
                /*PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                }*/
                network_mapping.0.insert(entity, projectile_entity.id());
            }
            ServerMessages::DespawnProjectile { entity } => {
                if let Some(entity) = network_mapping.0.remove(&entity) {
                    commands.entity(entity).despawn();
                }
            }
            ServerMessages::SpellCastStarted {
                entity,
                spell_id,
                target,
                cast_time_ms,
                facing,
            } => {
                if let Some(client_entity) = network_mapping.0.get(&entity) {
                    commands.trigger(ConfirmedSpellCastStarted {
                        entity: *client_entity,
                        spell_id,
                        target,
                        cast_time: std::time::Duration::from_millis(cast_time_ms.into()),
                        facing,
                    });
                }
            }
            ServerMessages::SpellCastCompleted {
                entity,
                spell_id,
                target,
                cooldown_ms,
            } => {
                if let Some(client_entity) = network_mapping.0.get(&entity) {
                    commands.trigger(ConfirmedSpellCastCompleted {
                        entity: *client_entity,
                        spell_id,
                        target,
                        cooldown: std::time::Duration::from_millis(cooldown_ms.into()),
                    });
                }
            }
            ServerMessages::SpawnMonster {
                entity,
                kind,
                translation,
                server_time,
            } => {
                let texture_atlas: TextureAtlas = match kind {
                    MonsterKind::Pig => TextureAtlas {
                        layout: pig_assets.layout.clone(),
                        index: 0,
                    },
                    MonsterKind::Orc => TextureAtlas {
                        layout: pig_assets.layout.clone(),
                        index: 0,
                    },
                };

                let mut monster_entity = commands.spawn((
                    Sprite3d {
                        pixels_per_metre: 25.,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        //transform: Transform::from_translation(translation.into()),
                        // pivot: Some(Vec2::new(0.5, 0.5)),
                        ..default()
                    },
                    Sprite {
                        image: pig_assets.sprite.clone(),
                        texture_atlas: Some(texture_atlas.clone()),
                        ..default()
                    },
                    kind,
                    Name::new("Pig"),
                    Transform::from_translation(translation.into()),
                ));

                monster_entity
                    //.insert(Billboard)
                    .insert(GameVelocity::default())
                    .insert(PositionHistory::new(translation.into(), server_time))
                    .insert(AuthoritativePosition::new(translation.into(), server_time))
                    .insert(Facing(4));

                /*let monster_entity = commands.spawn(PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                });*/
                network_mapping.0.insert(entity, monster_entity.id());
            }
            ServerMessages::SpawnEntity {
                entity,
                sprite_id,
                translation,
                facing,
                health,
                server_time,
            } => {
                let texture_atlas: TextureAtlas = TextureAtlas {
                    layout: seal_assets.layout.clone(),
                    index: 58,
                };

                let mut client_entity = commands.spawn((
                    Sprite3d {
                        pixels_per_metre: 25.,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        // pivot: Some(Vec2::new(0.5, 0.5)),
                        ..default()
                    },
                    Sprite {
                        image: seal_assets.sprite.clone(),
                        texture_atlas: Some(texture_atlas.clone()),
                        ..default()
                    },
                    Transform::from_translation(translation.into()),
                    MonsterKind::Pig,
                    Collider::capsule_y(0.5, 0.5),
                    /*CollisionGroups::new(
                        Group::GROUP_1,
                        Group::GROUP_2,
                    ),*/
                    ActiveCollisionTypes::KINEMATIC_STATIC,
                    RigidBody::KinematicPositionBased,
                    /*Collider::capsule(0.4, 1.0),
                    RigidBody::Kinematic,   */
                    //Mass(5.0),
                    Monster {
                        hp: 100,
                        kind: MonsterKind::Pig,
                    },
                    Name::new("Monster"),
                ));

                println!("PIG SPAWNED AT  {:?} ", translation);

                println!("Client entity  {:?} ", client_entity.id());

                if let Some(health) = health {
                    client_entity.insert(health);
                }

                client_entity
                    //.insert(Billboard)
                    .insert(GameVelocity::default())
                    .insert(PositionHistory::new(translation.into(), server_time))
                    .insert(AuthoritativePosition::new(translation.into(), server_time))
                    .insert(Facing(0));

                /*let client_entity = commands.spawn(PbrBundle {
                    mesh: sprite_params.meshes.add(Mesh::from(Sphere::new(0.1))),
                    material: sprite_params.materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    transform: Transform::from_translation(translation.into()),
                    ..Default::default()
                });*/
                network_mapping.0.insert(entity, client_entity.id());
            }
            ServerMessages::DespawnEntity { entity } => {
                println!("Entity despawned {:?} ", entity);
                if let Some(client_entity) = network_mapping.0.remove(&entity) {
                    commands.entity(client_entity).try_despawn();
                    lobby
                        .players
                        .retain(|_, player_info| player_info.server_entity != entity);
                }
            }
            ServerMessages::HealthChange {
                entity,
                amount: _,
                max,
                current,
            } => {
                // println!("Cambio el HP {}, {} ", max, current);
                if let Some(client_entity) = network_mapping.0.get(&entity) {
                    commands
                        .entity(*client_entity)
                        .insert(Health { max, current });
                }
            }
            ServerMessages::DamageNumber { entity, amount } => {
                if let Some(client_entity) = network_mapping.0.get(&entity) {
                    commands.trigger(DamageNumberEvent {
                        entity: *client_entity,
                        amount,
                    });
                }
            }
            ServerMessages::Attack {
                entity,
                enemy,
                attack_speed,
                auto_attack,
            } => {
                println!(
                    "Entity  {:?} attacking  {:?} with  {:?}  aspd",
                    entity, enemy, attack_speed
                );
                if let (Some(client_entity), Some(client_enemy)) = (
                    network_mapping.0.get(&entity),
                    network_mapping.0.get(&enemy),
                ) {
                    commands
                        .entity(*client_entity)
                        .insert(Animation::Attacking {
                            entity: *client_entity,
                            enemy: *client_enemy,
                            attack_speed,
                            auto_attack,
                        });
                }
            }
            ServerMessages::AttackStopped { entity } => {
                if let Some(client_entity) = network_mapping.0.get(&entity) {
                    commands.entity(*client_entity).insert(Animation::Idle);
                }
            }
        }
    }

    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let Ok(snapshot) = bincode::deserialize::<EntitySnapshot>(&message) else {
            warn!("Ignoring malformed position snapshot");
            continue;
        };

        let Some(client_entity) = network_mapping.0.get(&snapshot.entity) else {
            continue;
        };
        let Ok((mut history, mut authoritative, _, _, _, _)) = entities.get_mut(*client_entity)
        else {
            continue;
        };

        let position = IVec3::new(snapshot.x, snapshot.y, snapshot.z)
            .as_vec3()
            .mul(TRANSLATION_PRECISION);
        if history.add_absolute_position(position, snapshot.server_time) {
            authoritative.position = position;
            authoritative.timestamp = snapshot.server_time;
        }
    }
}

fn setup_camera(mut commands: Commands, mut scattering_mediums: ResMut<Assets<ScatteringMedium>>) {
    let earth_medium = scattering_mediums.add(ScatteringMedium::earth(256, 256));
    commands.spawn(Atmosphere::earth(earth_medium));

    // The atmosphere-derived environment map supplies the ambient sky light.
    // Disable Bevy's flat global ambient light so shaded areas keep the color
    // and directionality of the sky instead of being uniformly gray.
    commands.insert_resource(GlobalAmbientLight::NONE);

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
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 25.5, 5.0)),
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
        AtmosphereSettings::default(),
        // Bevy 0.19's atmosphere is physically based and expects sunlight-scale
        // illuminance. This exposure keeps the much brighter sun/sky in range.
        Exposure { ev100: 13.0 },
        Tonemapping::AcesFitted,
        Bloom::NATURAL,
        AtmosphereEnvironmentMapLight {
            // The sky itself uses the atmosphere LUTs above. This lower-cost
            // cubemap is only used to tint ambient light and reflections.
            size: UVec2::splat(128),
            ..default()
        },
        DepthPrepass,
    ));
}

fn configure_atmosphere_sun(
    mut commands: Commands,
    mut lights: Query<(Entity, &mut DirectionalLight, &mut Transform), Without<AtmosphereSun>>,
) {
    for (entity, mut light, mut transform) in &mut lights {
        light.illuminance = lux::RAW_SUNLIGHT;
        *transform = Transform::from_xyz(1.0, 0.8, 0.4).looking_at(Vec3::ZERO, Vec3::Y);
        commands.entity(entity).insert(AtmosphereSun);
    }
}

fn camera_follow(
    mut camera_query: Query<&mut PanOrbitCamera, (With<Camera>, Without<ControlledPlayer>)>,
    player_query: Query<&Transform, (With<ControlledPlayer>, Changed<Transform>)>,
) {
    if let (Ok(player_transform), Ok(mut pan_cam)) =
        (player_query.single(), camera_query.single_mut())
    {
        //cam.look = Transform::from_xyz(0., 8.0, 2.5).looking_at(player_transform.translation.into(), Vec3::Y);
        pan_cam.target_focus = player_transform.translation;
        pan_cam.force_update = true;
        /*cam_transform.eye.x = player_transform.translation.x;
        cam_transform.eye.z = player_transform.translation.z + 15.5; // Con esto se mueve el angulo de la camara
        cam_transform.target = player_transform.translation;*/
    }
}

/*fn sprite_movement(
    time: Res<Time>,
    mut q_parent: Query<(&mut AnimationTimer, &mut Facing, &GameVelocity, &mut Animation)>,
    mut q_child: Query<(&ChildOf, &mut Sprite3d)>,
    camera_rotation: Res<CameraFacing>
) {


    for (parent, mut sprite) in q_child.iter_mut() {


        if let Ok ((mut timer, facing, velocity, animation)) = q_parent.get_mut(parent.get()) {


            //println!("Animation {:?}", animation);

            // Cuando se cambia la rotación, se debe ajustar el sprite.
            if camera_rotation.is_changed() {

                if let Some(atlas) = &mut sprite.texture_atlas {

                    let col_index = atlas.index  % 8;
                    println!("col_index {:?}", col_index);

                    let row_index = camera_rotation.0+facing.0;
                    println!("row_index {:?}", row_index);
                    atlas.index = col_index + (( row_index * 8) % 64) as usize;
                }

            }


            if velocity.0 == Vec3::ZERO {
                continue;
            }



            let x = (velocity.0.x * 1000.0).round() / 1000.0;
            let z = (velocity.0.z * 1000.0).round() / 1000.0;

            if z != 0. || x  != 0.0 {

                //let row_index = (8 * atlas.index / 64) % 8;

                timer.tick(time.delta());
                if timer.just_finished() {

                    let row_index = ((camera_rotation.0+facing.0) % 8) as usize;
                    //let col_index = atlas.index  % 8;

                    //println!("row_index {:?}",row_index);
                    let starting_row_animation = row_index*8;
                    //println!("starting_row_animation {:?}",starting_row_animation);
                    let a = (starting_row_animation)..(starting_row_animation + 7);

                    //println!("range {:?}, atlas.index {:?}",a ,atlas.index );
                    if let Some(atlas) = &mut sprite.texture_atlas {
                        atlas.index = if !a.contains(&atlas.index) || atlas.index == ((row_index*8)+7) {
                            starting_row_animation
                        }
                        else {
                            atlas.index + 1
                        };
                    }


                }

            }

        }
    }


}*/
