use bevy::prelude::*;

use crate::shared::gameplay::components::GameVelocity;

#[derive(Debug, Component)]
pub struct Projectile {
    pub duration: Timer,
}

pub fn spawn_fireball(commands: &mut Commands, translation: Vec3, mut direction: Vec3) -> Entity {
    if !direction.is_normalized() {
        direction = Vec3::X;
    }

    commands
        .spawn(Transform::from_translation(translation))
        .insert(GameVelocity(direction * 10.))
        .insert(Projectile {
            duration: Timer::from_seconds(1.5, TimerMode::Once),
        })
        .id()
}
