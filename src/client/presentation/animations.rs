use bevy::{prelude::*, transform::TransformSystems};

use crate::client::assets::ChaskiAssets;
use crate::client::network::movement::InterpolationSet;
use crate::client::state::CameraFacing;
use crate::shared::constants::ATTACK_ANIMATION_FRAME_COUNT;
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::events::DeathEvent;
use crate::shared::states::ClientState;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraSystemSet};
use bevy_sprite3d::*;

const ATLAS_FRAMES_PER_DIRECTION: usize = 8;
const DIRECTION_ANGLE: f32 = std::f32::consts::TAU / 8.0;
const MIN_BILLBOARD_PITCH_COSINE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttackSheet {
    SideDown,
    SideUp,
}

fn attack_visual(direction: u8) -> (AttackSheet, bool) {
    match direction % 8 {
        0 => (AttackSheet::SideUp, false),
        1 => (AttackSheet::SideUp, true),
        2 | 3 => (AttackSheet::SideDown, true),
        4 | 5 | 6 => (AttackSheet::SideDown, false),
        7 => (AttackSheet::SideUp, false),
        _ => unreachable!(),
    }
}

fn attack_frame_duration(attack_period: f32) -> f32 {
    attack_period.max(0.001) / ATTACK_ANIMATION_FRAME_COUNT as f32
}

fn advance_attack_frame(current_frame: usize, advances: usize, auto_attack: bool) -> Option<usize> {
    let next_frame = current_frame + advances;
    if next_frame >= ATTACK_ANIMATION_FRAME_COUNT && !auto_attack {
        None
    } else {
        Some(next_frame % ATTACK_ANIMATION_FRAME_COUNT)
    }
}

fn facing_world_direction(facing: u8) -> Vec3 {
    let angle = facing as f32 * DIRECTION_ANGLE;
    Vec3::new(-angle.sin(), 0.0, angle.cos())
}

fn atlas_direction(camera: &Transform, world_direction: Vec3) -> u8 {
    let movement = Vec3::new(world_direction.x, 0.0, world_direction.z).normalize_or_zero();
    if movement == Vec3::ZERO {
        return 0;
    }

    let camera_forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or_zero();
    let camera_right = Vec3::new(camera.right().x, 0.0, camera.right().z).normalize_or_zero();
    let screen_right = movement.dot(camera_right);
    let away_from_camera = movement.dot(camera_forward);
    let octant = (screen_right.atan2(away_from_camera) / DIRECTION_ANGLE).round() as i32;

    octant.rem_euclid(8) as u8
}

fn animation_world_direction(velocity: Vec3, last_direction: Option<Vec3>, facing: u8) -> Vec3 {
    let planar_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
    if planar_velocity.length_squared() > f32::EPSILON {
        planar_velocity.normalize()
    } else {
        last_direction.unwrap_or_else(|| facing_world_direction(facing))
    }
}

fn align_billboard_to_camera(sprite_transform: &mut Transform, yaw: f32, pitch: f32) {
    // Keep the quad upright so it cannot tilt into nearby world geometry. The
    // inverse-cosine scale compensates for the vertical foreshortening caused by
    // camera pitch while retaining a cylindrical billboard.
    let pitch_correction = pitch.cos().abs().max(MIN_BILLBOARD_PITCH_COSINE).recip();

    sprite_transform.rotation = Quat::from_rotation_y(yaw);
    sprite_transform.scale = Vec3::new(1.0, pitch_correction, 1.0);
}

#[derive(Component, Debug, Clone, Copy)]
struct LastAnimationDirection(Vec3);

#[derive(Component)]
pub(crate) struct WalkingSpriteVisual;

#[derive(Component)]
pub(crate) struct AttackSpriteVisual;

pub struct AnimationsPlugin;

impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.add_systems(
            Update,
            (
                set_camera_facing
                    .run_if(in_state(ClientState::InGame))
                    .after(InterpolationSet),
                set_entities_facing
                    .run_if(in_state(ClientState::InGame))
                    .after(InterpolationSet),
            ),
        )
        .add_systems(
            PostUpdate,
            billboard
                .run_if(in_state(ClientState::InGame))
                .after(PanOrbitCameraSystemSet)
                .before(TransformSystems::Propagate),
        )
        .add_systems(
            FixedUpdate,
            (sprite_movement.run_if(in_state(ClientState::InGame)),),
        )
        .add_observer(on_death_animate);

        fn billboard(
            camera_query: Query<&PanOrbitCamera, (With<Camera3d>, Without<Billboard>)>,
            mut entities_query: Query<&mut Transform, With<Billboard>>,
        ) {
            if let Ok(pan_camera) = camera_query.single() {
                if let (Some(yaw), Some(pitch)) = (pan_camera.yaw, pan_camera.pitch) {
                    for mut entity_transform in entities_query.iter_mut() {
                        align_billboard_to_camera(&mut entity_transform, yaw, pitch);
                    }
                }
            }
        }

        fn set_camera_facing(
            mut camera_query: Query<
                (&Transform, &PanOrbitCamera),
                (With<Camera>, Changed<Transform>),
            >,
            mut camera_facing: ResMut<CameraFacing>,
        ) {
            if let Ok((camera_transform, pan_cam)) = camera_query.single_mut() {
                if let Some(yaw) = pan_cam.yaw {
                    let mut rotation = ((8.0 * (yaw.to_degrees()) / 360.0).round() % 8.0) as i32;

                    if rotation < 0 {
                        rotation += 8;
                    }

                    if rotation as u8 != camera_facing.0 {
                        camera_facing.0 = rotation as u8;
                        println!("camera_facing {:?}", camera_facing.0);
                    }
                }
            }
        }

        fn set_entities_facing(mut query: Query<(&mut Facing, &GameVelocity)>) {
            for (mut facing, velocity) in query.iter_mut() {
                if velocity.0 == Vec3::ZERO {
                    continue;
                }

                let x = (velocity.0.x * 1000.0).round() / 1000.0;
                let z = (velocity.0.z * 1000.0).round() / 1000.0;

                // Mirando hacia arriba
                if z > 0. && x == 0.0 {
                    *facing = Facing(0);
                }
                // Mirando hacia la arriba a la derecha
                else if z > 0. && x < 0.0 {
                    *facing = Facing(1);
                }
                // Mirando hacia la derecha
                else if z == 0. && x < 0.0 {
                    *facing = Facing(2);
                }
                // Mirando hacia la abajo a la derecha
                else if z < 0. && x < 0.0 {
                    *facing = Facing(3);
                }
                // Mirando hacia abajo
                else if z < 0. && x == 0.0 {
                    *facing = Facing(4);
                }
                // Mirando hacia la abajo a la izquierda
                else if z < 0. && x > 0.0 {
                    *facing = Facing(5);
                }
                // Mirando hacia la izquierda
                else if z == 0. && x > 0.0 {
                    *facing = Facing(6);
                }
                // Mirando hacia la arriba a la izquierda
                else if z > 0. && x > 0.0 {
                    *facing = Facing(7);
                }
            }
        }

        fn on_death_animate(
            trigger: On<DeathEvent>,
            mut query: Query<(&Transform)>,
            mut commands: Commands,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let death_event = trigger.event();
            let id: Entity = death_event.entity;

            if let Ok((transform)) = query.get_mut(id) {
                info!("Se crea loot en:  {:?} ", transform.translation);
                // Si es jugador, mantenrlo muerto en el piso.
                // Si es monstruo, debe soltar ítems.
            }
        }

        fn sprite_movement(
            mut commands: Commands,
            time: Res<Time>,
            chaski: Res<ChaskiAssets>,
            mut q_parent: Query<(
                &mut AnimationTimer,
                Ref<Facing>,
                &GameVelocity,
                &mut Animation,
                Option<&mut LastAnimationDirection>,
            )>,
            mut q_walking_child: Query<
                (&ChildOf, &mut Sprite, &mut Visibility),
                (With<WalkingSpriteVisual>, Without<AttackSpriteVisual>),
            >,
            mut q_attack_child: Query<
                (&ChildOf, &mut Sprite, &mut Visibility),
                (With<AttackSpriteVisual>, Without<WalkingSpriteVisual>),
            >,
            camera: Query<&Transform, (With<Camera3d>, Without<Billboard>)>,
            transforms: Query<&Transform>,
        ) {
            let Ok(camera_transform) = camera.single() else {
                return;
            };

            for (parent, mut walking_sprite, mut walking_visibility) in q_walking_child.iter_mut() {
                let owner = parent.parent();
                let Some((_, mut attack_sprite, mut attack_visibility)) = q_attack_child
                    .iter_mut()
                    .find(|(attack_parent, _, _)| attack_parent.parent() == owner)
                else {
                    continue;
                };

                if let Ok((mut timer, facing, velocity, mut animation, mut last_direction)) =
                    q_parent.get_mut(owner)
                {
                    let attack = match &*animation {
                        Animation::Attacking {
                            enemy,
                            attack_speed,
                            auto_attack,
                            ..
                        } => Some((*enemy, *attack_speed, *auto_attack)),
                        _ => None,
                    };

                    if let Some((enemy, attack_period, auto_attack)) = attack {
                        let previous_direction =
                            last_direction.as_deref().map(|direction| direction.0);
                        let attack_direction = transforms
                            .get(owner)
                            .ok()
                            .zip(transforms.get(enemy).ok())
                            .map(|(attacker, target)| {
                                let difference = target.translation - attacker.translation;
                                Vec3::new(difference.x, 0.0, difference.z).normalize_or_zero()
                            })
                            .filter(|direction| *direction != Vec3::ZERO)
                            .or(previous_direction)
                            .unwrap_or_else(|| facing_world_direction(facing.0));

                        if let Some(last_direction) = last_direction.as_deref_mut() {
                            last_direction.0 = attack_direction;
                        } else {
                            commands
                                .entity(owner)
                                .insert(LastAnimationDirection(attack_direction));
                        }

                        let visible_direction = atlas_direction(camera_transform, attack_direction);
                        let (sheet, flip_x) = attack_visual(visible_direction);
                        let attack_image = match sheet {
                            AttackSheet::SideDown => &chaski.attack_side_down,
                            AttackSheet::SideUp => &chaski.attack_side_up,
                        };

                        if animation.is_changed() {
                            timer.0 = Timer::from_seconds(
                                attack_frame_duration(attack_period),
                                TimerMode::Repeating,
                            );
                            if let Some(atlas) = &mut attack_sprite.texture_atlas {
                                atlas.index = 0;
                            }
                        }

                        *walking_visibility = Visibility::Hidden;
                        *attack_visibility = Visibility::Inherited;

                        if attack_sprite.image.id() != attack_image.id() {
                            attack_sprite.image = attack_image.clone();
                        }
                        attack_sprite.flip_x = flip_x;

                        timer.tick(time.delta());
                        let advances = timer.times_finished_this_tick() as usize;
                        if advances > 0 {
                            let current_frame = attack_sprite
                                .texture_atlas
                                .as_ref()
                                .map(|atlas| atlas.index)
                                .unwrap_or_default();
                            let next_frame =
                                advance_attack_frame(current_frame, advances, auto_attack);

                            if next_frame.is_none() {
                                *animation = Animation::Idle;
                                *walking_visibility = Visibility::Inherited;
                                *attack_visibility = Visibility::Hidden;
                                walking_sprite.flip_x = false;
                                if let Some(atlas) = &mut walking_sprite.texture_atlas {
                                    atlas.index =
                                        visible_direction as usize * ATLAS_FRAMES_PER_DIRECTION;
                                }
                                timer.0 = Timer::from_seconds(0.1, TimerMode::Repeating);
                            } else if let Some(atlas) = &mut attack_sprite.texture_atlas {
                                atlas.index = next_frame.unwrap();
                            }
                        }
                        continue;
                    }

                    *walking_visibility = Visibility::Inherited;
                    *attack_visibility = Visibility::Hidden;

                    //println!("Animation {:?}", animation);

                    // Cuando se cambia la rotación, se debe ajustar el sprite.
                    let previous_direction = last_direction.as_deref().map(|direction| direction.0);
                    let world_direction =
                        animation_world_direction(velocity.0, previous_direction, facing.0);
                    if velocity.0.x * velocity.0.x + velocity.0.z * velocity.0.z > f32::EPSILON {
                        let direction = world_direction;
                        if let Some(last_direction) = last_direction.as_deref_mut() {
                            last_direction.0 = direction;
                        } else {
                            commands
                                .entity(owner)
                                .insert(LastAnimationDirection(direction));
                        }
                    }
                    let row_index = atlas_direction(camera_transform, world_direction) as usize;

                    if walking_sprite.image.id() != chaski.sprite.id() {
                        walking_sprite.image = chaski.sprite.clone();
                        walking_sprite.flip_x = false;
                    }

                    if let Some(atlas) = &mut walking_sprite.texture_atlas {
                        let current_row = atlas.index / ATLAS_FRAMES_PER_DIRECTION;
                        if current_row != row_index {
                            let col_index = atlas.index % ATLAS_FRAMES_PER_DIRECTION;
                            atlas.index = col_index + row_index * ATLAS_FRAMES_PER_DIRECTION;
                        }
                    }

                    if velocity.0 == Vec3::ZERO {
                        continue;
                    }

                    let x = (velocity.0.x * 1000.0).round() / 1000.0;
                    let z = (velocity.0.z * 1000.0).round() / 1000.0;

                    if z != 0. || x != 0.0 {
                        //let row_index = (8 * atlas.index / 64) % 8;

                        timer.tick(time.delta());
                        if timer.just_finished() {
                            //let col_index = atlas.index  % 8;

                            //println!("row_index {:?}",row_index);
                            let starting_row_animation = row_index * ATLAS_FRAMES_PER_DIRECTION;
                            //println!("starting_row_animation {:?}",starting_row_animation);
                            let a = (starting_row_animation)
                                ..(starting_row_animation + ATLAS_FRAMES_PER_DIRECTION - 1);

                            //println!("range {:?}, atlas.index {:?}",a ,atlas.index );
                            if let Some(atlas) = &mut walking_sprite.texture_atlas {
                                atlas.index = if !a.contains(&atlas.index)
                                    || atlas.index
                                        == ((row_index * ATLAS_FRAMES_PER_DIRECTION)
                                            + ATLAS_FRAMES_PER_DIRECTION
                                            - 1)
                                {
                                    starting_row_animation
                                } else {
                                    atlas.index + 1
                                };
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

    fn camera_at(position: Vec3) -> Transform {
        Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y)
    }

    #[test]
    fn cardinal_movements_select_the_expected_visible_rows() {
        let camera = camera_at(Vec3::new(0.0, 10.0, 10.0));

        assert_eq!(atlas_direction(&camera, Vec3::Z), 4);
        assert_eq!(atlas_direction(&camera, Vec3::NEG_Z), 0);
        assert_eq!(atlas_direction(&camera, Vec3::X), 2);
        assert_eq!(atlas_direction(&camera, Vec3::NEG_X), 6);
    }

    #[test]
    fn movement_toward_camera_is_front_facing_at_any_yaw() {
        for octant in 0..8 {
            let angle = octant as f32 * DIRECTION_ANGLE;
            let camera_position = Vec3::new(angle.sin() * 10.0, 10.0, angle.cos() * 10.0);
            let camera = camera_at(camera_position);
            let toward_camera = Vec3::new(camera_position.x, 0.0, camera_position.z);

            assert_eq!(atlas_direction(&camera, toward_camera), 4);
        }
    }

    #[test]
    fn stopping_preserves_the_last_precise_movement_direction() {
        let camera = camera_at(Vec3::new(0.0, 10.0, 10.0));
        let last_direction = Vec3::new(0.1, 0.0, 1.0).normalize();
        let stopped_direction = animation_world_direction(Vec3::ZERO, Some(last_direction), 7);

        assert_eq!(atlas_direction(&camera, stopped_direction), 4);
        assert_eq!(
            atlas_direction(&camera, animation_world_direction(Vec3::ZERO, None, 7)),
            3
        );
    }

    #[test]
    fn stored_facing_values_round_trip_through_world_directions() {
        let camera = camera_at(Vec3::new(0.0, 10.0, 10.0));

        for facing in 0..8 {
            assert_eq!(
                atlas_direction(&camera, facing_world_direction(facing)),
                (facing + 4) % 8
            );
        }
    }

    #[test]
    fn attack_visuals_choose_a_sheet_and_mirror_for_each_side() {
        assert_eq!(attack_visual(7), (AttackSheet::SideUp, false));
        assert_eq!(attack_visual(1), (AttackSheet::SideUp, true));
        assert_eq!(attack_visual(5), (AttackSheet::SideDown, false));
        assert_eq!(attack_visual(3), (AttackSheet::SideDown, true));
    }

    #[test]
    fn attack_speed_is_the_duration_of_all_eight_frames() {
        assert!((attack_frame_duration(0.8) - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn one_shot_attacks_finish_while_auto_attacks_wrap() {
        assert_eq!(advance_attack_frame(7, 1, false), None);
        assert_eq!(advance_attack_frame(7, 1, true), Some(0));
        assert_eq!(advance_attack_frame(3, 1, false), Some(4));
    }

    #[test]
    fn cylindrical_billboard_stays_upright_and_preserves_apparent_height() {
        for pitch in [0.0, 15.0_f32.to_radians(), 30.0_f32.to_radians()] {
            let yaw = 35.0_f32.to_radians();
            let mut sprite_transform = Transform::from_scale(Vec3::new(1.0, 2.5, 1.0));

            align_billboard_to_camera(&mut sprite_transform, yaw, pitch);

            assert!((sprite_transform.scale.y * pitch.cos() - 1.0).abs() < 1e-5);
            assert_eq!(sprite_transform.scale.x, 1.0);
            assert_eq!(sprite_transform.scale.z, 1.0);
            assert!((sprite_transform.rotation * Vec3::Y).dot(Vec3::Y) > 1.0 - 1e-5);
        }
    }

    #[test]
    fn billboard_pitch_correction_is_bounded_at_extreme_angles() {
        let mut sprite_transform = Transform::default();

        align_billboard_to_camera(&mut sprite_transform, 0.0, std::f32::consts::FRAC_PI_2);

        assert_eq!(sprite_transform.scale.y, MIN_BILLBOARD_PITCH_COSINE.recip());
    }
}
