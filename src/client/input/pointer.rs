use bevy::ecs::system::SystemParam;
use bevy::{
    color::palettes::css::RED,
    light::ClusteredDecal,
    pbr::decal::{ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt},
    prelude::*,
    window::{CursorOptions, PrimaryWindow},
};
use bevy_asset_loader::prelude::*;

use crate::client::network::movement::{PositionHistory, PredictedMovement, PredictionInputSet};
use crate::client::presentation::action_bar::{
    pressed_action_bar_slot, ActionBarBindings, ActionBarState,
};
use crate::client::presentation::animations::LastAnimationDirection;
use crate::client::presentation::casting::{CastingSpell, RequestSpellCast};
use crate::client::presentation::death::DEATH_SCREEN_Z_INDEX;
use crate::client::presentation::ui_drag::{pointer_over_draggable_ui, DraggableUi};
use crate::client::state::*;
use crate::shared::constants::WATER_LEVEL;
use crate::shared::gameplay::action_bar::ActionBarBinding;
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::entities::{Player, NPC};
use crate::shared::gameplay::items::{item_definition, GroundItem};
use crate::shared::gameplay::spells::{spell_definition, SpellEffect, SpellTargeting};
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
const ATTACK_HOLD_SECONDS: f64 = 0.25;
const AREA_TARGET_SURFACE_OFFSET: f32 = 0.045;
const GAME_CURSOR_Z_INDEX: i32 = DEATH_SCREEN_Z_INDEX + 1;

#[derive(SystemParam)]
struct PointerUiStates<'w, 's> {
    action_bar: Option<Res<'w, ActionBarState>>,
    draggable_panels: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static InheritedVisibility,
        ),
        With<DraggableUi>,
    >,
    sitting_players: Query<'w, 's, (), (With<ControlledPlayer>, With<Sitting>)>,
}

#[derive(Resource, Debug, Default)]
struct PointerHoldState {
    ground_movement: bool,
    last_destination: Option<Pos>,
    held_attack: Option<HeldAttack>,
}

#[derive(Clone, Copy, Debug)]
struct HeldAttack {
    server_entity: Entity,
    pressed_at: f64,
    continuous_requested: bool,
    persistent: bool,
}

fn should_issue_held_move(state: &mut PointerHoldState, destination: Pos, new_press: bool) -> bool {
    if !state.ground_movement {
        return false;
    }
    if !new_press && state.last_destination == Some(destination) {
        return false;
    }
    state.last_destination = Some(destination);
    true
}

fn held_attack_should_upgrade(held_attack: &HeldAttack, now: f64) -> bool {
    !held_attack.continuous_requested
        && !held_attack.persistent
        && now - held_attack.pressed_at >= ATTACK_HOLD_SECONDS
}

fn held_attack_should_stop_on_release(held_attack: &HeldAttack) -> bool {
    held_attack.continuous_requested && !held_attack.persistent
}

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

#[derive(Component)]
struct GroundItemTooltip;

#[derive(Component)]
struct PointerOwned;

#[derive(Component, Debug)]
struct AreaTargetPreview {
    radius: f32,
    valid: bool,
}

