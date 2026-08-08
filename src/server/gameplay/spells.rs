use bevy::prelude::*;
use bevy_rapier3d::prelude::KinematicCharacterController;
use bevy_renet::RenetServer;

use super::pathing::TargetPos;
use super::spawn_protection::SpawnProtection;
use crate::shared::{
    gameplay::{
        components::{
            facing_from_direction, Aggro, Attacking, AttackingTimer, CharacterStats, Dead,
            Equipment, Facing, GameVelocity, Health, Monster, PlayerInput, Walking,
        },
        entities::AttackSpeed,
        events::{DamageOrigin, DirectSpellTargeted, HealthChange, HealthChangeType},
        items::equipment_derived_stats,
        progression::BaseProgression,
        spells::{spell_cooldown, spell_definition, SpellEffect, SpellTargeting},
    },
    network::{channels::ServerChannel, messages::ServerMessages},
};

#[derive(Event, Debug, Clone, Copy)]
pub(crate) struct RequestSpellCast {
    pub caster: Entity,
    pub spell_id: u16,
    pub target: Vec3,
    pub target_entity: Option<Entity>,
}

#[derive(Component, Debug)]
pub(crate) struct AuthoritativeCast {
    spell_id: u16,
    target: Vec3,
    target_entity: Option<Entity>,
    timer: Timer,
}

#[derive(Component, Debug)]
struct SpellCooldown(Timer);

#[derive(Component, Debug)]
struct ActiveSelfBuff {
    timer: Timer,
    original_attack_period: f32,
}

const BASELINE_MAGIC_POWER: u32 = 4;

fn spell_damage_bonus(
    stats: Option<&CharacterStats>,
    progression: Option<&BaseProgression>,
    equipment: Option<&Equipment>,
) -> u32 {
    let Some(stats) = stats else {
        return 0;
    };
    let level = progression.map_or(1, |progression| progression.level);
    equipment
        .map_or_else(
            || stats.derived(level),
            |equipment| equipment_derived_stats(stats, level, equipment),
        )
        .magic_power
        .saturating_sub(BASELINE_MAGIC_POWER)
}

pub(crate) struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_request_spell_cast).add_systems(
            Update,
            (
                tick_authoritative_casts,
                tick_spell_cooldowns,
                tick_self_buffs,
            ),
        );
    }
}

