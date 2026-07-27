use super::pathing::{get_path_between_translations, TargetPos};
use crate::{
    server::network::replication::LineOfSight,
    shared::{
        constants::ATTACK_HIT_FRACTION,
        gameplay::components::*,
        gameplay::entities::{AttackSpeed, MapEntity, Player, NPC},
        gameplay::progression::{BaseProgression, ExperienceReward},
        network::{channels::ServerChannel, messages::ServerMessages},
    },
};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy_renet::RenetServer;
use rand::Rng;
use std::time::Duration;
// use avian3d::{parry::shape, prelude::*};
use crate::shared::gameplay::events::*;
use crate::shared::states::ServerState;

#[derive(Event)]
struct AttackAnimation {
    entity: Entity,
    enemy: Entity,
    attack_speed: f32,
    auto_attack: bool,
}

const BASIC_ATTACK_DAMAGE_MIN: u32 = 4;
const BASIC_ATTACK_DAMAGE_MAX: u32 = 7;
const MONSTER_AGGRO_DETECTION_RANGE: f32 = 8.0;

fn roll_basic_attack_damage(rng: &mut impl Rng) -> u32 {
    rng.gen_range(BASIC_ATTACK_DAMAGE_MIN..=BASIC_ATTACK_DAMAGE_MAX)
}

fn aggression_reacts_to(aggression: MonsterAggression, _damage_origin: DamageOrigin) -> bool {
    match aggression {
        MonsterAggression::Passive
        | MonsterAggression::Aggressive
        | MonsterAggression::SpellReactive => true,
    }
}

fn is_within_monster_vision(monster: Vec3, player: Vec3) -> bool {
    let offset = player - monster;
    Vec2::new(offset.x, offset.z).length_squared()
        <= MONSTER_AGGRO_DETECTION_RANGE * MONSTER_AGGRO_DETECTION_RANGE
}

fn provoke_monster_on_damage(
    trigger: On<HealthChange>,
    monsters: Query<(&MonsterAggression, &Health, &Transform), With<Monster>>,
    player_sources: Query<&Transform, With<Player>>,
    mut commands: Commands,
) {
    let damage = trigger.event();
    let Ok((aggression, health, monster_transform)) = monsters.get(damage.entity) else {
        return;
    };
    if health.current == 0 || !aggression_reacts_to(*aggression, damage.origin) {
        return;
    }
    let Some(source) = damage.source else {
        return;
    };
    let Ok(source_transform) = player_sources.get(source) else {
        return;
    };
    if !is_within_monster_vision(monster_transform.translation, source_transform.translation) {
        return;
    }

    commands
        .entity(damage.entity)
        .try_remove::<Walking>()
        .try_remove::<TargetPos>()
        .try_remove::<Attacking>()
        .try_remove::<AttackingTimer>()
        .try_insert(Aggro {
            enemy: source,
            auto_attack: true,
            enemy_translation: source_transform.translation,
        });
}

fn provoke_spell_reactive_monster(
    trigger: On<DirectSpellTargeted>,
    monsters: Query<(&MonsterAggression, &Health, &Transform), With<Monster>>,
    player_sources: Query<&Transform, With<Player>>,
    mut commands: Commands,
) {
    let targeted = trigger.event();
    let Ok((aggression, health, monster_transform)) = monsters.get(targeted.monster) else {
        return;
    };
    if *aggression != MonsterAggression::SpellReactive || health.current == 0 {
        return;
    }
    let Ok(caster_transform) = player_sources.get(targeted.caster) else {
        return;
    };
    if !is_within_monster_vision(monster_transform.translation, caster_transform.translation) {
        return;
    }

    commands
        .entity(targeted.monster)
        .try_remove::<Walking>()
        .try_remove::<TargetPos>()
        .try_remove::<Attacking>()
        .try_remove::<AttackingTimer>()
        .try_insert(Aggro {
            enemy: targeted.caster,
            auto_attack: true,
            enemy_translation: caster_transform.translation,
        });
}

