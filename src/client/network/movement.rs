use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_rapier3d::prelude::KinematicCharacterController;
#[cfg(feature = "client_prediction")]
use bevy_rapier3d::prelude::KinematicCharacterControllerOutput;

use crate::shared::states::ClientState;
use crate::{
    client::state::{ControlledPlayer, RenderTime},
    shared::{constants::*, gameplay::components::GameVelocity},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionSnapshot {
    pub position: Vec3,
    /// Milliseconds on the server's virtual clock.
    pub timestamp: u128,
}

#[derive(Component, Debug)]
pub struct PositionHistory {
    buffer: VecDeque<PositionSnapshot>,
}

impl PositionHistory {
    pub fn new(position: Vec3, timestamp: u128) -> Self {
        Self {
            buffer: VecDeque::from([PositionSnapshot {
                position,
                timestamp,
            }]),
        }
    }

    /// Adds a newer authoritative sample. Stale unreliable packets are discarded.
    pub fn add_absolute_position(&mut self, position: Vec3, timestamp: u128) -> bool {
        if let Some(latest) = self.buffer.back_mut() {
            if timestamp < latest.timestamp {
                return false;
            }
            if timestamp == latest.timestamp {
                latest.position = position;
                return true;
            }
        }

        self.buffer.push_back(PositionSnapshot {
            position,
            timestamp,
        });

        while self.buffer.len() > MAX_POSITION_SNAPSHOTS {
            self.buffer.pop_front();
        }

        true
    }

    pub fn latest(&self) -> Option<PositionSnapshot> {
        self.buffer.back().copied()
    }

    fn sample(&mut self, render_time: u128) -> Option<Vec3> {
        while self.buffer.len() >= 2 && self.buffer[1].timestamp <= render_time {
            self.buffer.pop_front();
        }

        if self.buffer.len() >= 2 {
            let a = self.buffer[0];
            let b = self.buffer[1];

            if render_time < a.timestamp {
                return None;
            }

            let progress = if b.timestamp > a.timestamp {
                ((render_time - a.timestamp) as f32 / (b.timestamp - a.timestamp) as f32)
                    .clamp(0.0, 1.0)
            } else {
                1.0
            };

            return Some(a.position.lerp(b.position, progress));
        }

        self.buffer
            .back()
            .filter(|snapshot| snapshot.timestamp <= render_time)
            .map(|snapshot| snapshot.position)
    }
}

/// The newest authoritative state is kept separately from the rendered transform.
#[derive(Component, Debug, Clone, Copy)]
pub struct AuthoritativePosition {
    pub position: Vec3,
    pub timestamp: u128,
}

impl AuthoritativePosition {
    pub fn new(position: Vec3, timestamp: u128) -> Self {
        Self {
            position,
            timestamp,
        }
    }
}

/// Immediate local movement that is later reconciled against server snapshots.
#[derive(Component, Debug, Default)]
pub struct PredictedMovement {
    pub destination: Option<Vec3>,
    /// Do not reconcile against snapshots that predate the most recent command.
    pub reconcile_after: u128,
}

impl PredictedMovement {
    pub fn start(&mut self, destination: Vec3, latest_server_timestamp: u128) {
        self.destination = Some(destination);
        self.reconcile_after = latest_server_timestamp;
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PredictionInputSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PredictionSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterpolationSet;

pub struct InterpolationPlugin;

impl Plugin for InterpolationPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (PredictionInputSet, PredictionSet, InterpolationSet).chain(),
        );

        #[cfg(feature = "client_prediction")]
        app.add_systems(
            Update,
            predict_controlled_player
                .in_set(PredictionSet)
                .run_if(in_state(ClientState::InGame)),
        );

        app.add_systems(
            Update,
            interpolate_and_reconcile
                .in_set(InterpolationSet)
                .run_if(in_state(ClientState::InGame)),
        );
    }
}

#[cfg(feature = "client_prediction")]
fn predict_controlled_player(
    time: Res<Time>,
    mut players: Query<
        (
            &Transform,
            &mut PredictedMovement,
            &mut GameVelocity,
            &mut KinematicCharacterController,
            Option<&KinematicCharacterControllerOutput>,
        ),
        With<ControlledPlayer>,
    >,
) {
    let delta_seconds = time.delta_secs();
    if delta_seconds <= 0.0 {
        return;
    }

    for (transform, mut prediction, mut velocity, mut controller, output) in &mut players {
        let mut movement = Vec3::ZERO;

        // Match the authoritative controller: keep applying gravity until Rapier says
        // the capsule is grounded. Horizontal movement is then projected onto slopes.
        if !output.map(|output| output.grounded).unwrap_or(false) {
            movement.y = CHARACTER_GRAVITY * delta_seconds;
        }

        if let Some(destination) = prediction.destination {
            let current = transform.translation;
            let max_step = PLAYER_MOVE_SPEED * delta_seconds;
            let next_x = move_towards(current.x, destination.x, max_step);
            let next_z = move_towards(current.z, destination.z, max_step);

            movement.x = next_x - current.x;
            movement.z = next_z - current.z;

            if next_x == destination.x && next_z == destination.z {
                prediction.destination = None;
            }
        }

        velocity.0 = Vec3::new(movement.x, 0.0, movement.z) / delta_seconds;
        controller.translation = (movement.length_squared() > f32::EPSILON).then_some(movement);
    }
}

