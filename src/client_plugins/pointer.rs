use avian3d::math::Scalar;
use bevy::{pbr::{NotShadowCaster, NotShadowReceiver}, prelude::*, window::PrimaryWindow};
use bevy_asset_loader::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use client_plugins::shared::*;
use bevy_contact_projective_decals::{decal_mesh_quad, DecalBundle, DecalMaterial, DecalPlugin};
use bevy::pbr::ExtendedMaterial;
use crate::*;
// use avian3d::{parry::shape, prelude::*};

#[derive(AssetCollection, Resource)]
struct GridTarget {
    #[asset(path = "grid-transparent.png")]
    sprite: Handle<Image>,
}

#[derive(Component)]
pub struct Target;


#[derive(Component)]
struct GameCursor 
{
    action: CursorKind,
    hovered_entity: Option<Entity>
}

#[derive(PartialEq, Debug)]
enum CursorKind {
    Default,
    Attack,
    Cast
}


pub struct PointerPlugin;

impl Plugin for PointerPlugin {

    fn build(&self, app: &mut App) {
        // add things to your app here
        app          
            .add_loading_state(
                LoadingState::new(AppState::Setup)
                    .continue_to_state(AppState::InGame)
                    .load_collection::<GridTarget>()
            )
            .add_plugins((DecalPlugin))
            .add_systems(OnEnter(AppState::Setup), ((setup_cursor)))
            .add_systems(OnEnter(AppState::InGame), ((setup_target, setup_target_decal)))
            .add_systems(Update, (  
                    move_cursor.run_if(in_state(AppState::InGame)),
                    player_input.run_if(in_state(AppState::InGame)),        
                )
            )           
            .add_systems(FixedUpdate, (       
                    shape_cast.run_if(in_state(AppState::InGame)),
                    update_cursor_system_rapier3d.run_if(in_state(AppState::InGame)),
                    changed_cursor.run_if(in_state(AppState::InGame)).after(setup_cursor),
                )
            );
                


        fn setup_target_decal(
            mut commands: Commands,
            mut meshes: ResMut<Assets<Mesh>>,
            mut materials: ResMut<Assets<StandardMaterial>>,
            mut decal_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, DecalMaterial>>>,
            asset_server: Res<AssetServer>,
        ) {
            commands.spawn(DecalBundle {
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
            })
            .insert(Target)
            .insert(NotShadowCaster)
            .insert(NotShadowReceiver)
            .insert(Name::new("Target"));
        }
        
        fn setup_target(mut commands: Commands,
            assets            : Res<GridTarget>,
            mut meshes: ResMut<Assets<Mesh>>, 
            mut materials: ResMut<Assets<StandardMaterial>>) {

            let texture = assets.sprite.clone();   
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
                Name::new("Target old")))
                ;

        
        }

        fn shape_cast(
            primary_window: Query<&Window, With<PrimaryWindow>>,
            rapier_context: Res<RapierContext>,
            camera_query: Query<(&Camera, &GlobalTransform)>,
        ) {
            let (camera,camera_transform) = camera_query.single();

            if let Some(cursor_pos) = primary_window.single().cursor_position() {      

                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {

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

                    if let Some((entity, hit)) =
                        rapier_context.cast_shape(shape_pos,  Quat::IDENTITY, direction.normalize(), &shape, options, filter)
                    {
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
        fn update_cursor_system_rapier3d(
            primary_window: Query<&Window, With<PrimaryWindow>>,
            mut target_query: Query<&mut Transform, With<Target>>,
            camera_query: Query<(&Camera,  &GlobalTransform)>,
            rapier_context: Res<RapierContext>,
            interactive_entities: Query<(Entity), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut cursor: Query<&mut GameCursor>,
        ) {
            let (camera, camera_transform) = camera_query.single();
            
            let mut target_transform = target_query.single_mut();
            if let Some(cursor_pos) = primary_window.single().cursor_position() {

      
                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {

                    let cam_transform = camera_transform.compute_transform();
                    let direction: Dir3 = ray.direction;

                    if let Some((entity, time_of_impact)) = rapier_context.cast_ray(cam_transform.translation, direction.normalize(), bevy_rapier3d::prelude::Real::MAX, true, QueryFilter::default()) {
                        // The first collider hit has the entity `entity` and it hit after
                        // the ray travelled a distance equal to `ray_dir * time_of_impact`.
                        let hit_point = cam_transform.translation + direction.normalize() * time_of_impact;
                        // println!("Entity {:?} hit at point {}", entity, hit_point);

                        let shape = Collider::cuboid(1.0, 2.0, 1.0);
                        let origin = Vec3::new(hit_point.x, -10.0, hit_point.z);
                        let direction = Vec3::new(0.0, 1.0, 0.0).normalize(); // Move along the Y-axis upwards
                        let filter = QueryFilter::default();
                        let options = ShapeCastOptions {
                            max_time_of_impact: 150.0,
                            target_distance: 0.0,
                            stop_at_penetration: true,
                            compute_impact_geometry_on_penetration: false,
                        };
                        if let Some((entity, hit)) =
                        rapier_context.cast_shape(origin,  Quat::IDENTITY, direction.normalize(), &shape, options, filter)
                        {
                            // The first collider hit has the entity `entity`. The `hit` is a
                            // structure containing details about the hit configuration.
                            

                            if let Some( details) = hit.details {
                                let mut translation = ray.origin + *ray.direction * time_of_impact;
                                translation.x = translation.x.round();
                                translation.z = translation.z.round();
                                //translation.y =  translation.y + 0.15; 
                                translation.y = details.witness1.y.round();
                                target_transform.translation = translation;
                            }
                            
                            /*println!(
                                "Hit the entity {:?} with the configuration: {:?}",
                                entity, hit
                            );*/
                  

                        }

                        let mut game_cursor: Mut<'_, GameCursor> = cursor.single_mut();
                      
                        if let Ok((interactive_entity)) = interactive_entities.get(entity) {                            
                      
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

                        /*let mut translation = ray.origin + *ray.direction * time_of_impact;
                        translation.x = translation.x.round();
                        translation.z = translation.z.round();
                        //translation.y =  translation.y + 0.15; 
                        translation.y = translation.y ;
                        target_transform.translation = translation;*/
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
                        Scalar::MAX,                         // Maximum time of impact (travel distance)
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
            mut cursors: Query<(&GameCursor, &mut UiImage), (With<GameCursor>,Changed<GameCursor>)>,
            asset_server: Res<AssetServer>,
        ) {
            //let game_cursor = cursor.get_single_mut();

            if let Ok((cursor, mut img)) =  cursors.get_single_mut() {                
                match cursor.action {
                    CursorKind::Default => img.texture = asset_server.load("cursors/PNG/01.png").into(),
                    CursorKind::Attack => img.texture = asset_server.load("cursors/PNG/05.png").into(),
                    CursorKind::Cast => img.texture = asset_server.load("cursors/PNG/05.png").into(),     
                }
            }
            /*let (mut img, game_cursor) = cursor.single_mut();
          */

        } 



        
        fn setup_cursor(
            mut windows: Query<&mut Window>,
            mut commands: Commands,
            asset_server: Res<AssetServer>,
        ) {
            let mut window: Mut<Window> = windows.single_mut();
            window.cursor.visible = false;
            let cursor_spawn: Vec3 = Vec3::ZERO;

            commands.spawn((
                ImageBundle {
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
                },
                GameCursor {
                    action: CursorKind::Default,
                    hovered_entity: None
                }
            ));
        }

        fn move_cursor(
            primary_window: Query<&Window, With<PrimaryWindow>>,
            mut cursor: Query<&mut Style, With<GameCursor>>) {

            if let Some(position) = primary_window.single().cursor_position() {
                let mut img_style = cursor.single_mut();
                img_style.left = Val::Px(position.x);
                img_style.top = Val::Px(position.y);
            }
        }

        
        fn player_input(
            keyboard_input: Res<ButtonInput<KeyCode>>,
            mut player_input: ResMut<PlayerInput>,
            mouse_button_input: Res<ButtonInput<MouseButton>>,
            target_query: Query<&Transform, With<Target>>,
            mut player_commands: EventWriter<PlayerCommand>,
            mut commands: Commands,
            player_entities: Query<Entity, With<ControlledPlayer>>,
            mut cursors: Query<&GameCursor>,
            mut network_mapping: ResMut<NetworkMapping>,
            //interactive_entities: Query<(Entity), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,

        ) {
            player_input.left = keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
            player_input.right = keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
            player_input.up = keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
            player_input.down = keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);

            if mouse_button_input.just_pressed(MouseButton::Left) {

                if let Ok((cursor)) =  cursors.get_single_mut() {
                  
                    match cursor.action {
                        CursorKind::Default => {

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
                        },
                        CursorKind::Attack => {
                            info!("Attack: {:?}!", cursor.hovered_entity );
                            if let Some(hovered_entity) = cursor.hovered_entity {

                                info!("Hay un hovered entity: {:?}!", hovered_entity );
                                let server_entity = network_mapping.0.iter()
                                .find_map(|(key, &val)| if val == hovered_entity { Some(key) } else { None });

                                info!("server entity: {:?}!", server_entity );
                                if let Some((server_entity)) = server_entity {

                                    player_commands.send(PlayerCommand::BasicAttack {
                                        entity: *server_entity,
                                    });
                                  

                                }

                               
                            
                            }                         
                          
                        },
                        CursorKind::Cast => {
                            let target_transform = target_query.single();

                            player_commands.send(PlayerCommand::Cast {
                                cast_at: target_transform.translation,
                            });
                        },     
                    }
                }


               
            }
        }


    }
}