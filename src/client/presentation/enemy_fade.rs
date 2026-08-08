use bevy::prelude::*;
use std::collections::HashSet;

use crate::{client::state::NetworkMapping, shared::states::ClientState};

pub(crate) const ENEMY_FADE_OUT_SECONDS: f32 = 0.35;

#[derive(Default, Resource)]
pub(crate) struct EnemyVisibility {
    known_enemies: HashSet<Entity>,
    visible_enemies: HashSet<Entity>,
}

impl EnemyVisibility {
    pub(crate) fn mark_visible(&mut self, server_entity: Entity) {
        self.known_enemies.insert(server_entity);
        self.visible_enemies.insert(server_entity);
    }

    /// Returns true only for the visible-to-hidden transition. Duplicate
    /// despawns must not restart the fade timer.
    pub(crate) fn mark_hidden(&mut self, server_entity: Entity) -> bool {
        self.known_enemies.contains(&server_entity) && self.visible_enemies.remove(&server_entity)
    }

    pub(crate) fn is_known_enemy(&self, server_entity: Entity) -> bool {
        self.known_enemies.contains(&server_entity)
    }

    fn is_visible(&self, server_entity: Entity) -> bool {
        self.visible_enemies.contains(&server_entity)
    }

    fn forget(&mut self, server_entity: Entity) {
        self.known_enemies.remove(&server_entity);
        self.visible_enemies.remove(&server_entity);
    }

    pub(crate) fn clear(&mut self) {
        self.known_enemies.clear();
        self.visible_enemies.clear();
    }
}

#[derive(Component)]
pub(crate) struct EnemyFadeOut {
    server_entity: Entity,
    timer: Timer,
}

impl EnemyFadeOut {
    pub(crate) fn new(server_entity: Entity) -> Self {
        Self {
            server_entity,
            timer: Timer::from_seconds(ENEMY_FADE_OUT_SECONDS, TimerMode::Once),
        }
    }
}

pub(crate) struct EnemyFadePlugin;

impl Plugin for EnemyFadePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyVisibility>().add_systems(
            Update,
            fade_out_enemies.run_if(in_state(ClientState::InGame)),
        );
    }
}

fn fade_alpha(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - progress * progress * (3.0 - 2.0 * progress)
}

fn fade_out_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut network_mapping: ResMut<NetworkMapping>,
    mut visibility: ResMut<EnemyVisibility>,
    mut enemies: Query<(
        Entity,
        &mut EnemyFadeOut,
        Option<&mut Sprite>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
) {
    for (entity, mut fade, sprite, material_handle) in &mut enemies {
        if visibility.is_visible(fade.server_entity) {
            if let Some(mut sprite) = sprite {
                sprite.color.set_alpha(1.0);
            }
            if let Some(material_handle) = material_handle {
                if let Some(mut material) = materials.get_mut(&material_handle.0) {
                    material.base_color.set_alpha(1.0);
                }
            }
            commands.entity(entity).remove::<EnemyFadeOut>();
            continue;
        }

        fade.timer.tick(time.delta());
        let alpha = fade_alpha(fade.timer.fraction());

        if let Some(mut sprite) = sprite {
            sprite.color.set_alpha(alpha);
        }
        if let Some(material_handle) = material_handle {
            if let Some(mut material) = materials.get_mut(&material_handle.0) {
                material.alpha_mode = AlphaMode::Blend;
                material.base_color.set_alpha(alpha);
            }
        }

        if fade.timer.is_finished() {
            if network_mapping.0.get(&fade.server_entity) == Some(&entity) {
                network_mapping.0.remove(&fade.server_entity);
            }
            visibility.forget(fade.server_entity);
            commands.entity(entity).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemy_fade_is_opaque_at_the_start_and_invisible_at_the_end() {
        assert_eq!(fade_alpha(0.0), 1.0);
        assert_eq!(fade_alpha(1.0), 0.0);
        assert!(fade_alpha(0.25) > fade_alpha(0.75));
    }

    #[test]
    fn duplicate_hides_do_not_restart_a_fade_and_reentry_cancels_it() {
        let server_entity = Entity::from_bits(42);
        let mut visibility = EnemyVisibility::default();

        visibility.mark_visible(server_entity);
        assert!(visibility.mark_hidden(server_entity));
        assert!(!visibility.mark_hidden(server_entity));

        visibility.mark_visible(server_entity);
        assert!(visibility.is_visible(server_entity));
    }
}
