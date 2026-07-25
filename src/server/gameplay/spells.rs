use bevy::prelude::*;
use bevy_rapier3d::prelude::KinematicCharacterController;
use bevy_renet::RenetServer;

use super::pathing::TargetPos;
use crate::shared::{
    gameplay::{
        components::{
            facing_from_direction, Aggro, Attacking, AttackingTimer, Facing, GameVelocity,
            PlayerInput, Walking,
        },
        entities::AttackSpeed,
        spells::{spell_cooldown, spell_definition},
    },
    network::{channels::ServerChannel, messages::ServerMessages},
};

#[derive(Event, Debug, Clone, Copy)]
pub(crate) struct RequestSpellCast {
    pub caster: Entity,
    pub spell_id: u16,
    pub target: Vec3,
}

#[derive(Component, Debug)]
pub(crate) struct AuthoritativeCast {
    spell_id: u16,
    target: Vec3,
    timer: Timer,
}

#[derive(Component, Debug)]
struct SpellCooldown(Timer);

pub(crate) struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_request_spell_cast)
            .add_systems(Update, (tick_authoritative_casts, tick_spell_cooldowns));
    }
}

fn on_request_spell_cast(
    trigger: On<RequestSpellCast>,
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    mut casters: Query<(
        &Transform,
        &AttackSpeed,
        &mut Facing,
        &mut GameVelocity,
        &mut PlayerInput,
        Option<&AuthoritativeCast>,
        Option<&SpellCooldown>,
        Option<&mut KinematicCharacterController>,
    )>,
) {
    let request = trigger.event();
    let Some(definition) = spell_definition(request.spell_id) else {
        warn!("Rejecting unknown spell {}", request.spell_id);
        return;
    };
    if !request.target.is_finite() {
        warn!(
            "Rejecting spell {} with a non-finite target",
            request.spell_id
        );
        return;
    }

    let Ok((
        transform,
        attack_speed,
        mut facing,
        mut velocity,
        mut player_input,
        active_cast,
        cooldown,
        controller,
    )) = casters.get_mut(request.caster)
    else {
        warn!(
            "Rejecting spell cast from invalid caster {:?}",
            request.caster
        );
        return;
    };

    if active_cast.is_some() || cooldown.is_some() {
        info!(
            "Rejecting spell {} from {:?}: casting is locked",
            request.spell_id, request.caster
        );
        return;
    }

    let Some(new_facing) = facing_from_direction(request.target - transform.translation) else {
        info!(
            "Rejecting spell {} from {:?}: target is on the caster",
            request.spell_id, request.caster
        );
        return;
    };

    *facing = new_facing.clone();
    velocity.0 = Vec3::ZERO;
    *player_input = PlayerInput::default();
    if let Some(mut controller) = controller {
        controller.translation = None;
    }

    commands
        .entity(request.caster)
        .remove::<Walking>()
        .remove::<TargetPos>()
        .remove::<Aggro>()
        .remove::<Attacking>()
        .remove::<AttackingTimer>();

    broadcast(
        &mut server,
        ServerMessages::SpellCastStarted {
            entity: request.caster,
            spell_id: request.spell_id,
            target: request.target,
            cast_time_ms: duration_millis(definition.cast_time),
            facing: new_facing,
        },
    );

    if definition.cast_time.is_zero() {
        resolve_spell(
            &mut commands,
            &mut server,
            request.caster,
            request.spell_id,
            request.target,
            attack_speed.0,
        );
    } else {
        commands.entity(request.caster).insert(AuthoritativeCast {
            spell_id: request.spell_id,
            target: request.target,
            timer: Timer::new(definition.cast_time, TimerMode::Once),
        });
    }
}

