use bevy::prelude::*;

use crate::shared::gameplay::components::GameVelocity;

#[derive(Debug, Component)]
pub struct Projectile {
    pub duration: Timer,
}

pub fn spawn_fireball(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    translation: Vec3,
    mut direction: Vec3,
) -> Entity {
    if !direction.is_normalized() {
        direction = Vec3::X;
    }

    commands
        .spawn((
            Mesh3d(meshes.add(Sphere { radius: 0.1 })),
            MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
            Transform::from_translation(translation),
            /*PbrBundle {
                mesh: meshes.add(Sphere { radius: 0.1 }),
                material: materials.add(Color::srgb(1.0, 0.0, 0.0)),
                transform: Transform::from_translation(translation),
                ..Default::default()
            }*/
        ))
        .insert(GameVelocity(direction * 10.))
        .insert(Projectile {
            duration: Timer::from_seconds(1.5, TimerMode::Once),
        })
        .id()
}