fn acquire_aggressive_monster_targets(
    monsters: Query<
        (
            Entity,
            &Transform,
            &MonsterAggression,
            &Health,
            Option<&Aggro>,
        ),
        With<Monster>,
    >,
    players: Query<(Entity, &Transform, &Health), With<Player>>,
    mut commands: Commands,
) {
    let detection_range_squared = MONSTER_AGGRO_DETECTION_RANGE * MONSTER_AGGRO_DETECTION_RANGE;
    for (monster_entity, monster_transform, aggression, health, current_aggro) in &monsters {
        if *aggression != MonsterAggression::Aggressive
            || health.current == 0
            || current_aggro.is_some()
        {
            continue;
        }

        let closest = players
            .iter()
            .filter(|(_, _, player_health)| player_health.current > 0)
            .filter_map(|(player_entity, player_transform, _)| {
                let offset = player_transform.translation - monster_transform.translation;
                let distance_squared = Vec2::new(offset.x, offset.z).length_squared();
                (distance_squared <= detection_range_squared).then_some((
                    player_entity,
                    player_transform.translation,
                    distance_squared,
                ))
            })
            .min_by(|left, right| left.2.total_cmp(&right.2));

        let Some((player_entity, player_translation, _)) = closest else {
            continue;
        };
        commands
            .entity(monster_entity)
            .try_remove::<Walking>()
            .try_remove::<TargetPos>()
            .try_insert(Aggro {
                enemy: player_entity,
                auto_attack: true,
                enemy_translation: player_translation,
            });
    }
}

fn attack_cycle_timer(attack_period: f32, auto_attack: bool) -> Timer {
    let attack_period = attack_period.max(0.001);
    let mode = if auto_attack {
        TimerMode::Repeating
    } else {
        TimerMode::Once
    };
    let mut timer = Timer::from_seconds(attack_period, mode);

    // Prime the cycle so the first completion occurs when frame 5 begins.
    let pre_hit_elapsed = attack_period * (1.0 - ATTACK_HIT_FRACTION);
    timer.set_elapsed(Duration::from_secs_f32(pre_hit_elapsed));
    timer
}

fn on_death_give_experience(
    trigger: On<DeathEvent>,
    rewards: Query<&ExperienceReward, With<Monster>>,
    mut players: Query<&mut BaseProgression, With<Player>>,
) {
    let death_event = trigger.event();
    let Ok(reward) = rewards.get(death_event.entity) else {
        return;
    };
    let Some(killer) = death_event.killer else {
        return;
    };
    let Ok(mut progression) = players.get_mut(killer) else {
        return;
    };

    let previous_level = progression.level;
    let gain = progression.grant_experience(reward.0);
    info!(
        "Player {killer:?} gained {} base XP (level {}, XP {})",
        gain.amount, progression.level, progression.experience
    );
    if gain.levels_gained > 0 {
        info!(
            "Player {killer:?} gained {} base level(s): {} -> {}",
            gain.levels_gained, previous_level, progression.level
        );
    }
}

fn should_receive_attack_state(
    viewer_entity: Entity,
    attacking_entity: Entity,
    line_of_sight: &LineOfSight,
) -> bool {
    viewer_entity == attacking_entity || line_of_sight.0.contains(&attacking_entity)
}

