use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier3d::prelude::KinematicCharacterController;

use crate::client::network::movement::PredictedMovement;
use crate::client::presentation::animations::LastAnimationDirection;
use crate::client::presentation::health_bars::{
    BarHeight, BarSettings, ColorScheme, ForegroundColor, HealthBarPlugin, Percentage,
};
use crate::client::presentation::spells::CastSpell;
use crate::client::state::{ControlledPlayer, LocalPlayerInput};
use crate::shared::gameplay::components::{
    facing_from_direction, world_direction_from_facing, Animation, Facing, GameVelocity,
    PlayerInput,
};
use crate::shared::network::messages::PlayerCommand;
use crate::shared::states::ClientState;

const CAST_BAR_WORLD_OFFSET: f32 = 1.65;

#[derive(Event, Debug, Clone, Copy)]
pub(crate) struct RequestSpellCast {
    pub(crate) spell_id: u16,
    pub(crate) translation: Vec3,
}

#[derive(Event, Debug, Clone)]
pub(crate) struct ConfirmedSpellCastStarted {
    pub(crate) entity: Entity,
    pub(crate) spell_id: u16,
    pub(crate) target: Vec3,
    pub(crate) cast_time: Duration,
    pub(crate) facing: Facing,
}

#[derive(Event, Debug, Clone, Copy)]
pub(crate) struct ConfirmedSpellCastCompleted {
    pub(crate) entity: Entity,
    pub(crate) spell_id: u16,
    pub(crate) target: Vec3,
    pub(crate) cooldown: Duration,
}

#[derive(Component, Debug)]
pub(crate) struct CastingSpell {
    timer: Timer,
}

#[derive(Component, Debug)]
struct CastCooldown(Timer);

impl CastingSpell {
    fn new(duration: Duration) -> Self {
        Self {
            timer: Timer::new(duration, TimerMode::Once),
        }
    }
}

impl Percentage for CastingSpell {
    fn value(&self) -> f32 {
        self.timer.fraction()
    }
}

pub(crate) struct CastingPlugin;

impl Plugin for CastingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HealthBarPlugin::<CastingSpell>::default())
            .insert_resource(
                ColorScheme::<CastingSpell>::new()
                    .foreground_color(ForegroundColor::Static(Color::srgb(0.28, 0.62, 1.0))),
            )
            .add_systems(
                Update,
                (tick_casts, tick_cast_cooldowns).run_if(in_state(ClientState::InGame)),
            )
            .add_observer(on_request_spell_cast)
            .add_observer(on_confirmed_spell_cast_started)
            .add_observer(on_confirmed_spell_cast_completed);
    }
}

fn on_request_spell_cast(
    trigger: On<RequestSpellCast>,
    mut player_input: ResMut<LocalPlayerInput>,
    mut player_commands: MessageWriter<PlayerCommand>,
    mut controlled_player: Query<
        (
            Entity,
            Option<&CastingSpell>,
            Option<&CastCooldown>,
            &Transform,
            &mut Facing,
            &mut GameVelocity,
            &mut Animation,
            Option<&mut PredictedMovement>,
            Option<&mut KinematicCharacterController>,
        ),
        With<ControlledPlayer>,
    >,
) {
    let request = trigger.event();
    let Ok((
        _player_entity,
        active_cast,
        cooldown,
        transform,
        mut facing,
        mut velocity,
        mut animation,
        prediction,
        controller,
    )) = controlled_player.single_mut()
    else {
        warn!("Ignoring spell cast because there is no controlled player");
        return;
    };

    if active_cast.is_some() || cooldown.is_some() {
        info!("Ignoring spell cast because casting is currently locked");
        return;
    }

    if let Some(new_facing) = facing_from_direction(request.translation - transform.translation) {
        *facing = new_facing;
    }

    // This is presentation-side prediction only. The server validates the
    // request and owns the actual movement lock, facing, timer, and cooldown.
    **player_input = PlayerInput::default();
    velocity.0 = Vec3::ZERO;
    if let Some(mut prediction) = prediction {
        prediction.destination = None;
    }
    if let Some(mut controller) = controller {
        controller.translation = None;
    }
    *animation = Animation::Idle;

    player_commands.write(PlayerCommand::Cast {
        spell_id: request.spell_id,
        cast_at: request.translation,
    });
}

fn on_confirmed_spell_cast_started(
    trigger: On<ConfirmedSpellCastStarted>,
    mut commands: Commands,
    mut casters: Query<(&mut Facing, &mut Animation, &mut GameVelocity)>,
) {
    let confirmed = trigger.event();
    let Ok((mut facing, mut animation, mut velocity)) = casters.get_mut(confirmed.entity) else {
        return;
    };

    *facing = confirmed.facing.clone();
    velocity.0 = Vec3::ZERO;
    commands
        .entity(confirmed.entity)
        .insert(LastAnimationDirection(world_direction_from_facing(
            confirmed.facing.0,
        )));
    if confirmed.cast_time.is_zero() {
        return;
    }

    *animation = Animation::Casting;
    commands.entity(confirmed.entity).insert((
        CastingSpell::new(confirmed.cast_time),
        BarSettings::<CastingSpell> {
            offset: CAST_BAR_WORLD_OFFSET,
            width: 1.5,
            height: BarHeight::Static(0.1),
            foreground_color: Some(Color::srgb(0.28, 0.62, 1.0)),
            screen_anchor_offset: Some(CAST_BAR_WORLD_OFFSET),
            ..default()
        },
    ));
}

fn on_confirmed_spell_cast_completed(
    trigger: On<ConfirmedSpellCastCompleted>,
    mut commands: Commands,
    mut casters: Query<(&mut Animation, Option<&ControlledPlayer>)>,
) {
    let confirmed = trigger.event();
    commands.trigger(CastSpell {
        spell_id: confirmed.spell_id,
        translation: confirmed.target,
    });

    if let Ok((mut animation, controlled)) = casters.get_mut(confirmed.entity) {
        *animation = Animation::Idle;
        let mut caster = commands.entity(confirmed.entity);
        caster
            .remove::<CastingSpell>()
            .remove::<BarSettings<CastingSpell>>();
        if controlled.is_some() {
            caster.insert(CastCooldown(Timer::new(
                confirmed.cooldown,
                TimerMode::Once,
            )));
        }
    }
}

fn tick_casts(time: Res<Time>, mut casts: Query<&mut CastingSpell>) {
    for mut casting in &mut casts {
        casting.timer.tick(time.delta());
    }
}

fn tick_cast_cooldowns(
    mut commands: Commands,
    time: Res<Time>,
    mut cooldowns: Query<(Entity, &mut CastCooldown), With<ControlledPlayer>>,
) {
    for (player_entity, mut cooldown) in &mut cooldowns {
        cooldown.0.tick(time.delta());
        if cooldown.0.just_finished() {
            commands.entity(player_entity).remove::<CastCooldown>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casting_progress_tracks_elapsed_time() {
        let mut casting = CastingSpell::new(Duration::from_secs(4));
        casting.timer.tick(Duration::from_secs(1));

        assert!((casting.value() - 0.25).abs() < f32::EPSILON);
    }
}