#[derive(Resource)]
struct AreaTargetPreviewMaterials {
    valid: Handle<StandardMaterial>,
    invalid: Handle<StandardMaterial>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum CursorKind {
    Default,
    Attack,
    Pickup,
    Cast { spell_id: u16 },
}

fn area_spell_preview(spell_id: u16) -> Option<(f32, Option<u32>)> {
    let definition = spell_definition(spell_id)?;
    if definition.targeting != SpellTargeting::GroundArea {
        return None;
    }
    let SpellEffect::Damage {
        area_radius: Some(radius),
        ..
    } = definition.effect
    else {
        return None;
    };
    Some((radius as f32, definition.max_range))
}

fn area_target_in_range(caster: Vec3, target: Vec3, max_range: Option<u32>) -> bool {
    max_range.is_none_or(|max_range| {
        let offset = target - caster;
        Vec2::new(offset.x, offset.z).length_squared() <= (max_range * max_range) as f32
    })
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
        .init_resource::<PointerHoldState>()
        //.add_plugins((DecalPlugin))
        .add_systems(
            OnEnter(ClientState::InGame),
            (setup_cursor, setup_ground_item_tooltip),
        )
        .add_systems(
            OnEnter(ClientState::InGame),
            (
                //setup_target,
                setup_target_decal
            ),
        )
        .add_systems(Update, draw_gizmos.run_if(in_state(ClientState::InGame)))
        .add_systems(
            Update,
            (
                move_cursor.run_if(in_state(ClientState::InGame)),
                update_ground_item_tooltip.run_if(in_state(ClientState::InGame)),
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
                update_area_target_preview
                    .run_if(in_state(ClientState::InGame))
                    .after(update_cursor_system_rapier3d),
                changed_cursor
                    .run_if(in_state(ClientState::InGame))
                    .after(setup_cursor),
            ),
        )
        .add_systems(OnExit(ClientState::InGame), despawn_pointer_owned);

        fn despawn_pointer_owned(
            mut commands: Commands,
            entities: Query<Entity, With<PointerOwned>>,
        ) {
            for entity in &entities {
                commands.entity(entity).try_despawn();
            }
        }

        fn setup_ground_item_tooltip(mut commands: Commands) {
            commands.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::new(1.0, 1.0),
                    color: Color::BLACK,
                },
                Node {
                    position_type: PositionType::Absolute,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.03, 0.045, 0.94)),
                BorderColor::all(Color::srgba(0.72, 0.75, 0.82, 0.95)),
                GlobalZIndex(999),
                Visibility::Hidden,
                Pickable::IGNORE,
                GroundItemTooltip,
                PointerOwned,
                Name::new("Ground item tooltip"),
            ));
        }

        fn update_ground_item_tooltip(
            primary_window: Query<&Window, With<PrimaryWindow>>,
            cursors: Query<&GameCursor>,
            ground_items: Query<&GroundItem>,
            mut tooltips: Query<(&mut Text, &mut Node, &mut Visibility), With<GroundItemTooltip>>,
        ) {
            let (Ok(window), Ok(cursor), Ok((mut text, mut node, mut visibility))) = (
                primary_window.single(),
                cursors.single(),
                tooltips.single_mut(),
            ) else {
                return;
            };

            let Some((item, definition)) = cursor
                .hovered_entity
                .and_then(|entity| ground_items.get(entity).ok())
                .and_then(|item| {
                    item_definition(item.item_id).map(|definition| (item, definition))
                })
            else {
                *visibility = Visibility::Hidden;
                return;
            };

            text.0 = if item.quantity > 1 {
                format!("{} x{}", definition.name, item.quantity)
            } else {
                definition.name.to_string()
            };
            if let Some(pointer) = window.cursor_position() {
                node.left = Val::Px((pointer.x + 18.0).min(window.width() - 120.0));
                node.top = Val::Px((pointer.y + 18.0).min(window.height() - 30.0));
            }
            *visibility = Visibility::Inherited;
        }

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

        /// Draws the affected radius and center crosshair for a pending AoE cast.
        fn draw_gizmos(
            mut gizmos: Gizmos,
            previews: Query<(&Transform, &AreaTargetPreview, &Visibility)>,
        ) {
            for (transform, preview, visibility) in &previews {
                if *visibility == Visibility::Hidden {
                    continue;
                }
                let color = if preview.valid {
                    Color::srgb(0.18, 0.86, 1.0)
                } else {
                    Color::from(RED)
                };
                let isometry = Isometry3d::new(transform.translation, transform.rotation);
                gizmos
                    .circle(isometry, preview.radius, color)
                    .resolution(64);
                gizmos
                    .circle(isometry, preview.radius * 0.5, color)
                    .resolution(48);
                let x_axis = transform.rotation * Vec3::X * preview.radius;
                let y_axis = transform.rotation * Vec3::Y * preview.radius;
                gizmos.line(
                    transform.translation - x_axis,
                    transform.translation + x_axis,
                    color,
                );
                gizmos.line(
                    transform.translation - y_axis,
                    transform.translation + y_axis,
                    color,
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
                PointerOwned,
                Name::new("Target"),
                //Transform::from_scale(Vec3::splat(11.0)),
                /*Transform {
                    translation: vec3(1.0,1.0,1.0),
                    rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                    scale: Vec3::splat(2.)
                }*/
                cursor_decal_transform(Vec3::ZERO, Vec3::Y),
            ));

            let valid_material = materials.add(StandardMaterial {
                base_color: Color::srgba(0.05, 0.62, 0.95, 0.22),
                emissive: LinearRgba::rgb(0.02, 0.22, 0.42),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            let invalid_material = materials.add(StandardMaterial {
                base_color: Color::srgba(0.95, 0.08, 0.06, 0.24),
                emissive: LinearRgba::rgb(0.42, 0.02, 0.01),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            commands.insert_resource(AreaTargetPreviewMaterials {
                valid: valid_material.clone(),
                invalid: invalid_material,
            });
            commands.spawn((
                Mesh3d(meshes.add(Circle::new(1.0))),
                MeshMaterial3d(valid_material),
                Transform::default(),
                Visibility::Hidden,
                Pickable::IGNORE,
                AreaTargetPreview {
                    radius: 0.0,
                    valid: true,
                },
                PointerOwned,
                Name::new("AoE affected area preview"),
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
            interactive_entities: Query<
                (Entity, Option<&GroundItem>, Option<&Monster>),
                Or<(With<Player>, With<NPC>, With<Monster>, With<GroundItem>)>,
            >,
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
                                if let CursorKind::Cast { spell_id } = game_cursor.action {
                                    let direct_monster_spell = spell_definition(spell_id)
                                        .is_some_and(|definition| {
                                            definition.targeting == SpellTargeting::DirectMonster
                                        });
                                    let hovered_monster = interactive_entities
                                        .get(entity)
                                        .ok()
                                        .and_then(|(interactive_entity, _, monster)| {
                                            (direct_monster_spell && monster.is_some())
                                                .then_some(interactive_entity)
                                        });
                                    if game_cursor.hovered_entity != hovered_monster {
                                        game_cursor.hovered_entity = hovered_monster;
                                    }
                                } else if let Ok((interactive_entity, ground_item, _)) =
                                    interactive_entities.get(entity)
                                {
                                    if (Some(interactive_entity) != game_cursor.hovered_entity) {
                                        game_cursor.hovered_entity = Some(interactive_entity);
                                    }

                                    let action = if ground_item.is_some() {
                                        CursorKind::Pickup
                                    } else {
                                        CursorKind::Attack
                                    };
                                    if game_cursor.action != action {
                                        game_cursor.action = action;
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

        fn update_area_target_preview(
            cursors: Query<&GameCursor>,
            target: Query<
                (&Transform, &CursorMapPosition),
                (With<Target>, Without<AreaTargetPreview>),
            >,
            caster: Query<
                &Transform,
                (
                    With<ControlledPlayer>,
                    Without<Target>,
                    Without<AreaTargetPreview>,
                ),
            >,
            preview_materials: Option<Res<AreaTargetPreviewMaterials>>,
            mut previews: Query<
                (
                    &mut Transform,
                    &mut Visibility,
                    &mut AreaTargetPreview,
                    &mut MeshMaterial3d<StandardMaterial>,
                ),
                (Without<Target>, Without<ControlledPlayer>),
            >,
        ) {
            let Ok((mut transform, mut visibility, mut preview, mut material)) =
                previews.single_mut()
            else {
                return;
            };

            let Some((radius, max_range)) = cursors.single().ok().and_then(|cursor| {
                let CursorKind::Cast { spell_id } = cursor.action else {
                    return None;
                };
                area_spell_preview(spell_id)
            }) else {
                *visibility = Visibility::Hidden;
                return;
            };

            let (Ok((target_transform, target_position)), Ok(caster_transform), Some(materials)) =
                (target.single(), caster.single(), preview_materials)
            else {
                *visibility = Visibility::Hidden;
                return;
            };

            let surface_normal = (target_transform.rotation * Vec3::Z).normalize_or(Vec3::Y);
            let valid =
                area_target_in_range(caster_transform.translation, target_position.0, max_range);

            preview.radius = radius;
            preview.valid = valid;
            transform.translation = target_position.0 + surface_normal * AREA_TARGET_SURFACE_OFFSET;
            transform.rotation = target_transform.rotation;
            transform.scale = Vec3::new(radius, radius, 1.0);
            material.0 = if valid {
                materials.valid.clone()
            } else {
                materials.invalid.clone()
            };
            *visibility = Visibility::Inherited;
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
                    CursorKind::Pickup => {
                        img.image = asset_server.load("cursors/PNG/03.png").into()
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
                    PointerOwned,
                    GlobalZIndex(GAME_CURSOR_Z_INDEX),
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
            time: Res<Time>,
            primary_window: Query<&Window, With<PrimaryWindow>>,
            action_bar_bindings: Option<Res<ActionBarBindings>>,
            ui_states: PointerUiStates,
            mut pointer_hold: ResMut<PointerHoldState>,
            network_mapping: Res<NetworkMapping>,
            target_query: Query<&CursorMapPosition, With<Target>>,
            mut player_commands: MessageWriter<PlayerCommand>,
            mut player_entities: Query<
                (
                    Entity,
                    Option<&mut PredictedMovement>,
                    &PositionHistory,
                    &mut Animation,
                    &Transform,
                    &mut Facing,
                ),
                With<ControlledPlayer>,
            >,
            mut cursors: Query<&mut GameCursor>,
            casting_players: Query<(), (With<ControlledPlayer>, With<CastingSpell>)>,
            spell_targets: Query<&Transform, With<Monster>>,
            //interactive_entities: Query<(Entity), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
        ) {
            if keyboard_input.just_pressed(KeyCode::KeyM) {
                **player_input = PlayerInput::default();
                pointer_hold.ground_movement = false;
                pointer_hold.last_destination = None;
                pointer_hold.held_attack = None;
                player_commands.write(PlayerCommand::CycleMap);
                return;
            }

            if keyboard_input.just_pressed(KeyCode::Insert) {
                **player_input = PlayerInput::default();
                pointer_hold.ground_movement = false;
                pointer_hold.last_destination = None;
                pointer_hold.held_attack = None;
                if let Ok(mut cursor) = cursors.single_mut() {
                    cursor.action = CursorKind::Default;
                    cursor.hovered_entity = None;
                }
                player_commands.write(PlayerCommand::ToggleSitting);
                return;
            }

            if !ui_states.sitting_players.is_empty() {
                **player_input = PlayerInput::default();
                pointer_hold.ground_movement = false;
                pointer_hold.last_destination = None;
                pointer_hold.held_attack = None;

                if mouse_button_input.just_pressed(MouseButton::Left) {
                    let pointer_over_ui = primary_window
                        .single()
                        .ok()
                        .and_then(Window::cursor_position)
                        .is_some_and(|pointer_position| {
                            ui_states
                                .action_bar
                                .as_deref()
                                .is_some_and(|bar| bar.captures_pointer(pointer_position))
                                || pointer_over_draggable_ui(
                                    pointer_position,
                                    &ui_states.draggable_panels,
                                )
                        });
                    if !pointer_over_ui {
                        if let (
                            Ok(target_position),
                            Ok((player_entity, _, _, mut animation, transform, mut facing)),
                        ) = (target_query.single(), player_entities.single_mut())
                        {
                            if let Some(new_facing) =
                                facing_from_direction(target_position.0 - transform.translation)
                            {
                                let world_direction = world_direction_from_facing(new_facing.0);
                                *facing = new_facing.clone();
                                *animation = Animation::Sitting;
                                commands
                                    .entity(player_entity)
                                    .insert(LastAnimationDirection(world_direction));
                                player_commands.write(PlayerCommand::Face {
                                    target: target_position.0,
                                });
                            }
                        }
                    }
                }
                return;
            }

            let movement_locked = !casting_players.is_empty();
            if movement_locked {
                **player_input = PlayerInput::default();
                pointer_hold.ground_movement = false;
                pointer_hold.last_destination = None;
                pointer_hold.held_attack = None;
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

            let selected_spell = pressed_action_bar_slot(&keyboard_input)
                .and_then(|slot_index| action_bar_bindings.as_deref()?.binding(slot_index))
                .and_then(|binding| match binding {
                    ActionBarBinding::Spell(spell_id) => Some(spell_id),
                    ActionBarBinding::Item(_) | ActionBarBinding::Skill(_) => None,
                });

            if let Some(spell_id) = selected_spell {
                let self_cast = spell_definition(spell_id)
                    .is_some_and(|definition| definition.targeting == SpellTargeting::SelfOnly);
                if self_cast {
                    if let Ok((_, _, _, _, transform, _)) = player_entities.single() {
                        commands.trigger(RequestSpellCast {
                            spell_id,
                            translation: transform.translation,
                            target_entity: None,
                        });
                    }
                    if let Ok(mut cursor) = cursors.single_mut() {
                        cursor.action = CursorKind::Default;
                        cursor.hovered_entity = None;
                    }
                } else if let Ok(mut cursor) = cursors.single_mut() {
                    cursor.action = CursorKind::Cast { spell_id };
                    cursor.hovered_entity = None;
                    pointer_hold.ground_movement = false;
                    pointer_hold.last_destination = None;
                    pointer_hold.held_attack = None;
                }
            }

            if mouse_button_input.just_pressed(MouseButton::Right) {
                if let Ok(mut cursor) = cursors.single_mut() {
                    if matches!(cursor.action, CursorKind::Cast { .. }) {
                        cursor.action = CursorKind::Default;
                        cursor.hovered_entity = None;
                        info!("Cancelled pending spell target selection");
                        return;
                    }
                }
            }

            let left_just_pressed = mouse_button_input.just_pressed(MouseButton::Left);
            let left_pressed = mouse_button_input.pressed(MouseButton::Left);
            let left_just_released = mouse_button_input.just_released(MouseButton::Left);
            let now = time.elapsed_secs_f64();

            if left_just_pressed {
                pointer_hold.held_attack = None;
            }
            if left_pressed {
                if let Some(held_attack) = pointer_hold.held_attack.as_mut() {
                    if held_attack_should_upgrade(held_attack, now) {
                        player_commands.write(PlayerCommand::BasicAttack {
                            entity: held_attack.server_entity,
                            auto_attack: true,
                        });
                        held_attack.continuous_requested = true;
                    }
                }
            }
            if left_just_released {
                if pointer_hold
                    .held_attack
                    .take()
                    .is_some_and(|held_attack| held_attack_should_stop_on_release(&held_attack))
                {
                    player_commands.write(PlayerCommand::StopBasicAttack);
                }
            }
            if !left_pressed {
                pointer_hold.ground_movement = false;
                pointer_hold.last_destination = None;
            }

            if left_just_pressed || (left_pressed && pointer_hold.ground_movement) {
                let pointer_over_ui = primary_window
                    .single()
                    .ok()
                    .and_then(Window::cursor_position)
                    .is_some_and(|pointer_position| {
                        ui_states
                            .action_bar
                            .as_deref()
                            .is_some_and(|bar| bar.captures_pointer(pointer_position))
                            || pointer_over_draggable_ui(
                                pointer_position,
                                &ui_states.draggable_panels,
                            )
                    });
                if pointer_over_ui {
                    if left_just_pressed {
                        pointer_hold.ground_movement = false;
                        pointer_hold.last_destination = None;
                    }
                    return;
                }
                if movement_locked {
                    return;
                }

                if let Ok(mut cursor) = cursors.single_mut() {
                    if left_just_pressed {
                        pointer_hold.ground_movement = matches!(cursor.action, CursorKind::Default);
                        pointer_hold.last_destination = None;
                    }
                    let action = if pointer_hold.ground_movement {
                        CursorKind::Default
                    } else {
                        cursor.action
                    };
                    match action {
                        CursorKind::Default => {
                            if let (
                                Ok(target_position),
                                Ok((
                                    player_entity,
                                    prediction,
                                    history,
                                    mut animation,
                                    _transform,
                                    _,
                                )),
                            ) = (target_query.single(), player_entities.single_mut())
                            {
                                let mut move_translation = target_position.0;
                                move_translation.x = move_translation.x.round();
                                move_translation.z = move_translation.z.round();
                                let destination =
                                    Pos(move_translation.x as i32, move_translation.z as i32);
                                if !should_issue_held_move(
                                    &mut pointer_hold,
                                    destination,
                                    left_just_pressed,
                                ) {
                                    return;
                                }
                                if target_position.0.y < WATER_LEVEL {
                                    info!("Ignoring movement into submerged terrain");
                                    return;
                                }

                                player_input.destination_at = Some(destination);
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
                                    let persistent = keyboard_input.pressed(KeyCode::ControlLeft)
                                        || keyboard_input.pressed(KeyCode::ControlRight);
                                    player_commands.write(PlayerCommand::BasicAttack {
                                        entity: *server_entity,
                                        auto_attack: persistent,
                                    });
                                    pointer_hold.held_attack = Some(HeldAttack {
                                        server_entity: *server_entity,
                                        pressed_at: now,
                                        continuous_requested: persistent,
                                        persistent,
                                    });
                                }
                            }
                        }
                        CursorKind::Pickup => {
                            if let Some(hovered_entity) = cursor.hovered_entity {
                                let server_entity =
                                    network_mapping.0.iter().find_map(|(key, &value)| {
                                        (value == hovered_entity).then_some(*key)
                                    });
                                if let Some(entity) = server_entity {
                                    player_commands.write(PlayerCommand::PickupItem { entity });
                                }
                            }
                        }
                        CursorKind::Cast { spell_id } => {
                            let Some(definition) = spell_definition(spell_id) else {
                                cursor.action = CursorKind::Default;
                                cursor.hovered_entity = None;
                                return;
                            };
                            let direct_target = match definition.targeting {
                                SpellTargeting::GroundArea => None,
                                SpellTargeting::DirectMonster => {
                                    let Some(client_entity) = cursor.hovered_entity else {
                                        info!("Spell {spell_id} requires a monster target");
                                        return;
                                    };
                                    let Some(server_entity) =
                                        network_mapping.0.iter().find_map(|(server, &client)| {
                                            (client == client_entity).then_some(*server)
                                        })
                                    else {
                                        warn!("Selected spell target has no server entity mapping");
                                        return;
                                    };
                                    Some((server_entity, client_entity))
                                }
                                SpellTargeting::SelfOnly => {
                                    cursor.action = CursorKind::Default;
                                    cursor.hovered_entity = None;
                                    return;
                                }
                            };

                            if let Ok(target_position) = target_query.single() {
                                let translation = direct_target
                                    .and_then(|(_, client_entity)| {
                                        spell_targets
                                            .get(client_entity)
                                            .ok()
                                            .map(|transform| transform.translation)
                                    })
                                    .unwrap_or(target_position.0);
                                commands.trigger(RequestSpellCast {
                                    spell_id,
                                    translation,
                                    target_entity: direct_target
                                        .map(|(server_entity, _)| server_entity),
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
    fn game_cursor_renders_above_the_death_screen() {
        assert!(GAME_CURSOR_Z_INDEX > DEATH_SCREEN_Z_INDEX);
    }

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

    #[test]
    fn area_preview_uses_the_ground_spell_definition() {
        assert_eq!(area_spell_preview(2), Some((3.0, Some(12))));
    }

    #[test]
    fn area_preview_ignores_spells_without_an_affected_radius() {
        assert_eq!(area_spell_preview(1), None);
        assert_eq!(area_spell_preview(3), None);
        assert_eq!(area_spell_preview(4), None);
    }

    #[test]
    fn area_preview_range_is_horizontal_and_includes_the_boundary() {
        let caster = Vec3::new(1.0, 2.0, 1.0);

        assert!(area_target_in_range(
            caster,
            Vec3::new(13.0, 50.0, 1.0),
            Some(12)
        ));
        assert!(!area_target_in_range(
            caster,
            Vec3::new(13.01, 2.0, 1.0),
            Some(12)
        ));
        assert!(area_target_in_range(
            caster,
            Vec3::new(1000.0, 2.0, 1000.0),
            None
        ));
    }

    #[test]
    fn held_ground_movement_sends_once_per_grid_destination() {
        let mut state = PointerHoldState {
            ground_movement: true,
            ..default()
        };

        assert!(should_issue_held_move(&mut state, Pos(2, 3), true));
        assert!(!should_issue_held_move(&mut state, Pos(2, 3), false));
        assert!(should_issue_held_move(&mut state, Pos(3, 3), false));
    }

    #[test]
    fn non_ground_holds_never_issue_movement() {
        let mut state = PointerHoldState::default();

        assert!(!should_issue_held_move(&mut state, Pos(2, 3), true));
        assert_eq!(state.last_destination, None);
    }

    #[test]
    fn normal_attack_hold_upgrades_after_threshold_and_stops_on_release() {
        let mut held_attack = HeldAttack {
            server_entity: Entity::PLACEHOLDER,
            pressed_at: 10.0,
            continuous_requested: false,
            persistent: false,
        };

        assert!(!held_attack_should_upgrade(&held_attack, 10.249));
        assert!(held_attack_should_upgrade(&held_attack, 10.25));
        held_attack.continuous_requested = true;
        assert!(!held_attack_should_upgrade(&held_attack, 11.0));
        assert!(held_attack_should_stop_on_release(&held_attack));
    }

    #[test]
    fn control_click_auto_attack_persists_after_release() {
        let held_attack = HeldAttack {
            server_entity: Entity::PLACEHOLDER,
            pressed_at: 10.0,
            continuous_requested: true,
            persistent: true,
        };

        assert!(!held_attack_should_upgrade(&held_attack, 11.0));
        assert!(!held_attack_should_stop_on_release(&held_attack));
    }
}