#[cfg(test)]
mod attack_timing_tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use std::collections::HashSet;

    #[test]
    fn basic_attack_damage_stays_inside_the_inclusive_range() {
        let mut rng = StdRng::seed_from_u64(42);
        let rolls: HashSet<u32> = (0..1_024)
            .map(|_| roll_basic_attack_damage(&mut rng))
            .collect();

        assert_eq!(
            rolls,
            HashSet::from([
                BASIC_ATTACK_DAMAGE_MIN,
                BASIC_ATTACK_DAMAGE_MIN + 1,
                BASIC_ATTACK_DAMAGE_MIN + 2,
                BASIC_ATTACK_DAMAGE_MAX,
            ])
        );
    }

    #[test]
    fn aggression_modes_apply_the_expected_provocation_rules() {
        assert!(aggression_reacts_to(
            MonsterAggression::Passive,
            DamageOrigin::BasicAttack
        ));
        assert!(aggression_reacts_to(
            MonsterAggression::Passive,
            DamageOrigin::DirectSpell
        ));
        assert!(aggression_reacts_to(
            MonsterAggression::Aggressive,
            DamageOrigin::BasicAttack
        ));
        assert!(aggression_reacts_to(
            MonsterAggression::SpellReactive,
            DamageOrigin::BasicAttack
        ));
        assert!(aggression_reacts_to(
            MonsterAggression::SpellReactive,
            DamageOrigin::DirectSpell
        ));
    }

    #[test]
    fn passive_monster_retaliates_against_the_attacking_player() {
        let mut app = App::new();
        app.add_observer(provoke_monster_on_damage);
        let player = app
            .world_mut()
            .spawn((Player { id: 1 }, Transform::from_xyz(3.0, 1.0, 2.0)))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                MonsterAggression::Passive,
                Transform::default(),
                Health {
                    current: 100,
                    max: 100,
                },
            ))
            .id();

        app.world_mut().trigger(HealthChange {
            entity: monster,
            source: Some(player),
            amount: 5,
            damage: 5,
            damage_type: HealthChangeType::Normal,
            origin: DamageOrigin::BasicAttack,
        });
        app.world_mut().flush();

        let aggro = app.world().get::<Aggro>(monster).unwrap();
        assert_eq!(aggro.enemy, player);
        assert_eq!(aggro.enemy_translation, Vec3::new(3.0, 1.0, 2.0));
    }

    #[test]
    fn spell_reactive_monster_retaliates_against_attacks_and_direct_spells() {
        let mut app = App::new();
        app.add_observer(provoke_monster_on_damage);
        let player = app
            .world_mut()
            .spawn((Player { id: 1 }, Transform::from_xyz(2.0, 1.0, 0.0)))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                MonsterAggression::SpellReactive,
                Transform::default(),
                Health {
                    current: 100,
                    max: 100,
                },
            ))
            .id();

        for origin in [DamageOrigin::BasicAttack, DamageOrigin::DirectSpell] {
            app.world_mut().trigger(HealthChange {
                entity: monster,
                source: Some(player),
                amount: 5,
                damage: 5,
                damage_type: HealthChangeType::Normal,
                origin,
            });
            app.world_mut().flush();
            assert_eq!(app.world().get::<Aggro>(monster).unwrap().enemy, player);
            app.world_mut().entity_mut(monster).remove::<Aggro>();
        }
    }

    #[test]
    fn spell_reactive_monster_aggroes_when_direct_cast_begins() {
        let mut app = App::new();
        app.add_observer(provoke_spell_reactive_monster);
        let player = app
            .world_mut()
            .spawn((Player { id: 1 }, Transform::from_xyz(4.0, 1.0, 2.0)))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                MonsterAggression::SpellReactive,
                Transform::default(),
                Health {
                    current: 100,
                    max: 100,
                },
            ))
            .id();

        app.world_mut().trigger(DirectSpellTargeted {
            monster,
            caster: player,
        });
        app.world_mut().flush();

        let aggro = app.world().get::<Aggro>(monster).unwrap();
        assert_eq!(aggro.enemy, player);
        assert_eq!(aggro.enemy_translation, Vec3::new(4.0, 1.0, 2.0));
    }

    #[test]
    fn provocation_outside_monster_vision_does_not_create_aggro() {
        let mut app = App::new();
        app.add_observer(provoke_monster_on_damage)
            .add_observer(provoke_spell_reactive_monster);
        let player = app
            .world_mut()
            .spawn((Player { id: 1 }, Transform::from_xyz(9.0, 1.0, 0.0)))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                MonsterAggression::SpellReactive,
                Transform::default(),
                Health {
                    current: 100,
                    max: 100,
                },
            ))
            .id();

        app.world_mut().trigger(DirectSpellTargeted {
            monster,
            caster: player,
        });
        app.world_mut().trigger(HealthChange {
            entity: monster,
            source: Some(player),
            amount: 20,
            damage: 20,
            damage_type: HealthChangeType::Normal,
            origin: DamageOrigin::DirectSpell,
        });
        app.world_mut().flush();

        assert!(app.world().get::<Aggro>(monster).is_none());
    }

    #[test]
    fn aggressive_monster_acquires_the_closest_living_player() {
        let mut app = App::new();
        app.add_systems(Update, acquire_aggressive_monster_targets);
        let farther = app
            .world_mut()
            .spawn((
                Player { id: 1 },
                Transform::from_xyz(6.0, 1.0, 0.0),
                Health {
                    current: 40,
                    max: 40,
                },
            ))
            .id();
        let closest = app
            .world_mut()
            .spawn((
                Player { id: 2 },
                Transform::from_xyz(3.0, 1.0, 0.0),
                Health {
                    current: 40,
                    max: 40,
                },
            ))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Orc,
                },
                MonsterAggression::Aggressive,
                Transform::default(),
                Health {
                    current: 100,
                    max: 100,
                },
            ))
            .id();

        app.update();

        let aggro = app.world().get::<Aggro>(monster).unwrap();
        assert_eq!(aggro.enemy, closest);
        assert_ne!(aggro.enemy, farther);
    }

    #[test]
    fn first_hit_occurs_when_the_fifth_frame_begins() {
        let mut timer = attack_cycle_timer(0.8, true);

        assert!((timer.elapsed_secs() - 0.4).abs() < f32::EPSILON);
        timer.tick(Duration::from_secs_f32(0.399));
        assert!(!timer.just_finished());
        timer.tick(timer.remaining());
        assert!(timer.just_finished());
    }

    #[test]
    fn target_death_clears_combat_follow_movement() {
        let mut app = App::new();
        app.add_plugins(CombatPlugin);

        let target = app.world_mut().spawn_empty().id();
        let mut controller = KinematicCharacterController::default();
        controller.translation = Some(Vec3::X);
        let attacker = app
            .world_mut()
            .spawn((
                Aggro {
                    enemy: target,
                    auto_attack: true,
                    enemy_translation: Vec3::X,
                },
                Attacking {
                    enemy: target,
                    auto_attack: true,
                },
                AttackingTimer(Timer::from_seconds(0.5, TimerMode::Repeating)),
                Walking {
                    target_translation: Vec3::X,
                    path: None,
                },
                TargetPos { position: Vec3::X },
                GameVelocity(Vec3::X),
                controller,
            ))
            .id();

        app.world_mut().trigger(DeathEvent {
            entity: target,
            killer: Some(attacker),
        });
        app.world_mut().flush();

        let attacker = app.world().entity(attacker);
        assert!(attacker.get::<Aggro>().is_none());
        assert!(attacker.get::<Attacking>().is_none());
        assert!(attacker.get::<AttackingTimer>().is_none());
        assert!(attacker.get::<Walking>().is_none());
        assert!(attacker.get::<TargetPos>().is_none());
        assert_eq!(attacker.get::<GameVelocity>().unwrap().0, Vec3::ZERO);
        assert_eq!(
            attacker
                .get::<KinematicCharacterController>()
                .unwrap()
                .translation,
            None
        );
    }

    #[test]
    fn killing_a_monster_rewards_the_killing_player() {
        let mut app = App::new();
        app.add_plugins(CombatPlugin);

        let killer = app
            .world_mut()
            .spawn((Player { id: 1 }, BaseProgression::default()))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                ExperienceReward(120),
                Transform::default(),
            ))
            .id();

        app.world_mut().trigger(DeathEvent {
            entity: monster,
            killer: Some(killer),
        });
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<BaseProgression>(killer),
            Some(&BaseProgression {
                level: 2,
                experience: 20,
            })
        );
    }

    #[test]
    fn player_receives_own_attack_state_without_self_in_line_of_sight() {
        let mut world = World::new();
        let viewer = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        let empty_line_of_sight = LineOfSight::default();

        assert!(should_receive_attack_state(
            viewer,
            viewer,
            &empty_line_of_sight
        ));
        assert!(!should_receive_attack_state(
            viewer,
            other,
            &empty_line_of_sight
        ));
    }
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.add_systems(
            FixedUpdate,
            (
                network_change_attacking_state.run_if(in_state(ServerState::InGame)),
                network_send_delta_health_system.run_if(in_state(ServerState::InGame)),
                network_send_progression_system.run_if(in_state(ServerState::InGame)),
                recalculate_path
                    .before(crate::server::gameplay::pathing::apply_rapier3d_velocity_system),
                acquire_aggressive_monster_targets.run_if(in_state(ServerState::InGame)),
                aggro_rapier3d
                    .run_if(in_state(ServerState::InGame))
                    .before(crate::server::gameplay::pathing::apply_rapier3d_velocity_system),
                attack.run_if(in_state(ServerState::InGame)),
            ),
        )
        .add_observer(provoke_monster_on_damage)
        .add_observer(provoke_spell_reactive_monster)
        .add_observer(on_death_stop_attackers)
        .add_observer(on_health_change)
        .add_observer(on_death_give_experience)
        .add_observer(on_death_despawn_monsters);

        fn aggro_rapier3d(
            mut aggroed_entities: Query<
                (
                    Entity,
                    &Transform,
                    &mut Aggro,
                    Option<&mut Attacking>,
                    Option<&mut Walking>,
                ),
                (Or<(With<Player>, With<NPC>, With<Monster>)>),
            >,
            //attacked_entities: Query<(Entity, &Transform), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut commands: Commands,
            //spatial_query: SpatialQuery,
            // rapier_context: Res<RapierContext>,
            read_rapier_context: ReadRapierContext,
            map_query: Query<&MapEntity>,
            time: Res<Time>,
            map: Res<Map>,
        ) {
            if let Ok(rapier_context) = read_rapier_context.single() {
                for (entity, attacker_transform, aggroed, is_attacking, mut is_walking) in
                    aggroed_entities.iter_mut()
                {
                    let attack_range: f32 = 1.;
                    // If in range, attack.
                    if (
                        is_in_attack_range(
                            attack_range,
                            attacker_transform.translation,
                            aggroed.enemy_translation,
                        ) && is_in_view_rapier3d(
                            &rapier_context,
                            attacker_transform.translation,
                            aggroed.enemy_translation,
                            aggroed.enemy,
                            &map_query,
                        )
                        // && is_attacking.is_none()
                    ) {
                        if is_attacking.is_some() {
                            //info!("walking? {:?}", is_walking);
                            continue;
                        }
                        //info!("ATACARRRRRRRRRRRR");
                        // STOP WALKING. ALREADY NEAR TARGET.
                        is_walking = None;

                        let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
                        timer.pause(); // Timer pausado hasta que este en rango de ataque;

                        commands
                            .entity(entity)
                            .try_insert(AttackingTimer(timer))
                            .try_insert(Attacking {
                                enemy: aggroed.enemy,
                                auto_attack: aggroed.auto_attack,
                                //enemy_translation: aggroed.enemy_translation,
                                // timer: timer
                            })
                            .try_remove::<Walking>()
                            .try_remove::<TargetPos>();

                        continue;
                    }

                    if let Some(walking) = is_walking {
                        if (walking.target_translation == aggroed.enemy_translation) {
                            //info!("Already walking. {:?}", walking);
                            continue;
                        }
                    }

                    info!("No esta en attack range ni puede ver al enemigo. No está caminando. Se cambia a caminando.");
                    let path = get_path_between_translations(
                        attacker_transform.translation,
                        aggroed.enemy_translation,
                        &map,
                    );
                    info!(
                        "Se calcula camino nuevo hacia el enemigo que está en {:?} {:?}",
                        aggroed.enemy_translation, path
                    );

                    commands
                        .entity(entity)
                        .try_insert(Walking {
                            target_translation: aggroed.enemy_translation,
                            path: path,
                        })
                        .try_remove::<Attacking>()
                        .try_remove::<AttackingTimer>();

                    // Si hay camino, se intenta acercar.
                    /*if let Some((steps_vec, steps_left)) = aggroed.path.clone() {

                        let current_cell_index: Option<usize>  =  steps_vec.iter().position(|&r| r ==  Pos(
                            attacker_transform.translation.x.round() as i32,
                            attacker_transform.translation.z.round() as i32
                        ));

                        if let Some(current_index) = current_cell_index {

                            if let Some(final_pos) = steps_vec.get(current_index+1) {
                                //info!("5. Final Pos: {:?}!", final_pos);
                                // Se cambia el punto objetivo.
                                commands.entity(entity).insert(TargetPos {
                                    position: Vec3 { x: final_pos.0 as f32, y: 2.0, z: final_pos.1 as f32},
                                });
                            }

                            //}

                        }
                    }    */
                }
            }
        }

        fn attack(
            mut attacking_entities: Query<
                (Entity, &mut Attacking, &AttackSpeed, &mut AttackingTimer),
                (Or<(With<Player>, With<NPC>, With<Monster>)>),
            >,
            mut commands: Commands,
            time: Res<Time>,
        ) {
            let mut rng = rand::thread_rng();
            for (entity, attacking, attack_speed, mut attacking_timer) in
                attacking_entities.iter_mut()
            {
                // Los timers de atraque empiezan detenidos.
                // Se inicia cuando ya esta en rango y las validaciones son exitosas
                if attacking_timer.0.is_paused() {
                    info!("El timer está parado. No se ha empezado a atacar aún.");
                    attacking_timer.0 = attack_cycle_timer(attack_speed.0, attacking.auto_attack);
                    continue;
                }

                // con el aspd que inicio el timer, se empieza a correr el tiempo.
                // Cuando llega al final, se envía el evento de ataque.
                attacking_timer.0.tick(time.delta());

                if (!attacking_timer.0.just_finished()) {
                    continue;
                }

                info!("Finalizó el timer. Timer: {:?}", attacking_timer.0);
                let damage = roll_basic_attack_damage(&mut rng);
                commands.trigger(HealthChange {
                    entity: attacking.enemy,
                    source: Some(entity),
                    amount: damage as i32,
                    damage,
                    damage_type: HealthChangeType::Normal,
                    origin: DamageOrigin::BasicAttack,
                });

                if (attacking.auto_attack == false) {
                    commands
                        .entity(entity)
                        .try_remove::<Aggro>()
                        .try_remove::<Attacking>()
                        .try_remove::<AttackingTimer>();
                    continue;
                }
            }
        }

        /*fn attack_avian3d(
            mut aggroed_entities: Query<(Entity, &Transform, &mut Aggro), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            //attacked_entities: Query<(Entity, &Transform), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut commands: Commands,
            spatial_query: SpatialQuery,
            map_query: Query<&MapEntity>,
            time: Res<Time>,
        ) {
            for (entity, attacker_transform,  mut attacking) in aggroed_entities.iter_mut() {

                let attack_range:f32 = 2.;
                // If in range, attack.
                //info!("1. Se ataca entity {:?}", attacking.enemy);
                if(is_in_attack_range(attack_range, attacker_transform.translation, attacking.enemy_translation)
                && is_in_view_avian3d(&spatial_query, attacker_transform.translation, attacking.enemy_translation, attacking.enemy, &map_query)) {

                    // Los timers de atraque empiezan detenidos.
                    // Se inicia cuando ya esta en rango y las validaciones son exitosas
                    if(attacking.timer.paused()) {
                        info!("El timer está parado. No se ha empezado a atacar aún.");
                        let attack_speed = 0.5;
                        attacking.timer = Timer::from_seconds(attack_speed, TimerMode::Once);
                        continue;
                    }

                    // con el aspd que inicio el timer, se empieza a correr el tiempo.
                    // Cuando llega al final, se envía el evento de ataque.
                    attacking.timer.tick(time.delta());

                    if(!attacking.timer.just_finished()) {
                        continue;
                    }

                    info!("Finalizó el timer. Timer: {:?}", attacking.timer);
                    commands.trigger(HealthChange {
                        entity: attacking.enemy,
                        damage: 10
                    });

                    if(attacking.auto_attack == false) {
                        commands.entity(entity).remove::<Aggro>();
                        continue;
                    }

                    continue;

                }

                // Si hay camino, se intenta acercar.
                if let Some((steps_vec, steps_left)) = attacking.path.clone() {

                    let current_cell_index: Option<usize>  =  steps_vec.iter().position(|&r| r ==  Pos(
                        attacker_transform.translation.x.round() as i32,
                        attacker_transform.translation.z.round() as i32
                    ));

                    if let Some(current_index) = current_cell_index {

                        // Tiene dos de attack range
                        // Hay 10 celdas, de la 0 a la 9.
                        // Se tiene que acercar a la 7 (9-2)
                        // [ ][ ][ ][ ][ ][ ][ ][*][ ][ENEMY]
                        // Si tuviera un número impar, ejemplo 2.5 de attack range
                        // Siempre lo redondeamos hacia abajo y hacemos caminar el .5 extra. Igual no pasa nada porque apenas lo ve, lo ataca.
                        // [ ][ ][ ][ ][ ][ ][ ][*][ ][ENEMY]
                        /*let attack_range_u32 = attack_range.floor() as u32;

                        let target_cell_index =  if(steps_left >= attack_range_u32) {
                            (steps_left - attack_range_u32) as usize
                        }
                        else {
                            // Ya está dentro del attack range pero aun no lo ve.
                            current_index + 1
                        };     */


                        // Aún no llega al m
                        //info!("4. Index objetivo: {:?}", target_cell_index);
                        //if current_index < target_cell_index{

                        if let Some(final_pos) = steps_vec.get(current_index+1) {
                            //info!("5. Final Pos: {:?}!", final_pos);
                            // Se cambia el punto objetivo.
                            commands.entity(entity).insert(TargetPos {
                                position: Vec3 { x: final_pos.0 as f32, y: 2.0, z: final_pos.1 as f32},
                            });
                        }

                        //}

                    }
                }
            }
        }*/

        // falta el caso en q se mueve el jugador de alguna forma random, debemos tambien
        pub fn recalculate_path(
            mut attackers: Query<
                (&mut Walking, &Transform, &mut Aggro),
                (Or<(With<Player>, With<NPC>, With<Monster>)>),
            >,
            enemies: Query<
                (&Transform),
                (
                    Or<(With<Player>, With<NPC>, With<Monster>)>,
                    Changed<Transform>,
                ),
            >,
            map: Res<Map>,
        ) {
            for (mut walking, attacker_transform, mut aggroed) in attackers.iter_mut() {
                let mut enemy_translation_changed = false;
                // Caso 1. El enemigo objetivo se ha movido.
                if let Ok((enemy_transform)) = enemies.get(aggroed.enemy) {
                    if (aggroed.enemy_translation != enemy_transform.translation) {
                        aggroed.enemy_translation = enemy_transform.translation;
                        enemy_translation_changed = true;
                    }
                }

                // Caso 2. El mapa ha cambiado. Esto podría pasar si implementamos por ejemplo magias como "Icewall" que puedan bloquear el camino temporalmente.
                if (map.is_changed() || enemy_translation_changed) {
                    walking.path = get_path_between_translations(
                        attacker_transform.translation,
                        aggroed.enemy_translation,
                        &map,
                    );
                    info!("Cambio el translation del enemigo: {:?}", walking.path);
                    /*
                    aggroed.path = get_path_between_translations(attacker_transform.translation, aggroed.enemy_translation, &map);  */
                }
            }
        }

        fn on_death_despawn_monsters(
            trigger: On<DeathEvent>,
            query: Query<Entity, With<Monster>>,
            mut commands: Commands,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let death_event = trigger.event();
            let id: Entity = death_event.entity;
            info!("Muere la entidad:  {:?} ", id);
            if let Ok(entity) = query.get(id) {
                commands.entity(entity).despawn();
                info!("Muere la entidad:  {:?} ", entity);
                // Si es jugador, mantenrlo muerto en el piso.
                // Si es monstruo, debe soltar ítems.
            }
        }

        fn on_death_stop_attackers(
            trigger: On<DeathEvent>,
            mut attackers: Query<(
                Entity,
                &Aggro,
                Option<&mut GameVelocity>,
                Option<&mut KinematicCharacterController>,
            )>,
            mut commands: Commands,
        ) {
            let dead_entity = trigger.event().entity;

            for (attacker, aggro, velocity, controller) in &mut attackers {
                if aggro.enemy != dead_entity {
                    continue;
                }

                if let Some(mut velocity) = velocity {
                    velocity.0 = Vec3::ZERO;
                }
                if let Some(mut controller) = controller {
                    controller.translation = None;
                }

                commands
                    .entity(attacker)
                    .try_remove::<Aggro>()
                    .try_remove::<Attacking>()
                    .try_remove::<AttackingTimer>()
                    .try_remove::<Walking>()
                    .try_remove::<TargetPos>();
            }
        }

        /*
        fn on_death_spawn_loot(
            trigger: On<DeathEvent>,
            mut query: Query<(&Transform)>,
            mut commands: Commands,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let death_event = trigger.event();
            let id: Entity = death_event.entity;

            if let Ok((transform)) = query.get_mut(id) {
                info!("Se crea loot en:  {:?} ", transform.translation);
                // Si es jugador, mantenrlo muerto en el piso.
                // Si es monstruo, debe soltar ítems.
            }
        }
        */

        fn on_health_change(
            trigger: On<HealthChange>,
            mut query: Query<(Entity, &mut Health)>,
            mut commands: Commands,
            mut server: ResMut<RenetServer>,
            players: Query<(&Player, &LineOfSight)>,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let health_change: &HealthChange = trigger.event();
            let id: Entity = health_change.entity;

            if let Ok((entity, mut health)) = query.get_mut(id) {
                if health.current == 0 {
                    return;
                }
                info!("Entity  {:?} damaged.", id.index());

                let message = bincode::serialize(&ServerMessages::DamageNumber {
                    entity,
                    amount: i32::try_from(health_change.damage).unwrap_or(i32::MAX),
                })
                .unwrap();
                for (player, line_of_sight) in &players {
                    if line_of_sight.0.contains(&entity) {
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            message.clone(),
                        );
                    }
                }

                if (health.current <= health_change.damage) {
                    health.current = 0;
                    commands.trigger(DeathEvent {
                        entity: health_change.entity,
                        killer: health_change.source,
                    });
                } else {
                    health.current -= health_change.damage;
                    info!("Health  {:?} ", health);
                }
            }
        }

        pub fn network_change_attacking_state(
            mut server: ResMut<RenetServer>,
            players: Query<(Entity, &Player, &LineOfSight)>,
            entities: Query<
                (Entity, &Attacking, &AttackSpeed),
                Or<(Changed<Attacking>, Changed<AttackSpeed>)>,
            >,
            mut stopped_attacking: RemovedComponents<Attacking>,
        ) {
            for (entity, attacking, attack_speed) in &entities {
                let message = bincode::serialize(&ServerMessages::Attack {
                    entity,
                    enemy: attacking.enemy,
                    attack_speed: attack_speed.0,
                    auto_attack: attacking.auto_attack,
                })
                .expect("attack message should serialize");

                for (viewer_entity, player, line_of_sight) in &players {
                    if should_receive_attack_state(viewer_entity, entity, line_of_sight) {
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            message.clone(),
                        );
                    }
                }
            }

            for entity in stopped_attacking.read() {
                let message =
                    bincode::serialize(&ServerMessages::AttackStopped { entity }).unwrap();

                for (viewer_entity, player, line_of_sight) in &players {
                    if should_receive_attack_state(viewer_entity, entity, line_of_sight) {
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            message.clone(),
                        );
                    }
                }
            }
        }

        pub fn network_send_delta_health_system(
            mut server: ResMut<RenetServer>,
            players: Query<(&Player, &LineOfSight)>,
            mut entities: Query<(Entity, &Health), Changed<Health>>,
            //time: Res<Time>,
        ) {
            for (player, line_of_sight) in players.iter() {
                for entity in line_of_sight.0.iter() {
                    if let Ok((entity, health)) = entities.get_mut(*entity) {
                        let message = ServerMessages::HealthChange {
                            entity,
                            amount: 5,
                            max: health.max,
                            current: health.current,
                        };

                        let sync_message = bincode::serialize(&message).unwrap();
                        // Send message to only one client
                        server.send_message(player.id, ServerChannel::ServerMessages, sync_message);
                    }
                }
            }
        }

        pub fn network_send_progression_system(
            mut server: ResMut<RenetServer>,
            viewers: Query<(Entity, &Player, &LineOfSight)>,
            changed_progression: Query<(Entity, &BaseProgression), Changed<BaseProgression>>,
        ) {
            for (entity, progression) in &changed_progression {
                let message = bincode::serialize(&ServerMessages::ProgressionChanged {
                    entity,
                    progression: *progression,
                })
                .expect("progression message should serialize");

                for (viewer_entity, player, line_of_sight) in &viewers {
                    if viewer_entity == entity || line_of_sight.0.contains(&entity) {
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            message.clone(),
                        );
                    }
                }
            }
        }
    }
}