fn tick_authoritative_casts(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    time: Res<Time>,
    mut casts: Query<(Entity, &AttackSpeed, &mut AuthoritativeCast)>,
) {
    for (caster, attack_speed, mut casting) in &mut casts {
        casting.timer.tick(time.delta());
        if !casting.timer.just_finished() {
            continue;
        }

        resolve_spell(
            &mut commands,
            &mut server,
            caster,
            casting.spell_id,
            casting.target,
            attack_speed.0,
        );
    }
}

fn resolve_spell(
    commands: &mut Commands,
    server: &mut RenetServer,
    caster: Entity,
    spell_id: u16,
    target: Vec3,
    attack_period_seconds: f32,
) {
    let cooldown = spell_cooldown(attack_period_seconds);

    // This is the authoritative spell-resolution boundary. Damage, mana costs,
    // status effects, and spawned server entities belong here as spell
    // definitions gain those properties.
    broadcast(
        server,
        ServerMessages::SpellCastCompleted {
            entity: caster,
            spell_id,
            target,
            cooldown_ms: duration_millis(cooldown),
        },
    );

    commands
        .entity(caster)
        .remove::<AuthoritativeCast>()
        .insert(SpellCooldown(Timer::new(cooldown, TimerMode::Once)));
}

fn tick_spell_cooldowns(
    mut commands: Commands,
    time: Res<Time>,
    mut cooldowns: Query<(Entity, &mut SpellCooldown)>,
) {
    for (entity, mut cooldown) in &mut cooldowns {
        cooldown.0.tick(time.delta());
        if cooldown.0.just_finished() {
            commands.entity(entity).remove::<SpellCooldown>();
        }
    }
}

fn duration_millis(duration: std::time::Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}

fn broadcast(server: &mut RenetServer, message: ServerMessages) {
    let message = bincode::serialize(&message).expect("spell message should serialize");
    server.broadcast_message(ServerChannel::ServerMessages, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::network::channels::connection_config;

    #[test]
    fn duration_conversion_is_in_milliseconds() {
        assert_eq!(duration_millis(std::time::Duration::from_secs(4)), 4_000);
    }

    #[test]
    fn server_accepts_cast_and_locks_caster_movement() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(connection_config()))
            .add_plugins(SpellsPlugin);

        let caster = app
            .world_mut()
            .spawn((
                Transform::from_xyz(2.0, 1.0, 2.0),
                AttackSpeed(0.5),
                Facing(0),
                GameVelocity(Vec3::X),
                PlayerInput::default(),
                Walking {
                    target_translation: Vec3::X * 10.0,
                    path: None,
                },
                TargetPos {
                    position: Vec3::X * 3.0,
                },
                KinematicCharacterController {
                    translation: Some(Vec3::X),
                    ..default()
                },
            ))
            .id();

        app.world_mut().trigger(RequestSpellCast {
            caster,
            spell_id: 2,
            target: Vec3::new(10.0, 1.0, 2.0),
        });
        app.world_mut().flush();

        let caster = app.world().entity(caster);
        assert!(caster.contains::<AuthoritativeCast>());
        assert!(!caster.contains::<Walking>());
        assert!(!caster.contains::<TargetPos>());
        assert_eq!(caster.get::<Facing>(), Some(&Facing(6)));
        assert_eq!(caster.get::<GameVelocity>().unwrap().0, Vec3::ZERO);
        assert_eq!(
            caster
                .get::<KinematicCharacterController>()
                .unwrap()
                .translation,
            None
        );
    }

    #[test]
    fn instant_spell_enters_server_owned_cooldown() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(connection_config()))
            .add_plugins(SpellsPlugin);

        let caster = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.0, 0.0),
                AttackSpeed(0.5),
                Facing(0),
                GameVelocity::default(),
                PlayerInput::default(),
            ))
            .id();

        app.world_mut().trigger(RequestSpellCast {
            caster,
            spell_id: 1,
            target: Vec3::X,
        });
        app.world_mut().flush();

        let caster = app.world().entity(caster);
        assert!(!caster.contains::<AuthoritativeCast>());
        assert!(caster.contains::<SpellCooldown>());
    }
}
