

use bevy::prelude::*;
use crate::*;
use avian3d::math::{AdjustPrecision, Quaternion, Scalar, Vector};
use avian3d::prelude::{CoefficientCombine, Collider, ColliderParent, Collisions, Friction, GravityScale, LinearVelocity, LockedAxes, Mass, Position, PostProcessCollisions, Restitution, RigidBody, Rotation, Sensor};
use avian3d::{position, PhysicsPlugins};



pub struct ServerPhysicsPlugin;

impl Plugin for ServerPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app                     
            .add_systems(Update, apply_gravity)
            .add_systems(
                // Run collision handling after collision detection.
                //
                // NOTE: The collision implementation here is very basic and a bit buggy.
                //       A collide-and-slide algorithm would likely work better.
                PostProcessCollisions,
                (kinematic_controller_collisions)
                
            );
        }
}



#[allow(clippy::type_complexity)]
fn kinematic_controller_collisions(
    collisions: Res<Collisions>,
    collider_parents: Query<&ColliderParent, Without<Sensor>>,
    mut players: Query<(&Transform, &mut Position,&mut LinearVelocity), (With<RigidBody>, With<Player>)>,
    bodies: Query<&RigidBody>,
    time: Res<Time>,
) {

    for contacts in collisions.iter() {

        // Get the rigid body entities of the colliders (colliders could be children)
        let Ok([collider_parent1, collider_parent2]) =
            collider_parents.get_many([contacts.entity1, contacts.entity2])
        else {
            continue;
        };
      
        // Get the body of the character controller and whether it is the first
        // or second entity in the collision.
        let is_first: bool;
        let character_rb: RigidBody;
        let is_other_dynamic: bool;
       

        let (mut transform, mut position, mut linear_velocity) =
            if let Ok(character) = players.get_mut(collider_parent1.get()) {
                is_first = true;
                character_rb = *bodies.get(collider_parent1.get()).unwrap();
                is_other_dynamic = bodies
                    .get(collider_parent2.get())
                    .is_ok_and(|rb| rb.is_dynamic());
                character
            } else if let Ok(character) = players.get_mut(collider_parent2.get()) {
                is_first = false;
                character_rb = *bodies.get(collider_parent2.get()).unwrap();
                is_other_dynamic = bodies
                    .get(collider_parent1.get())
                    .is_ok_and(|rb| rb.is_dynamic());
                character
            } else {
                continue;
            };

            let rotation =  Rotation(Quaternion::default());

        for manifold in contacts.manifolds.iter() {
            //println!("manifold: {:?}", manifold); 
            let normal = if is_first {
                -manifold.global_normal1(&rotation)
            } else {
                -manifold.global_normal2(&rotation)
            };

            let mut deepest_penetration: Scalar = Scalar::MIN;

            // Solve each penetrating contact in the manifold.
            for contact in manifold.contacts.iter() {
                if contact.penetration > 0.0 {
                    position.0 += normal * contact.penetration;
                }
                deepest_penetration = deepest_penetration.max(contact.penetration);
            }

            // For now, this system only handles velocity corrections for collisions against static geometry.
            if is_other_dynamic {
                continue;
            }
          

            // Determine if the slope is climbable or if it's too steep to walk on.
            let slope_angle = normal.angle_between(Vector::Y);
            let max_slope_angle = Some(30.0 as Scalar);
            let climbable = max_slope_angle.is_some_and(|angle| slope_angle.abs() <= angle);

            if deepest_penetration > 0.0 {
                // If the slope is climbable, snap the velocity so that the character
                // up and down the surface smoothly.
                if climbable {
                  
                    // Points in the normal's direction in the XZ plane.
                    let normal_direction_xz =
                        normal.reject_from_normalized(Vector::Y).normalize_or_zero();

                    // The movement speed along the direction above.
                    let linear_velocity_xz = linear_velocity.dot(normal_direction_xz);

                    // Snap the Y speed based on the speed at which the character is moving
                    // up or down the slope, and how steep the slope is.
                    //
                    // A 2D visualization of the slope, the contact normal, and the velocity components:
                    //
                    //             ╱
                    //     normal ╱
                    // *         ╱
                    // │   *    ╱   velocity_x
                    // │       * - - - - - -
                    // │           *       | velocity_y
                    // │               *   |
                    // *───────────────────*

                    let max_y_speed = -linear_velocity_xz * slope_angle.tan();
                    //println!("max_y_speed: {:?}", max_y_speed); 
                
                    linear_velocity.y = linear_velocity.y.max(max_y_speed);
                    //println!("linear_velocity_y: {:?}", linear_velocity.y); 
                } else {
                    // The character is intersecting an unclimbable object, like a wall.
                    // We want the character to slide along the surface, similarly to
                    // a collide-and-slide algorithm.

                    // Don't apply an impulse if the character is moving away from the surface.
                    if linear_velocity.dot(normal) > 0.0 {
                        continue;
                    }

                    // Slide along the surface, rejecting the velocity along the contact normal.
                    let impulse = linear_velocity.reject_from_normalized(normal);
                    linear_velocity.0 = impulse;
                }
            } else {
                // The character is not yet intersecting the other object,
                // but the narrow phase detected a speculative collision.
                //
                // We need to push back the part of the velocity
                // that would cause penetration within the next frame.

                let normal_speed = linear_velocity.dot(normal);

                // Don't apply an impulse if the character is moving away from the surface.
                if normal_speed > 0.0 {
                    continue;
                }

                // Compute the impulse to apply.
                let impulse_magnitude = normal_speed
                    - (deepest_penetration / time.delta_seconds_f64().adjust_precision());
                let mut impulse = impulse_magnitude * normal;

                // Apply the impulse differently depending on the slope angle.
                if climbable {
                    // Avoid sliding down slopes.
                    linear_velocity.y -= impulse.y.min(0.0);
                } else {
                    // Avoid climbing up walls.
                    impulse.y = impulse.y.max(0.0);
                    linear_velocity.0 -= impulse;
                }
            }
        }
    }

    
}

/// Applies [`ControllerGravity`] to character controllers.
fn apply_gravity(
    time: Res<Time>,
    mut players: Query<(&mut Velocity, &mut LinearVelocity), With<Player>>,
) {
    // Precision is adjusted so that the example works with
    // both the `f32` and `f64` features. Otherwise you don't need this.
    let delta_time = time.delta_seconds();
    let gravity = Vector::NEG_Y * 9.81 * 2.0;

    for (mut velocity, mut linear_velocity) in &mut players {
        linear_velocity.0 += gravity * delta_time;
        //velocity.0 += gravity * delta_time;
        //println!("linear_velocity: {:?}", linear_velocity.0); 
    }
}