fn on_request_spell_cast(
    trigger: On<RequestSpellCast>,
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    spell_targets: Query<(Entity, &Transform, &Health), With<Monster>>,
    mut casters: Query<
        (
            &Transform,
            &mut AttackSpeed,
            &mut Facing,
            &mut GameVelocity,
            &mut PlayerInput,
            Option<&AuthoritativeCast>,
            Option<&SpellCooldown>,
            Option<&mut ActiveSelfBuff>,
            Option<&mut KinematicCharacterController>,
            Option<&CharacterStats>,
            Option<&BaseProgression>,
            Option<&Equipment>,
        ),
        Without<Dead>,
    >,
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
        mut attack_speed,
        mut facing,
        mut velocity,
        mut player_input,
        active_cast,
        cooldown,
        mut active_self_buff,
        controller,
        stats,
        progression,
        equipment,
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

    let (target, target_entity) = match definition.targeting {
        SpellTargeting::GroundArea => (request.target, None),
        SpellTargeting::DirectMonster => {
            let Some(target_entity) = request.target_entity else {
                info!(
                    "Rejecting direct spell {} from {:?}: no monster was targeted",
                    request.spell_id, request.caster
                );
                return;
            };
            let Ok((_, target_transform, target_health)) = spell_targets.get(target_entity) else {
                info!(
                    "Rejecting direct spell {} from {:?}: target {:?} is not a monster",
                    request.spell_id, request.caster, target_entity
                );
                return;
            };
            if target_health.current == 0 {
                info!(
                    "Rejecting direct spell {} from {:?}: target {:?} is dead",
                    request.spell_id, request.caster, target_entity
                );
                return;
            }
            (target_transform.translation, Some(target_entity))
        }
        SpellTargeting::SelfOnly => (transform.translation, None),
    };

    if definition.max_range.is_some_and(|max_range| {
        let offset = target - transform.translation;
        Vec2::new(offset.x, offset.z).length_squared() > (max_range * max_range) as f32
    }) {
        info!(
            "Rejecting spell {} from {:?}: target is out of range",
            request.spell_id, request.caster
        );
        return;
    }

    let new_facing = if definition.targeting == SpellTargeting::SelfOnly {
        facing.clone()
    } else {
        let Some(new_facing) = facing_from_direction(target - transform.translation) else {
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
        new_facing
    };

    commands
        .entity(request.caster)
        .try_remove::<SpawnProtection>();

    if let Some(monster) = target_entity {
        commands.trigger(DirectSpellTargeted {
            monster,
            caster: request.caster,
        });
    }

    broadcast(
        &mut server,
        ServerMessages::SpellCastStarted {
            entity: request.caster,
            spell_id: request.spell_id,
            target,
            cast_time_ms: duration_millis(definition.cast_time),
            facing: new_facing,
        },
    );

    if definition.cast_time.is_zero() {
        let damage_bonus = spell_damage_bonus(stats, progression, equipment);
        resolve_spell(
            &mut commands,
            &mut server,
            request.caster,
            request.spell_id,
            target,
            target_entity,
            attack_speed.0,
            &mut attack_speed,
            active_self_buff.as_deref_mut(),
            &spell_targets,
            damage_bonus,
        );
    } else {
        commands.entity(request.caster).insert(AuthoritativeCast {
            spell_id: request.spell_id,
            target,
            target_entity,
            timer: Timer::new(definition.cast_time, TimerMode::Once),
        });
    }
}

fn tick_authoritative_casts(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    time: Res<Time>,
    mut casts: Query<
        (
            Entity,
            &mut AttackSpeed,
            Option<&mut ActiveSelfBuff>,
            &mut AuthoritativeCast,
            Option<&CharacterStats>,
            Option<&BaseProgression>,
            Option<&Equipment>,
        ),
        Without<Dead>,
    >,
    spell_targets: Query<(Entity, &Transform, &Health), With<Monster>>,
) {
    for (
        caster,
        mut attack_speed,
        mut active_self_buff,
        mut casting,
        stats,
        progression,
        equipment,
    ) in &mut casts
    {
        casting.timer.tick(time.delta());
        if !casting.timer.just_finished() {
            continue;
        }

        let damage_bonus = spell_damage_bonus(stats, progression, equipment);
        resolve_spell(
            &mut commands,
            &mut server,
            caster,
            casting.spell_id,
            casting.target,
            casting.target_entity,
            attack_speed.0,
            &mut attack_speed,
            active_self_buff.as_deref_mut(),
            &spell_targets,
            damage_bonus,
        );
    }
}

fn resolve_spell(
    commands: &mut Commands,
    server: &mut RenetServer,
    caster: Entity,
    spell_id: u16,
    target: Vec3,
    target_entity: Option<Entity>,
    attack_period_seconds: f32,
    attack_speed: &mut AttackSpeed,
    active_self_buff: Option<&mut ActiveSelfBuff>,
    spell_targets: &Query<(Entity, &Transform, &Health), With<Monster>>,
    damage_bonus: u32,
) {
    let cooldown = spell_cooldown(attack_period_seconds);
    let Some(definition) = spell_definition(spell_id) else {
        return;
    };
    let resolved_target = target_entity
        .and_then(|entity| {
            spell_targets
                .get(entity)
                .ok()
                .map(|(_, transform, _)| transform.translation)
        })
        .unwrap_or(target);

    match definition.effect {
        SpellEffect::None => {}
        SpellEffect::Damage {
            amount,
            area_radius,
        } => {
            let amount = amount.saturating_add(damage_bonus);
            if let Some(area_radius) = area_radius {
                let radius_squared = (area_radius * area_radius) as f32;
                for (monster, transform, health) in spell_targets.iter() {
                    let offset = transform.translation - resolved_target;
                    if health.current > 0
                        && Vec2::new(offset.x, offset.z).length_squared() <= radius_squared
                    {
                        commands.trigger(HealthChange {
                            entity: monster,
                            source: Some(caster),
                            amount: amount as i32,
                            damage: amount,
                            damage_type: HealthChangeType::Normal,
                            origin: DamageOrigin::AreaSpell,
                        });
                    }
                }
            } else if let Some(target_entity) = target_entity {
                if spell_targets
                    .get(target_entity)
                    .is_ok_and(|(_, _, health)| health.current > 0)
                {
                    commands.trigger(HealthChange {
                        entity: target_entity,
                        source: Some(caster),
                        amount: amount as i32,
                        damage: amount,
                        damage_type: HealthChangeType::Normal,
                        origin: DamageOrigin::DirectSpell,
                    });
                }
            }
        }
        SpellEffect::AttackSpeedBuff {
            duration,
            attack_period_percent,
        } => {
            if let Some(buff) = active_self_buff {
                buff.timer = Timer::new(duration, TimerMode::Once);
            } else {
                let original_attack_period = attack_speed.0;
                attack_speed.0 *= f32::from(attack_period_percent) / 100.0;
                commands.entity(caster).insert(ActiveSelfBuff {
                    timer: Timer::new(duration, TimerMode::Once),
                    original_attack_period,
                });
            }
        }
    }

    // This is the authoritative spell-resolution boundary. Damage, mana costs,
    // status effects, and spawned server entities belong here as spell
    // definitions gain those properties.
    broadcast(
        server,
        ServerMessages::SpellCastCompleted {
            entity: caster,
            spell_id,
            target: resolved_target,
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

fn tick_self_buffs(
    mut commands: Commands,
    time: Res<Time>,
    mut buffs: Query<(Entity, &mut AttackSpeed, &mut ActiveSelfBuff)>,
) {
    for (entity, mut attack_speed, mut buff) in &mut buffs {
        buff.timer.tick(time.delta());
        if buff.timer.just_finished() {
            attack_speed.0 = buff.original_attack_period;
            commands.entity(entity).remove::<ActiveSelfBuff>();
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
    use crate::shared::gameplay::items::APPRENTICE_STAFF;
    use crate::shared::network::channels::connection_config;
    use bevy::time::TimeUpdateStrategy;

    #[derive(Resource, Default)]
    struct ObservedDamage(Option<(Entity, Entity, u32, DamageOrigin)>);

    #[derive(Resource, Default)]
    struct ObservedAreaDamage(Vec<(Entity, Entity, u32, DamageOrigin)>);

    fn record_damage(trigger: On<HealthChange>, mut observed: ResMut<ObservedDamage>) {
        let event = trigger.event();
        observed.0 = event
            .source
            .map(|source| (event.entity, source, event.damage, event.origin));
    }

    fn record_area_damage(trigger: On<HealthChange>, mut observed: ResMut<ObservedAreaDamage>) {
        let event = trigger.event();
        if event.origin == DamageOrigin::AreaSpell {
            if let Some(source) = event.source {
                observed
                    .0
                    .push((event.entity, source, event.damage, event.origin));
            }
        }
    }

    #[test]
    fn apprentice_staff_increases_spell_damage_bonus() {
        let stats = CharacterStats::default();
        let progression = BaseProgression::default();
        let mut equipment = Equipment::default();
        equipment.set(
            crate::shared::gameplay::components::EquipmentSlot::MainHand,
            Some(APPRENTICE_STAFF),
        );

        assert_eq!(
            spell_damage_bonus(Some(&stats), Some(&progression), Some(&equipment)),
            5
        );
        assert_eq!(spell_damage_bonus(None, None, None), 0);
    }

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
                SpawnProtection::default(),
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
            target_entity: None,
        });
        app.world_mut().flush();

        let caster = app.world().entity(caster);
        assert!(caster.contains::<AuthoritativeCast>());
        assert!(!caster.contains::<SpawnProtection>());
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
    fn server_rejects_spell_casts_from_dead_players() {
        let mut app = App::new();
        app.insert_resource(RenetServer::new(connection_config()))
            .add_plugins(SpellsPlugin);

        let caster = app
            .world_mut()
            .spawn((
                Transform::default(),
                AttackSpeed(0.5),
                Facing(0),
                GameVelocity::default(),
                PlayerInput::default(),
                Dead,
            ))
            .id();

        app.world_mut().trigger(RequestSpellCast {
            caster,
            spell_id: 2,
            target: Vec3::X,
            target_entity: None,
        });
        app.world_mut().flush();

        assert!(!app.world().entity(caster).contains::<AuthoritativeCast>());
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
            target_entity: None,
        });
        app.world_mut().flush();

        let caster = app.world().entity(caster);
        assert!(!caster.contains::<AuthoritativeCast>());
        assert!(caster.contains::<SpellCooldown>());
    }

    #[test]
    fn second_spell_damages_each_living_monster_inside_its_ground_area() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(4),
            ))
            .insert_resource(RenetServer::new(connection_config()))
            .init_resource::<ObservedAreaDamage>()
            .add_plugins(SpellsPlugin)
            .add_observer(record_area_damage);

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
        let spawn_monster = |world: &mut World, translation: Vec3, current_health: u32| {
            world
                .spawn((
                    Monster {
                        hp: 100,
                        kind: crate::shared::gameplay::components::MonsterKind::Pig,
                    },
                    Transform::from_translation(translation),
                    Health {
                        current: current_health,
                        max: 100,
                    },
                ))
                .id()
        };
        let center = spawn_monster(app.world_mut(), Vec3::new(5.0, 1.0, 0.0), 100);
        let edge = spawn_monster(app.world_mut(), Vec3::new(8.0, 1.0, 0.0), 100);
        let outside = spawn_monster(app.world_mut(), Vec3::new(8.1, 1.0, 0.0), 100);
        let dead = spawn_monster(app.world_mut(), Vec3::new(5.0, 1.0, 1.0), 0);

        app.update();
        app.world_mut()
            .resource_mut::<Time<bevy::time::Virtual>>()
            .set_max_delta(std::time::Duration::from_secs(4));
        app.world_mut().trigger(RequestSpellCast {
            caster,
            spell_id: 2,
            target: Vec3::new(5.0, 1.0, 0.0),
            target_entity: None,
        });
        app.world_mut().flush();
        app.update();

        let observed = &app.world().resource::<ObservedAreaDamage>().0;
        assert_eq!(observed.len(), 2);
        assert!(observed.contains(&(center, caster, 15, DamageOrigin::AreaSpell)));
        assert!(observed.contains(&(edge, caster, 15, DamageOrigin::AreaSpell)));
        assert!(!observed
            .iter()
            .any(|(entity, _, _, _)| { *entity == outside || *entity == dead }));
    }

    #[test]
    fn fourth_spell_buffs_self_instantly_without_interrupting_movement_then_expires() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(10),
            ))
            .insert_resource(RenetServer::new(connection_config()))
            .add_plugins(SpellsPlugin);

        let caster = app
            .world_mut()
            .spawn((
                Transform::from_xyz(2.0, 1.0, 2.0),
                AttackSpeed(1.0),
                Facing(6),
                GameVelocity(Vec3::X),
                PlayerInput::default(),
                Walking {
                    target_translation: Vec3::X * 10.0,
                    path: None,
                },
            ))
            .id();

        app.update();
        app.world_mut()
            .resource_mut::<Time<bevy::time::Virtual>>()
            .set_max_delta(std::time::Duration::from_secs(10));
        app.world_mut().trigger(RequestSpellCast {
            caster,
            spell_id: 4,
            target: Vec3::splat(999.0),
            target_entity: None,
        });
        app.world_mut().flush();

        let caster_state = app.world().entity(caster);
        assert!((caster_state.get::<AttackSpeed>().unwrap().0 - 0.7).abs() < f32::EPSILON);
        assert!(caster_state.contains::<ActiveSelfBuff>());
        assert!(caster_state.contains::<Walking>());
        assert_eq!(caster_state.get::<GameVelocity>().unwrap().0, Vec3::X);
        assert_eq!(caster_state.get::<Facing>(), Some(&Facing(6)));

        app.update();

        let caster_state = app.world().entity(caster);
        assert_eq!(caster_state.get::<AttackSpeed>().unwrap().0, 1.0);
        assert!(!caster_state.contains::<ActiveSelfBuff>());
    }

    #[test]
    fn third_spell_hits_its_monster_target_for_twenty_after_three_seconds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(3),
            ))
            .insert_resource(RenetServer::new(connection_config()))
            .init_resource::<ObservedDamage>()
            .add_plugins(SpellsPlugin)
            .add_observer(record_damage);

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
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: crate::shared::gameplay::components::MonsterKind::Pig,
                },
                Transform::from_xyz(3.0, 1.0, 0.0),
                Health {
                    current: 100,
                    max: 100,
                },
            ))
            .id();

        // Initialize Bevy's virtual clock before beginning the measured cast.
        app.update();
        app.world_mut()
            .resource_mut::<Time<bevy::time::Virtual>>()
            .set_max_delta(std::time::Duration::from_secs(3));
        app.world_mut().trigger(RequestSpellCast {
            caster,
            spell_id: 3,
            target: Vec3::ZERO,
            target_entity: Some(monster),
        });
        app.world_mut().flush();

        assert!(app.world().entity(caster).contains::<AuthoritativeCast>());
        assert!(app.world().resource::<ObservedDamage>().0.is_none());

        app.update();

        assert_eq!(
            app.world().resource::<ObservedDamage>().0,
            Some((monster, caster, 20, DamageOrigin::DirectSpell))
        );
        assert!(!app.world().entity(caster).contains::<AuthoritativeCast>());
        assert!(app.world().entity(caster).contains::<SpellCooldown>());
    }
}
