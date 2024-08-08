
use bevy_sprite3d::*;
use local_ip_address::local_ip;
use roguelike::*;
use bevy::{asset::LoadState, input::mouse::MouseWheel, log::LogPlugin, pbr::NotShadowCaster, prelude::*, render::render_resource::Texture, window::{PrimaryWindow, Window, WindowResolution}};
pub use bevy_renet::renet::transport::ClientAuthentication;
use bevy_renet::{renet::*, transport::NetcodeClientPlugin};
use bevy_renet::*;
use bevy_renet::renet::transport::NetcodeClientTransport;
use std::{
    collections::HashMap, net::{SocketAddr, UdpSocket}, time::SystemTime
};
use bevy_asset_loader::prelude::*;

use bevy_inspector_egui::prelude::ReflectInspectorOptions;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_inspector_egui::InspectorOptions;
use bevy::input::common_conditions::input_toggle_active;
use smooth_bevy_cameras::{LookTransform, LookTransformBundle, LookTransformPlugin, Smoother};


// 0.14 (Solution 1)
#[derive(States, Default, Hash, Debug, PartialEq, Clone, Eq)]
enum AppState {
    // Make this the default instead of `InMenu`.
    #[default]
    Setup,
    _InMenu,
    InGame,
}

#[derive(Component)]
struct ControlledPlayer;


#[derive(Default, Resource)]
struct NetworkMapping(HashMap<Entity, Entity>);

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
                .load_collection::<GridTarget>(),
        )
        .add_plugins(  
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
        )
        .add_plugins(LookTransformPlugin)
        //.add_plugins(DefaultPlugins)
        .add_plugins(RenetClientPlugin)
        .insert_resource(PlayerInput::default())
        .insert_resource(ClientLobby::default())
        .insert_resource(NetworkMapping::default())
        .add_event::<PlayerCommand>()
        .add_plugins(NetcodeClientPlugin)   
        .add_systems(Update, client_ping)
        .add_systems(Startup, (setup_level,setup_camera))
        .add_plugins(Sprite3dPlugin)

        .add_systems(OnEnter(AppState::InGame), ((setup_player, setup_target)))
        .add_systems(Update, 
            (
                update_target_system.run_if(in_state(AppState::InGame)),
                player_input.run_if(in_state(AppState::InGame)),
                camera_follow.run_if(in_state(AppState::InGame)),
                camera_zoom.run_if(in_state(AppState::InGame)),
                client_send_input.run_if(in_state(AppState::InGame)),
              
                client_send_player_commands.run_if(in_state(AppState::InGame)),

            
            )
        )  
        .add_systems(
            FixedUpdate, (
                client_sync_players.run_if(in_state(AppState::InGame))
             
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

fn client_ping(mut client: ResMut<RenetClient>, keyboard: Res<ButtonInput<KeyCode>>) {

    if keyboard.just_pressed(KeyCode::Space) {
        let ping_message = bincode::serialize(&ClientMessage::Ping).unwrap();

        client.send_message(ClientChannel::Ping, ping_message);
        //client.send_message(reliable_channel_id, ping_message);
        info!("Sent ping!");
    }


    while let Some(message) = client.receive_message(ClientChannel::Ping) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessage::Pong => {
                info!("Got pong!");
            }
        }
    }


}

 
fn player_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    target_query: Query<&Transform, With<Target>>,
    mut player_commands: EventWriter<PlayerCommand>,
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

        player_commands.send(PlayerCommand::Move {
            destination_at: move_translation,
        });
        player_commands.send(PlayerCommand::BasicAttack {
            cast_at: target_transform.translation,
        });
    }
}

fn client_send_input(player_input: Res<PlayerInput>, mut client: ResMut<RenetClient>) {
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
    mut sprite_params : Sprite3dParams,
) {
    let client_id = client_id.0;
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate { id, translation, entity } => {
                println!("Player {} connected.", id);

     

                let texture_atlas = TextureAtlas {
                    layout: assets.layout.clone(),
                    index: 3,
                };
                
                let mut client_entity = commands.spawn((Sprite3d {
                    image: assets.sprite.clone(),
                    pixels_per_metre: 10.,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    transform: Transform::from_xyz(translation[0], translation[1]+1.0, translation[2]),
                    // transform: Transform::from_xyz(0., 0., 0.),
                    // pivot: Some(Vec2::new(0.5, 0.5)),

                    ..default()
                }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()), Name::new("Player")));
                
                

                //client_entity1.insert(ControlledPlayer)
                /*let mut client_entity = commands.spawn(PbrBundle {
                    mesh: meshes.add(Mesh::from(Capsule3d::default())),
                    material: materials.add(Color::srgb(0.8, 0.7, 0.6)),
                    transform: Transform::from_xyz(translation[0], translation[1], translation[2]),
                    ..Default::default()
                });*/

                if client_id == id.raw() {
                    client_entity.insert(ControlledPlayer).insert(AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)));;
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
        }
    }

    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let networked_entities: NetworkedEntities = bincode::deserialize(&message).unwrap();

        for i in 0..networked_entities.entities.len() {
            if let Some(entity) = network_mapping.0.get(&networked_entities.entities[i]) {
                let mut translation: Vec3 = networked_entities.translations[i].into();
                translation.y = 2.0;
                let transform = Transform {
                    translation,
                    ..Default::default()
                };
                commands.entity(*entity).insert(transform);
            }
        }
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
    commands
        .spawn(LookTransformBundle {
            transform: LookTransform {
                eye: Vec3::new(0.0, 20., 2.5),
                target: Vec3::new(0.0, 0.5, 0.0),
                up: Vec3::Y,
            },
            smoother: Smoother::new(0.9),
        })
        .insert(Camera3dBundle {
            transform: Transform::from_xyz(0., 20.0, 2.5).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
            ..default()
        });
}


fn camera_follow(
    mut camera_query: Query<&mut LookTransform, (With<Camera>, Without<ControlledPlayer>)>,
    player_query: Query<&Transform, With<ControlledPlayer>>,
) {
    let mut cam_transform = camera_query.single_mut();
    if let Ok(player_transform) = player_query.get_single() {
        cam_transform.eye.x = player_transform.translation.x;
        cam_transform.eye.z = player_transform.translation.z + 2.5;
        cam_transform.target = player_transform.translation;
    }
}



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
}

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


fn update_target_system(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut target_query: Query<&mut Transform, With<Target>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = camera_query.single();
    let mut target_transform = target_query.single_mut();
    if let Some(cursor_pos) = primary_window.single().cursor_position() {
        if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
            if let Some(distance) = ray.intersect_plane(Vec3::Y, InfinitePlane3d::new(Vec3::Y)) {
                //info!("Ray {:?}!", ray.direction * distance + ray.origin );
                let mut translation = ray.direction * distance + ray.origin;
                translation.x = translation.x.round();
                translation.z = translation.z.round();
                target_transform.translation = translation;
            }
        }
    }
}
