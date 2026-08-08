use std::time::Duration;

use bevy::prelude::*;

/// rAthena's default protection after entering a map or being revived.
pub const SPAWN_PROTECTION_DURATION: Duration = Duration::from_millis(3_000);

/// Server-authoritative immunity granted when a living player enters play.
///
/// Walking, attacking, or casting removes this component before the timer
/// expires, matching Ragnarok Online's spawn-invincibility behavior.
#[derive(Component, Debug)]
pub struct SpawnProtection {
    timer: Timer,
}

impl Default for SpawnProtection {
    fn default() -> Self {
        Self {
            timer: Timer::new(SPAWN_PROTECTION_DURATION, TimerMode::Once),
        }
    }
}

impl SpawnProtection {
    fn tick(&mut self, delta: Duration) -> bool {
        self.timer.tick(delta).just_finished()
    }
}

fn expire_spawn_protection(
    time: Res<Time>,
    mut protected_players: Query<(Entity, &mut SpawnProtection)>,
    mut commands: Commands,
) {
    for (entity, mut protection) in &mut protected_players {
        if protection.tick(time.delta()) {
            commands.entity(entity).try_remove::<SpawnProtection>();
        }
    }
}

pub struct SpawnProtectionPlugin;

impl Plugin for SpawnProtectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, expire_spawn_protection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ragnarok_spawn_protection_expires_after_three_seconds() {
        let mut protection = SpawnProtection::default();

        assert!(!protection.tick(Duration::from_millis(2_999)));
        assert!(protection.tick(Duration::from_millis(1)));
    }
}