pub fn is_in_view_rapier3d(
    rapier_context: &RapierContext,
    origin_translation: Vec3,
    target_translation: Vec3,
    target_entity: Entity,
    map_query: &Query<&MapEntity>,
) -> bool {
    let direction = (target_translation - origin_translation).normalize_or_zero();

    let predicate = |handle| {
        // We can use a query to bevy inside the predicate.
        map_query.contains(handle) || handle == target_entity
    };

    if let Some((entity, _time_of_impact)) = rapier_context.cast_ray(
        origin_translation,
        direction,
        bevy_rapier3d::prelude::Real::MAX,
        true,
        QueryFilter::default().predicate(&predicate),
    ) {
        if (entity == target_entity) {
            //println!("PUEDO VER AL OBJETIVO: {:?}", entity);
            return true;
        } else {
            //println!("NO PUEDO VER AL OBJETIV{:?}", entity);
        }
    }

    return false;
}

/*
pub fn is_in_view_avian3d(spatial_query: &SpatialQuery, origin_translation: Vec3, target_translation: Vec3, target_entity: Entity, map_query: &Query<&MapEntity>) -> bool {

    let xyz = (target_translation - origin_translation).normalize_or_zero();

    let direction = Dir3::from_xyz(xyz.x, xyz.y, xyz.z);

    let direction = if let Ok(direction) = direction {  direction  }  else { return false; };

    // Cast ray and print first hit
    if let Some(first_hit) = spatial_query.cast_ray_predicate(
        origin_translation,                    // Origin
        direction,                       // Direction
        15.0,                         // Maximum time of impact (travel distance)
        true,                          // Does the ray treat colliders as "solid"
        SpatialQueryFilter::default(), // Query filter
        &|entity| {
           // println!("map_query: {:?}", map_query);
            //println!("Contains entity: {:?}", map_query.contains(entity));

            map_query.contains(entity) || entity == target_entity

        }
    ) {



        if(first_hit.entity == target_entity) {
            println!("PUEDO VER AL OBJETIVO: {:?}", first_hit);
            return true;
        }
        else {
            println!("NO PUEDO VER AL OBJETIV{:?}", first_hit);

        }
    }

    return false;
}*/

pub fn is_in_attack_range(
    attack_range: f32,
    attacker_translation: Vec3,
    attacked_translation: Vec3,
) -> bool {
    // let distance = (attacker_translation - attacked_translation).round();
    let distance = attacker_translation - attacked_translation;
    //info!("Distancia {:?}", distance);
    if (distance.x.abs() <= attack_range && distance.z.abs() <= attack_range) {
        //info!("esta en attack range");
        return true;
    }

    return false;
}
