use bevy::{
    color::palettes::css::RED,
    light::ClusteredDecal,
    pbr::decal::{ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt},
    prelude::*,
    window::{CursorOptions, PrimaryWindow},
};
use bevy_asset_loader::prelude::*;

use crate::client::network::movement::{PositionHistory, PredictedMovement, PredictionInputSet};
use crate::client::presentation::action_bar::ActionBarState;
use crate::client::presentation::casting::{CastingSpell, RequestSpellCast};
use crate::client::state::*;
use crate::shared::constants::WATER_LEVEL;
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::entities::{Player, NPC};
use crate::shared::network::messages::*;
use crate::shared::states::ClientState;
use bevy::pbr::ExtendedMaterial;
use bevy_rapier3d::prelude::*;

// use avian3d::{parry::shape, prelude::*};

const CURSOR_DECAL_SIZE: f32 = 11.0;
const CURSOR_DECAL_DEPTH: f32 = 1.5;
const CURSOR_DECAL_SURFACE_OFFSET: f32 = 0.02;
const CURSOR_GRID_SIZE: f32 = 1.0;
const CURSOR_GRID_HEIGHT_PROBE_DISTANCE: f32 = 256.0;
const ENABLE_FORWARD_DECAL_EXPERIMENTS: bool = false;

#[derive(AssetCollection, Resource)]
struct GridTarget {
    #[asset(path = "grid-transparent.png")]
    sprite: Handle<Image>,
}

#[derive(Component)]
pub struct Target;

#[derive(Component, Default)]
struct CursorMapPosition(Vec3);

#[derive(Component)]
struct GameCursor {
    action: CursorKind,
    hovered_entity: Option<Entity>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum CursorKind {
    Default,
    Attack,
    Cast { spell_id: u16 },
}

fn snap_cursor_to_grid(point: Vec3) -> Vec3 {
    Vec3::new(
        (point.x / CURSOR_GRID_SIZE).round() * CURSOR_GRID_SIZE,
        point.y,
        (point.z / CURSOR_GRID_SIZE).round() * CURSOR_GRID_SIZE,
    )
}

fn cursor_decal_transform(surface_point: Vec3, surface_normal: Vec3) -> Transform {
    let normal = surface_normal.normalize_or(Vec3::Y);

    // Keep the decal's X axis aligned with the map as much as the surface
    // allows. On a near-vertical X-facing wall, Z is the safer reference.
    let reference_axis = if normal.dot(Vec3::X).abs() < 0.95 {
        Vec3::X
    } else {
        Vec3::Z
    };
    let tangent_x = (reference_axis - normal * reference_axis.dot(normal)).normalize_or(Vec3::X);
    let tangent_y = normal.cross(tangent_x).normalize_or(Vec3::Z);

    // Clustered decals use local X/Y for their UVs and local Z for projection
    // depth. Put almost all of that depth behind the hit surface, leaving only
    // a tiny tolerance in front so character and monster sprites are excluded.
    Transform {
        translation: surface_point
            - normal * (CURSOR_DECAL_DEPTH * 0.5 - CURSOR_DECAL_SURFACE_OFFSET),
        rotation: Quat::from_mat3(&Mat3::from_cols(tangent_x, tangent_y, normal)),
        scale: Vec3::new(CURSOR_DECAL_SIZE, CURSOR_DECAL_SIZE, CURSOR_DECAL_DEPTH),
    }
}

pub struct PointerPlugin;

impl Plugin for PointerPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.add_loading_state(
            LoadingState::new(ClientState::Setup).load_collection::<GridTarget>(),
        )
        //.add_plugins((DecalPlugin))
        .add_systems(OnEnter(ClientState::InGame), (setup_cursor))
        .add_systems(
            OnEnter(ClientState::InGame),
            (
                //setup_target,
                setup_target_decal
            ),
        )
        .add_systems(Update, draw_gizmos)
        .add_systems(
            Update,
            (
                move_cursor.run_if(in_state(ClientState::InGame)),
                player_input
                    .run_if(in_state(ClientState::InGame))
                    .in_set(PredictionInputSet),
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                //shape_cast.run_if(in_state(ClientState::InGame)),
                update_cursor_system_rapier3d.run_if(in_state(ClientState::InGame)),
                changed_cursor
                    .run_if(in_state(ClientState::InGame))
                    .after(setup_cursor),
            ),
        );