#[cfg(feature = "client_prediction")]
fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    if (target - current).abs() <= max_delta {
        target
    } else {
        current + (target - current).signum() * max_delta
    }
}

fn interpolate_and_reconcile(
    time: Res<Time>,
    render_time: Res<RenderTime>,
    mut entities: Query<(
        &mut Transform,
        &mut PositionHistory,
        &AuthoritativePosition,
        &mut GameVelocity,
        Option<&mut PredictedMovement>,
        Option<&ControlledPlayer>,
        Option<&mut KinematicCharacterController>,
    )>,
) {
    if render_time.0 == 0 {
        return;
    }

    let delta_seconds = time.delta_secs();
    if delta_seconds <= 0.0 {
        return;
    }

    for (
        mut transform,
        mut history,
        authoritative,
        mut velocity,
        prediction,
        controlled,
        mut controller,
    ) in &mut entities
    {
        #[cfg(feature = "client_prediction")]
        if controlled.is_some() {
            let Some(mut prediction) = prediction else {
                continue;
            };

            if authoritative.timestamp <= prediction.reconcile_after {
                continue;
            }

            let error = authoritative.position - transform.translation;
            let horizontal_error = Vec2::new(error.x, error.z).length();

            if horizontal_error >= PREDICTION_HARD_SNAP_DISTANCE {
                transform.translation = authoritative.position;
                prediction.destination = None;
                velocity.0 = Vec3::ZERO;
                if let Some(controller) = controller.as_deref_mut() {
                    controller.translation = None;
                }
            } else {
                let correction = 1.0 - (-PREDICTION_SOFT_CORRECTION_RATE * delta_seconds).exp();
                transform.translation += error * correction;
            }
            continue;
        }

        #[cfg(not(feature = "client_prediction"))]
        let _ = (authoritative, prediction, controlled, controller);

        let Some(sampled_position) = history.sample(render_time.0) else {
            continue;
        };

        let previous = transform.translation;
        let has_interpolation_pair = history.buffer.len() >= 2;
        transform.translation = if has_interpolation_pair {
            sampled_position
        } else {
            // Smoothly catch the newest snapshot if jitter temporarily drains the buffer.
            let smoothing = 1.0 - (-20.0 * delta_seconds).exp();
            previous.lerp(sampled_position, smoothing)
        };
        velocity.0 = (transform.translation - previous) / delta_seconds;

        if velocity.0.length_squared() < 0.000_001 {
            velocity.0 = Vec3::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "client_prediction")]
    use std::time::Duration;

    #[cfg(feature = "client_prediction")]
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn rejects_stale_unreliable_snapshots() {
        let mut history = PositionHistory::new(Vec3::ZERO, 100);
        assert!(history.add_absolute_position(Vec3::X, 110));
        assert!(!history.add_absolute_position(Vec3::splat(99.0), 105));
        assert_eq!(history.latest().unwrap().position, Vec3::X);
    }

    #[test]
    fn bounds_snapshot_history() {
        let mut history = PositionHistory::new(Vec3::ZERO, 0);
        for timestamp in 1..=(MAX_POSITION_SNAPSHOTS as u128 + 10) {
            history.add_absolute_position(Vec3::X * timestamp as f32, timestamp);
        }
        assert_eq!(history.buffer.len(), MAX_POSITION_SNAPSHOTS);
    }

    #[test]
    fn interpolates_between_server_snapshots() {
        let mut history = PositionHistory::new(Vec3::ZERO, 100);
        history.add_absolute_position(Vec3::X * 10.0, 200);
        assert_eq!(history.sample(150), Some(Vec3::X * 5.0));
    }

    #[cfg(feature = "client_prediction")]
    #[test]
    fn prediction_queues_collision_aware_controller_motion() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_millis(100));
        world.insert_resource(time);

        let entity = world
            .spawn((
                Transform::default(),
                ControlledPlayer,
                PredictedMovement {
                    destination: Some(Vec3::X * 10.0),
                    reconcile_after: 0,
                },
                GameVelocity::default(),
                KinematicCharacterController::default(),
            ))
            .id();

        world.run_system_once(predict_controlled_player).unwrap();

        let transform = world.get::<Transform>(entity).unwrap();
        let controller = world.get::<KinematicCharacterController>(entity).unwrap();
        let movement = controller.translation.unwrap();

        assert_eq!(transform.translation, Vec3::ZERO);
        assert!((movement.x - 1.0).abs() < 0.000_1);
        assert!((movement.y + 0.981).abs() < 0.000_1);
    }
}
