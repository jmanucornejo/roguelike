use bevy::prelude::*;
use bevy_renet::RenetServer;

use crate::{
    server::network::replication::LineOfSight,
    shared::{
        gameplay::{
            components::{Dead, Health, Mana, Sitting},
            entities::Player,
        },
        network::{channels::ServerChannel, messages::ServerMessages},
        states::ServerState,
    },
};

/// One natural-recovery tick is earned after this much standing time.
pub const PASSIVE_REGEN_INTERVAL_SECONDS: f32 = 5.0;
/// Sitting advances the same recovery clock at twice the standing rate.
pub const SITTING_REGEN_SPEED_MULTIPLIER: f32 = 2.0;
/// Each tick restores this percentage of the resource maximum.
pub const PASSIVE_REGEN_PERCENT: u32 = 1;

#[derive(Component, Debug, Default)]
pub struct PassiveRegeneration {
    progress_seconds: f32,
}

fn regeneration_amount(maximum: u32) -> u32 {
    maximum
        .saturating_mul(PASSIVE_REGEN_PERCENT)
        .div_ceil(100)
        .max(1)
}

fn restore_resource(current: u32, maximum: u32, amount: u32, ticks: u32) -> u32 {
    current
        .saturating_add(amount.saturating_mul(ticks))
        .min(maximum)
}

fn advance_regeneration(
    regeneration: &mut PassiveRegeneration,
    health: &Health,
    mana: &Mana,
    delta_seconds: f32,
    sitting: bool,
) -> (u32, u32, u32) {
    // Zero HP must never be passively raised before the deferred Dead marker is
    // applied. Respawning remains an explicit player action.
    if health.current == 0 {
        return (0, health.current, mana.current);
    }

    let speed = if sitting {
        SITTING_REGEN_SPEED_MULTIPLIER
    } else {
        1.0
    };
    regeneration.progress_seconds += delta_seconds.max(0.0) * speed;

    let ticks = (regeneration.progress_seconds / PASSIVE_REGEN_INTERVAL_SECONDS).floor() as u32;
    if ticks == 0 {
        return (0, health.current, mana.current);
    }
    regeneration.progress_seconds -= ticks as f32 * PASSIVE_REGEN_INTERVAL_SECONDS;

    (
        ticks,
        restore_resource(
            health.current,
            health.max,
            regeneration_amount(health.max),
            ticks,
        ),
        restore_resource(mana.current, mana.max, regeneration_amount(mana.max), ticks),
    )
}

fn regenerate_players(
    time: Res<Time>,
    mut players: Query<
        (
            &mut PassiveRegeneration,
            &mut Health,
            &mut Mana,
            Has<Sitting>,
        ),
        (With<Player>, Without<Dead>),
    >,
) {
    for (mut regeneration, mut health, mut mana, sitting) in &mut players {
        let (_, recovered_health, recovered_mana) = advance_regeneration(
            &mut regeneration,
            &health,
            &mana,
            time.delta_secs(),
            sitting,
        );
        if health.current != recovered_health {
            health.current = recovered_health;
        }
        if mana.current != recovered_mana {
            mana.current = recovered_mana;
        }
    }
}

fn send_changed_mana(
    mut server: ResMut<RenetServer>,
    players: Query<(Entity, &Player, &LineOfSight)>,
    changed_mana: Query<(Entity, &Mana), Changed<Mana>>,
) {
    for (entity, mana) in &changed_mana {
        let message = bincode::serialize(&ServerMessages::ManaChange {
            entity,
            max: mana.max,
            current: mana.current,
        })
        .expect("mana update should serialize");

        for (viewer_entity, player, line_of_sight) in &players {
            if viewer_entity == entity || line_of_sight.0.contains(&entity) {
                server.send_message(player.id, ServerChannel::ServerMessages, message.clone());
            }
        }
    }
}

pub struct RegenerationPlugin;

impl Plugin for RegenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                regenerate_players.run_if(in_state(ServerState::InGame)),
                send_changed_mana
                    .run_if(in_state(ServerState::InGame))
                    .after(regenerate_players),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resources() -> (Health, Mana) {
        (
            Health {
                current: 50,
                max: 100,
            },
            Mana {
                current: 10,
                max: 100,
            },
        )
    }

    #[test]
    fn standing_regenerates_one_percent_every_five_seconds() {
        let mut regeneration = PassiveRegeneration::default();
        let (mut health, mut mana) = resources();

        assert_eq!(
            advance_regeneration(&mut regeneration, &health, &mana, 4.99, false),
            (0, 50, 10)
        );
        let recovery = advance_regeneration(&mut regeneration, &health, &mana, 0.01, false);
        assert_eq!(recovery, (1, 51, 11));
        health.current = recovery.1;
        mana.current = recovery.2;
        assert_eq!((health.current, mana.current), (51, 11));
    }

    #[test]
    fn sitting_accumulates_recovery_twice_as_fast_without_resetting_progress() {
        let mut regeneration = PassiveRegeneration::default();
        let (mut health, mut mana) = resources();

        assert_eq!(
            advance_regeneration(&mut regeneration, &health, &mana, 2.0, false),
            (0, 50, 10)
        );
        let recovery = advance_regeneration(&mut regeneration, &health, &mana, 1.5, true);
        assert_eq!(recovery, (1, 51, 11));
        health.current = recovery.1;
        mana.current = recovery.2;
        assert_eq!((health.current, mana.current), (51, 11));
    }

    #[test]
    fn recovery_rounds_up_caps_at_maximum_and_never_revives() {
        assert_eq!(regeneration_amount(10), 1);
        assert_eq!(regeneration_amount(101), 2);

        let mut regeneration = PassiveRegeneration::default();
        let mut health = Health {
            current: 100,
            max: 101,
        };
        let mut mana = Mana {
            current: 9,
            max: 10,
        };
        assert_eq!(
            advance_regeneration(&mut regeneration, &health, &mana, 5.0, false),
            (1, 101, 10)
        );

        health.current = 0;
        mana.current = 0;
        assert_eq!(
            advance_regeneration(&mut regeneration, &health, &mana, 100.0, true),
            (0, 0, 0)
        );
        assert_eq!((health.current, mana.current), (0, 0));
    }
}