        fn calculate_initial_decal_transform(
            start: Vec3,
            looking_at: Vec3,
            size: Vec2,
        ) -> Transform {
            let direction = looking_at - start;
            let center = start + direction * 0.5;
            Transform::from_translation(center)
                .with_scale((size * 0.5).extend(direction.length()))
                .looking_to(direction, Vec3::Y)
        }

        /// Draws the outlines that show the bounds of the clustered decals.
        fn draw_gizmos(
            mut gizmos: Gizmos,
            decals: Query<(&GlobalTransform), With<ClusteredDecal>>,
        ) {
            for (global_transform) in &decals {
                gizmos.primitive_3d(
                    &Cuboid {
                        // Since the clustered decal is a 1×1×1 cube in model space, its
                        // half-size is half of the scaling part of its transform.
                        half_size: global_transform.scale() * 0.5,
                    },
                    Isometry3d {
                        rotation: global_transform.rotation(),
                        translation: global_transform.translation_vec3a(),
                    },
                    RED,
                );
            }
        }

        fn setup_target_decal(
            mut commands: Commands,
            mut meshes: ResMut<Assets<Mesh>>,
            mut materials: ResMut<Assets<StandardMaterial>>,
            mut decal_standard_materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>,
            asset_server: Res<AssetServer>,
        ) {
            commands.spawn((
                ClusteredDecal {
                    //image: asset_server.load("branding/icon.png"),
                    base_color_texture: Some(asset_server.load("grid_whitespace_big.png")),
                    // Tint with red.
                    tag: 1,
                    ..default()
                },
                Target,
                CursorMapPosition::default(),
                Name::new("Target"),
                //Transform::from_scale(Vec3::splat(11.0)),
                /*Transform {
                    translation: vec3(1.0,1.0,1.0),
                    rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                    scale: Vec3::splat(2.)
                }*/
                cursor_decal_transform(Vec3::ZERO, Vec3::Y),
            ));

            // These were useful while comparing Bevy's two decal paths, but
            // spawning them alongside the cursor adds two unrelated projectors
            // at the world origin.
            if ENABLE_FORWARD_DECAL_EXPERIMENTS {
                commands.spawn((
                    Name::new("Decal"),
                    ForwardDecal,
                    MeshMaterial3d(decal_standard_materials.add(ForwardDecalMaterial {
                        base: StandardMaterial {
                            base_color_texture: Some(
                                asset_server.load("textures/uv_checker_bw.png"),
                            ),
                            ..default()
                        },
                        extension: ForwardDecalMaterialExt {
                            depth_fade_factor: 10.0,
                        },
                    })),
                    //Transform::from_xyz(11.0, 1., 11.0),
                    Transform::from_scale(Vec3::splat(11.0)),
                ));

                commands.spawn((
                    /*DecalBundle {
                        transform: Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(11.0)),
                        decal_material: decal_materials.add(ExtendedMaterial::<StandardMaterial, DecalMaterial> {
                            base: StandardMaterial {
                                base_color_texture: Some(asset_server.load("grid_whitespace_big.png")),
                                //base_color_texture: Some(asset_server.load("blast.png")),
                                //base_color: Color::Srgba(Srgba::RED),
                                alpha_mode: AlphaMode::Blend,
                                ..default()
                            },
                            extension: DecalMaterial {
                                depth_fade_factor:0.0,
                            },
                        }),
                        mesh: meshes.add(decal_mesh_quad(Vec3::Y)),

                        ..default()
                    }*/
                    ForwardDecal,
                    MeshMaterial3d(decal_standard_materials.add(ForwardDecalMaterial {
                        base: StandardMaterial {
                            base_color_texture: Some(asset_server.load("grid_whitespace_big.png")),
                            alpha_mode: AlphaMode::Blend,
                            ..default()
                        },
                        extension: ForwardDecalMaterialExt {
                            depth_fade_factor: 10.0,
                        },
                        //mesh: meshes.add(decal_mesh_quad(Vec3::Y)),
                    })),
                    Transform::from_scale(Vec3::splat(11.0)),
                    //Target,
                    //Name::new("Target")
                ));
            }
        }

        fn setup_target(
            mut commands: Commands,
            assets: Res<GridTarget>,
            mut meshes: ResMut<Assets<Mesh>>,
            mut materials: ResMut<Assets<StandardMaterial>>,
        ) {
            let texture = assets.sprite.clone();
            commands.spawn((
                Mesh3d(meshes.add(Mesh::from(Cuboid::new(1., 0., 1.)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(texture),
                    //unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..Default::default()
                })),
                Transform::from_xyz(0.0, 1., 0.0),
                /*PbrBundle {
                    mesh: meshes.add(Mesh::from(Cuboid::new(1., 0., 1.))),
                    //material: materials.add(Color::srgb(1.0, 0.0, 0.0)),
                    //material: materials.add((texture, alpha_mode: )),
                    material:  materials.add(StandardMaterial {
                        base_color_texture: Some(texture),
                        //unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        ..Default::default()
                    }
                ),
                transform: Transform::from_xyz(0.0, 1., 0.0),
                ..Default::default()
                },*/
                Target,
                Name::new("Target old"),
            ));
        }

        fn shape_cast(
            primary_window_query: Query<&Window, With<PrimaryWindow>>,
            //rapier_context: Res<RapierContext>,
            read_rapier_context: ReadRapierContext,
            camera_query: Query<(&Camera, &GlobalTransform)>,
        ) {
            if let (Ok((camera, camera_transform)), Ok(rapier_context), Ok(primary_window)) = (
                camera_query.single(),
                read_rapier_context.single(),
                primary_window_query.single(),
            ) {
                if let Some(cursor_pos) = primary_window.cursor_position() {
                    if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let cam_transform = camera_transform.compute_transform();
                        let direction: Dir3 = ray.direction;

                        let shape = Collider::cuboid(1.0, 2.0, 1.0);
                        let shape_pos = cam_transform.translation;
                        let shape_rot = Quat::from_rotation_z(0.8);
                        let shape_vel = Vec3::new(0.0, 0.4, 0.0);
                        let filter = QueryFilter::default();
                        let options = ShapeCastOptions {
                            max_time_of_impact: 150.0,
                            target_distance: 0.0,
                            stop_at_penetration: false,
                            compute_impact_geometry_on_penetration: true,
                        };

                        let origin = Vec3::new(cursor_pos.x, 100.0, cursor_pos.y);
                        //let direction = Vec3::new(0.0, -1.0, 0.0).normalize(); // Move along the X-axis
                        let max_distance = 150.0; // Maximum travel distance

                        if let Some((entity, hit)) = rapier_context.cast_shape(
                            shape_pos,
                            Quat::IDENTITY,
                            direction.normalize(),
                            shape.raw.as_ref(),
                            options,
                            filter,
                        ) {
                            // The first collider hit has the entity `entity`. The `hit` is a
                            // structure containing details about the hit configuration.
                            /*println!(
                                "Hit the entity {:?} with the configuration: {:?}",
                                entity, hit
                            );*/
                        }
                    }
                }
            }
        }

        fn update_cursor_system_rapier3d(
            primary_window_query: Query<&Window, With<PrimaryWindow>>,
            mut target_query: Query<(&mut Transform, &mut CursorMapPosition), With<Target>>,
            camera_query: Query<(&Camera, &GlobalTransform)>,
            read_rapier_context: ReadRapierContext,
            //rapier_context: Res<RapierContext>,
            interactive_entities: Query<(Entity), (Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut cursor: Query<&mut GameCursor>,
        ) {
            if let (
                Ok((camera, camera_transform)),
                Ok(rapier_context),
                Ok((mut target_transform, mut target_position)),
                Ok(primary_window),
            ) = (
                camera_query.single(),
                read_rapier_context.single(),
                target_query.single_mut(),
                primary_window_query.single(),
            ) {
                if let Some(cursor_pos) = primary_window.cursor_position() {
                    if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let cam_transform = camera_transform.compute_transform();
                        let direction: Dir3 = ray.direction;

                        // This ray is only for positioning the map decal. Fixed
                        // colliders are terrain and map geometry; players and
                        // monsters use kinematic bodies and are excluded.
                        if let Some((_map_entity, map_hit)) = rapier_context
                            .cast_ray_and_get_normal(
                                cam_transform.translation,
                                direction.normalize(),
                                bevy_rapier3d::prelude::Real::MAX,
                                true,
                                QueryFilter::only_fixed().exclude_sensors(),
                            )
                        {
                            let mut snapped_point = snap_cursor_to_grid(map_hit.point);
                            let mut snapped_normal = map_hit.normal;

                            // Once X/Z have snapped to the cell center, probe
                            // vertically there. Reusing the freely-moving hit's
                            // height would make the decal float or sink on slopes.
                            let probe_origin =
                                snapped_point + Vec3::Y * (CURSOR_GRID_HEIGHT_PROBE_DISTANCE * 0.5);
                            if let Some((_map_entity, snapped_hit)) = rapier_context
                                .cast_ray_and_get_normal(
                                    probe_origin,
                                    Vec3::NEG_Y,
                                    CURSOR_GRID_HEIGHT_PROBE_DISTANCE,
                                    true,
                                    QueryFilter::only_fixed().exclude_sensors(),
                                )
                            {
                                snapped_point.y = snapped_hit.point.y;
                                snapped_normal = snapped_hit.normal;
                            }

                            target_position.0 = snapped_point;
                            *target_transform =
                                cursor_decal_transform(snapped_point, snapped_normal);
                        }

                        // Keep the unfiltered ray separate: it still needs to
                        // see characters and monsters for the attack cursor.
                        if let Some((entity, time_of_impact)) = rapier_context.cast_ray(
                            cam_transform.translation,
                            direction.normalize(),
                            bevy_rapier3d::prelude::Real::MAX,
                            true,
                            QueryFilter::default(),
                        ) {
                            if let Ok(mut game_cursor) = cursor.single_mut() {
                                if matches!(game_cursor.action, CursorKind::Cast { .. }) {
                                    if game_cursor.hovered_entity.is_some() {
                                        game_cursor.hovered_entity = None;
                                    }
                                } else if let Ok((interactive_entity)) =
                                    interactive_entities.get(entity)
                                {
                                    if (Some(interactive_entity) != game_cursor.hovered_entity) {
                                        game_cursor.hovered_entity = Some(interactive_entity);
                                    }

                                    if (game_cursor.action != CursorKind::Attack) {
                                        game_cursor.action = CursorKind::Attack;
                                    }
                                } else {
                                    // println!("No le dimos a nada.Frist hit {:?}", first_hit.entity);
                                    if (game_cursor.hovered_entity != None) {
                                        game_cursor.hovered_entity = None;
                                    }

                                    if (game_cursor.action != CursorKind::Default) {
                                        game_cursor.action = CursorKind::Default;
                                    }
                                }
                            }

                            //println!("First hit: {:?}", first_hit);
                            /*println!(
                                "Hit entity {:?} at {} with normal {}",
                                first_hit.entity,
                                ray.origin + *ray.direction * first_hit.time_of_impact,
                                first_hit.normal,
                            );*/

                            /*let mut translation = ray.origin + *ray.direction * time_of_impact;
                            translation.x = translation.x.round();
                            translation.z = translation.z.round();
                            //translation.y =  translation.y + 0.15;
                            translation.y = translation.y ;
                            target_transform.translation = translation;*/

                            let _ = time_of_impact;
                        }
                    }
                }
            }
        }

        /*fn update_cursor_system_avian3d(
            primary_window: Query<&Window, With<PrimaryWindow>>,
            mut target_query: Query<&mut Transform, With<Target>>,
            camera_query: Query<(&Camera, &GlobalTransform)>,
            spatial_query: SpatialQuery,
            interactive_entities: Query<(Entity), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut cursor: Query<&mut GameCursor>,
        ) {
            let (camera,camera_transform) = camera_query.single();

            let mut target_transform = target_query.single_mut();
            if let Some(cursor_pos) = primary_window.single().cursor_position() {

                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {

                    let cam_transform = camera_transform.compute_transform();
                    let direction: Dir3 = ray.direction;


                    if let Some(first_hit) = spatial_query.cast_ray(
                        cam_transform.translation,                    // Origin
                        direction,                       // Direction
                        f32::MAX,                            // Maximum time of impact (travel distance)
                        true,                          // Does the ray treat colliders as "solid"
                        SpatialQueryFilter::default(), // Query filter
                    ) {

                        let mut game_cursor: Mut<'_, GameCursor> = cursor.single_mut();

                        if let Ok((interactive_entity)) = interactive_entities.get(first_hit.entity) {

                            if(Some(interactive_entity) != game_cursor.hovered_entity) {
                                game_cursor.hovered_entity = Some(interactive_entity);
                            }

                            if(game_cursor.action != CursorKind::Attack) {
                                game_cursor.action = CursorKind::Attack;
                            }
                        }
                        else {
                           // println!("No le dimos a nada.Frist hit {:?}", first_hit.entity);
                            if(game_cursor.hovered_entity != None) {
                                game_cursor.hovered_entity = None;
                            }

                            if(game_cursor.action != CursorKind::Default) {
                                game_cursor.action = CursorKind::Default;
                            }
                        }

                        //println!("First hit: {:?}", first_hit);
                        /*println!(
                            "Hit entity {:?} at {} with normal {}",
                            first_hit.entity,
                            ray.origin + *ray.direction * first_hit.time_of_impact,
                            first_hit.normal,
                        );*/

                        let mut translation = ray.origin + *ray.direction * first_hit.time_of_impact;
                        translation.x = translation.x.round();
                        translation.z = translation.z.round();
                        translation.y =  translation.y + 0.15;
                        target_transform.translation = translation;


                    }

                    /*let mut hits = vec![];

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
                    }*/



                    /*if let Some(distance) = ray.intersect_plane(Vec3::Y, InfinitePlane3d::new(Vec3::Y)) {
                        //info!("Ray {:?}!", ray.direction * distance + ray.origin );
                        let mut translation = ray.direction * distance + ray.origin;
                        translation.x = translation.x.round();
                        translation.z = translation.z.round();
                        target_transform.translation = translation;
                    }*/
                }
            }
        }*/

        fn changed_cursor(
            mut cursors: Query<
                (&GameCursor, &mut ImageNode),
                (With<GameCursor>, Changed<GameCursor>),
            >,
            asset_server: Res<AssetServer>,
        ) {
            //let game_cursor = cursor.single_mut();

            if let Ok((cursor, mut img)) = cursors.single_mut() {
                match cursor.action {
                    CursorKind::Default => {
                        img.image = asset_server.load("cursors/PNG/01.png").into()
                    }
                    CursorKind::Attack => {
                        img.image = asset_server.load("cursors/PNG/05.png").into()
                    }
                    CursorKind::Cast { spell_id } => {
                        let cursor_path = match spell_id {
                            1 => "cursors/PNG/11.png",
                            2 => "cursors/PNG/14.png",
                            3 => "cursors/PNG/15.png",
                            _ => "cursors/PNG/11.png",
                        };
                        img.image = asset_server.load(cursor_path).into();
                    }
                }
            }
            /*let (mut img, game_cursor) = cursor.single_mut();
             */
        }

        fn setup_cursor(
            mut windows: Query<(&mut Window, &mut CursorOptions)>,
            mut commands: Commands,
            asset_server: Res<AssetServer>,
        ) {
            if let Ok((_window, mut cursor_options)) = windows.single_mut() {
                cursor_options.visible = false;
                let cursor_spawn: Vec3 = Vec3::ZERO;

                commands.spawn((
                    ImageNode {
                        image: asset_server.load("cursors/PNG/01.png").into(),
                        ..default()
                    },
                    Node {
                        height: Val::Px(32.),
                        width: Val::Px(32.),
                        position_type: PositionType::Absolute,

                        ..default()
                    },
                    /*ImageBundle {
                        image: asset_server.load("cursors/PNG/01.png").into(),
                        style: Style {
                            //display: Display::None,
                            height: Val::Px(32.),
                            width: Val::Px(32.),
                            position_type: PositionType::Absolute,
                            //position: UiRect::all(Val::Auto),
                            ..default()
                        },
                        z_index: ZIndex::Global(15),
                        transform: Transform::from_translation(cursor_spawn),
                        ..default()
                    },*/
                    GameCursor {
                        action: CursorKind::Default,
                        hovered_entity: None,
                    },
                    GlobalZIndex(1000),
                    Pickable::IGNORE,
                ));
            }
        }

        fn move_cursor(
            primary_window: Query<&Window, With<PrimaryWindow>>,
            mut cursor: Query<&mut Node, With<GameCursor>>,
        ) {
            if let (Ok(window), Ok(mut cursor)) = (primary_window.single(), cursor.single_mut()) {
                if let Some(position) = window.cursor_position() {
                    cursor.left = Val::Px(position.x);
                    cursor.top = Val::Px(position.y);
                }
            }
        }

        fn player_input(
            mut commands: Commands,
            keyboard_input: Res<ButtonInput<KeyCode>>,
            mut player_input: ResMut<LocalPlayerInput>,
            mouse_button_input: Res<ButtonInput<MouseButton>>,
            primary_window: Query<&Window, With<PrimaryWindow>>,
            action_bar: Option<Res<ActionBarState>>,
            target_query: Query<&CursorMapPosition, With<Target>>,
            mut player_commands: MessageWriter<PlayerCommand>,
            mut player_entities: Query<
                (
                    Entity,
                    Option<&mut PredictedMovement>,
                    &PositionHistory,
                    &mut Animation,
                ),
                With<ControlledPlayer>,
            >,
            mut cursors: Query<&mut GameCursor>,
            mut network_mapping: ResMut<NetworkMapping>,
            casting_players: Query<(), (With<ControlledPlayer>, With<CastingSpell>)>,
            //interactive_entities: Query<(Entity), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
        ) {
            let movement_locked = !casting_players.is_empty();
            if movement_locked {
                **player_input = PlayerInput::default();
            } else {
                player_input.left = keyboard_input.pressed(KeyCode::KeyA)
                    || keyboard_input.pressed(KeyCode::ArrowLeft);
                player_input.right = keyboard_input.pressed(KeyCode::KeyD)
                    || keyboard_input.pressed(KeyCode::ArrowRight);
                player_input.up = keyboard_input.pressed(KeyCode::KeyW)
                    || keyboard_input.pressed(KeyCode::ArrowUp);
                player_input.down = keyboard_input.pressed(KeyCode::KeyS)
                    || keyboard_input.pressed(KeyCode::ArrowDown);
            }

            let selected_spell = if keyboard_input.just_pressed(KeyCode::F1) {
                Some(1)
            } else if keyboard_input.just_pressed(KeyCode::F2) {
                Some(2)
            } else if keyboard_input.just_pressed(KeyCode::F3) {
                Some(3)
            } else {
                None
            };

            if let Some(spell_id) = selected_spell {
                if let Ok(mut cursor) = cursors.single_mut() {
                    cursor.action = CursorKind::Cast { spell_id };
                    cursor.hovered_entity = None;
                }
            }

            if mouse_button_input.just_pressed(MouseButton::Left) {
                let pointer_over_action_bar = primary_window
                    .single()
                    .ok()
                    .and_then(Window::cursor_position)
                    .is_some_and(|pointer_position| {
                        action_bar
                            .as_deref()
                            .is_some_and(|bar| bar.captures_pointer(pointer_position))
                    });
                if pointer_over_action_bar {
                    return;
                }
                if movement_locked {
                    return;
                }

                if let Ok(mut cursor) = cursors.single_mut() {
                    match cursor.action {
                        CursorKind::Default => {
                            if let (
                                Ok(target_position),
                                Ok((player_entity, prediction, history, mut animation)),
                            ) = (target_query.single(), player_entities.single_mut())
                            {
                                if target_position.0.y < WATER_LEVEL {
                                    info!("Ignoring movement into submerged terrain");
                                    return;
                                }

                                let mut move_translation = target_position.0;
                                move_translation.x = move_translation.x.round();
                                move_translation.z = move_translation.z.round();

                                player_input.destination_at =
                                    Some(Pos(move_translation.x as i32, move_translation.z as i32));
                                *animation = Animation::Walking;

                                info!("Hay un player entity: {:?}!", player_entity);

                                #[cfg(feature = "client_prediction")]
                                if let Some(mut prediction) = prediction {
                                    prediction.start(
                                        move_translation,
                                        history
                                            .latest()
                                            .map(|snapshot| snapshot.timestamp)
                                            .unwrap_or_default(),
                                    );
                                }

                                #[cfg(not(feature = "client_prediction"))]
                                let _ = (prediction, history);

                                /*commands.entity(*player_entity).insert(PlayerCommand::Move {
                                    destination_at: move_translation,
                                });*/
                                player_commands.write(PlayerCommand::Move {
                                    destination_at: move_translation,
                                });
                            }
                        }
                        CursorKind::Attack => {
                            info!("Attack: {:?}!", cursor.hovered_entity);
                            if let Some(hovered_entity) = cursor.hovered_entity {
                                info!("Hay un hovered entity: {:?}!", hovered_entity);
                                let server_entity =
                                    network_mapping.0.iter().find_map(|(key, &val)| {
                                        if val == hovered_entity {
                                            Some(key)
                                        } else {
                                            None
                                        }
                                    });

                                info!("server entity: {:?}!", server_entity);
                                if let Some((server_entity)) = server_entity {
                                    player_commands.write(PlayerCommand::BasicAttack {
                                        entity: *server_entity,
                                    });
                                }
                            }
                        }
                        CursorKind::Cast { spell_id } => {
                            if let Ok(target_position) = target_query.single() {
                                commands.trigger(RequestSpellCast {
                                    spell_id,
                                    translation: target_position.0,
                                });
                                cursor.action = CursorKind::Default;
                                cursor.hovered_entity = None;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_decal_projects_from_behind_the_surface() {
        let surface_point = Vec3::new(3.0, 7.0, -2.0);
        let surface_normal = Vec3::new(0.25, 1.0, -0.4).normalize();
        let transform = cursor_decal_transform(surface_point, surface_normal);
        let decal_normal = transform.rotation * Vec3::Z;
        let front_face = transform.translation + decal_normal * (transform.scale.z * 0.5);

        assert!((decal_normal - surface_normal).length() < 1e-5);
        assert!(
            (front_face - (surface_point + surface_normal * CURSOR_DECAL_SURFACE_OFFSET)).length()
                < 1e-5
        );
    }

    #[test]
    fn cursor_decal_preserves_its_surface_size() {
        let transform = cursor_decal_transform(Vec3::ZERO, Vec3::Y);

        assert_eq!(
            transform.scale,
            Vec3::new(CURSOR_DECAL_SIZE, CURSOR_DECAL_SIZE, CURSOR_DECAL_DEPTH)
        );
    }

    #[test]
    fn cursor_position_snaps_to_one_unit_grid() {
        let snapped = snap_cursor_to_grid(Vec3::new(2.49, 7.25, -3.51));

        assert_eq!(snapped, Vec3::new(2.0, 7.25, -4.0));
    }
}
