use super::monsters::StartingMapMonster;
use super::pathing::{
    get_path_between_translations, DamageWalkDelay, DamageWalkDelayImmunity, TargetPos,
};
use super::spawn_protection::SpawnProtection;
use crate::{
    server::network::replication::{should_receive_player_action, LineOfSight},
    shared::{
        constants::ATTACK_HIT_FRACTION,
        gameplay::components::*,
        gameplay::entities::{AttackSpeed, MapEntity, Player, NPC},
        gameplay::items::equipment_derived_stats,
        gameplay::progression::{BaseProgression, ExperienceReward, JobProgression},
        gameplay::skills::SkillTree,
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
const STARTING_MONSTER_DAMAGE_MIN: u32 = 2;
const STARTING_MONSTER_DAMAGE_MAX: u32 = 4;
const BASELINE_PHYSICAL_ATTACK: u32 = 3;
const MONSTER_AGGRO_DETECTION_RANGE: f32 = 8.0;
const BASE_BASIC_ATTACK_HIT_CHANCE: i32 = 85;
const MIN_BASIC_ATTACK_HIT_CHANCE: i32 = 20;
const MAX_BASIC_ATTACK_HIT_CHANCE: i32 = 95;
const MONSTER_REPATH_HZ: f32 = 6.0;
const MONSTER_REPATH_INTERVAL_SECONDS: f32 = 1.0 / MONSTER_REPATH_HZ;
// The server runs at 60 Hz, so ten phases distribute a 6 Hz repath cycle
// across successive fixed ticks instead of updating every monster together.
const MONSTER_REPATH_PHASES: u32 = 10;

#[derive(Component, Debug)]
struct MonsterRepathSchedule {
    timer: Timer,
    map_change_pending: bool,
}

impl MonsterRepathSchedule {
    fn new(entity: Entity, map_change_pending: bool) -> Self {
        let mut timer = Timer::from_seconds(MONSTER_REPATH_INTERVAL_SECONDS, TimerMode::Repeating);
        timer.set_elapsed(Duration::from_secs_f32(monster_repath_stagger_seconds(
            entity.index().index(),
        )));
        Self {
            timer,
            map_change_pending,
        }
    }
}

fn monster_repath_stagger_seconds(entity_index: u32) -> f32 {
    let phase = entity_index % MONSTER_REPATH_PHASES;
    MONSTER_REPATH_INTERVAL_SECONDS * phase as f32 / MONSTER_REPATH_PHASES as f32
}

fn navigation_cell(translation: Vec3) -> Pos {
    Pos(translation.x.round() as i32, translation.z.round() as i32)
}

fn pursuit_target_cell_changed(walking: &Walking, target_translation: Vec3) -> bool {
    navigation_cell(walking.target_translation) != navigation_cell(target_translation)
}

fn should_recalculate_pursuit_path(
    is_monster: bool,
    monster_timer_due: bool,
    target_cell_changed: bool,
    map_change_pending: bool,
) -> bool {
    (target_cell_changed || map_change_pending) && (!is_monster || monster_timer_due)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CombatRatings {
    hit: i32,
    flee: i32,
}

fn roll_basic_attack_damage(
    rng: &mut impl Rng,
    damage_bonus: u32,
    starting_map_monster: bool,
) -> u32 {
    let damage = if starting_map_monster {
        rng.gen_range(STARTING_MONSTER_DAMAGE_MIN..=STARTING_MONSTER_DAMAGE_MAX)
    } else {
        rng.gen_range(BASIC_ATTACK_DAMAGE_MIN..=BASIC_ATTACK_DAMAGE_MAX)
    };
    damage.saturating_add(damage_bonus)
}

fn basic_attack_damage_bonus(
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
        .physical_attack
        .saturating_sub(BASELINE_PHYSICAL_ATTACK)
}

fn combat_ratings(
    stats: Option<&CharacterStats>,
    progression: Option<&BaseProgression>,
    equipment: Option<&Equipment>,
    monster: Option<&Monster>,
) -> CombatRatings {
    if let Some(stats) = stats {
        let level = progression.map_or(1, |progression| progression.level);
        let derived = equipment.map_or_else(
            || stats.derived(level),
            |equipment| equipment_derived_stats(stats, level, equipment),
        );
        return CombatRatings {
            hit: i32::try_from(derived.hit).unwrap_or(i32::MAX),
            flee: i32::try_from(derived.flee).unwrap_or(i32::MAX),
        };
    }

    match monster.map(|monster| monster.kind) {
        Some(MonsterKind::Pig) => CombatRatings { hit: 2, flee: 2 },
        Some(MonsterKind::Orc) => CombatRatings { hit: 6, flee: 4 },
        None => CombatRatings { hit: 1, flee: 1 },
    }
}

fn basic_attack_hit_chance(attacker: CombatRatings, defender: CombatRatings) -> u8 {
    (BASE_BASIC_ATTACK_HIT_CHANCE + attacker.hit - defender.flee)
        .clamp(MIN_BASIC_ATTACK_HIT_CHANCE, MAX_BASIC_ATTACK_HIT_CHANCE) as u8
}

fn roll_basic_attack_hit(rng: &mut impl Rng, chance_percent: u8) -> bool {
    rng.gen_range(0..100_u8) < chance_percent
}

fn mitigate_damage(
    raw_damage: u32,
    origin: DamageOrigin,
    derived: Option<DerivedCharacterStats>,
) -> u32 {
    if raw_damage == 0 {
        return 0;
    }
    let defense = derived.map_or(0, |derived| match origin {
        DamageOrigin::BasicAttack => derived.physical_defense,
        DamageOrigin::DirectSpell | DamageOrigin::AreaSpell => derived.magic_defense,
    });
    raw_damage.saturating_sub(defense).max(1)
}

fn should_apply_damage_walk_delay(damage: u32, target_is_player: bool, has_immunity: bool) -> bool {
    damage > 0 && target_is_player && !has_immunity
}

fn spawn_protection_blocks_damage(
    damage: u32,
    target_is_player: bool,
    has_spawn_protection: bool,
) -> bool {
    damage > 0 && target_is_player && has_spawn_protection
}

fn sync_derived_resource_maxima(
    mut players: Query<
        (
            &CharacterStats,
            &BaseProgression,
            &Equipment,
            &mut Health,
            &mut Mana,
        ),
        (
            With<Player>,
            Or<(
                Changed<CharacterStats>,
                Changed<BaseProgression>,
                Changed<Equipment>,
            )>,
        ),
    >,
) {
    for (stats, progression, equipment, mut health, mut mana) in &mut players {
        let derived = equipment_derived_stats(stats, progression.level, equipment);
        health.max = derived.max_health;
        health.current = health.current.min(health.max);
        mana.max = derived.max_mana;
        mana.current = mana.current.min(mana.max);
    }
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

fn should_release_monster_aggro(monster: Vec3, player: Option<Vec3>) -> bool {
    player.is_none_or(|player| !is_within_monster_vision(monster, player))
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
    players: Query<(Entity, &Transform, &Health), (With<Player>, Without<SpawnProtection>)>,
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

fn release_monsters_targeting_protected_players(
    protected_players: Query<(), (With<Player>, With<SpawnProtection>)>,
    mut monsters: Query<
        (
            Entity,
            &Aggro,
            &mut GameVelocity,
            Option<&mut KinematicCharacterController>,
        ),
        With<Monster>,
    >,
    mut commands: Commands,
) {
    for (monster, aggro, mut velocity, controller) in &mut monsters {
        if !protected_players.contains(aggro.enemy) {
            continue;
        }

        velocity.0 = Vec3::ZERO;
        if let Some(mut controller) = controller {
            controller.translation = None;
        }
        commands
            .entity(monster)
            .try_remove::<Aggro>()
            .try_remove::<Attacking>()
            .try_remove::<AttackingTimer>()
            .try_remove::<Walking>()
            .try_remove::<TargetPos>();
    }
}

fn release_out_of_range_monster_aggro(
    mut commands: Commands,
    mut monsters: Query<
        (
            Entity,
            &Transform,
            &Aggro,
            &mut GameVelocity,
            Option<&mut KinematicCharacterController>,
        ),
        With<Monster>,
    >,
    players: Query<(&Transform, &Health), With<Player>>,
    map: Res<Map>,
) {
    for (monster, monster_transform, aggro, mut velocity, controller) in &mut monsters {
        let player_translation = players
            .get(aggro.enemy)
            .ok()
            .and_then(|(transform, health)| (health.current > 0).then_some(transform.translation));
        if !should_release_monster_aggro(monster_transform.translation, player_translation) {
            continue;
        }

        let last_known_position = aggro.enemy_translation;
        let last_known_path =
            get_path_between_translations(monster_transform.translation, last_known_position, &map)
                .filter(|(steps, _)| steps.len() > 1);

        velocity.0 = Vec3::ZERO;
        if let Some(mut controller) = controller {
            controller.translation = None;
        }
        let mut monster_commands = commands.entity(monster);
        monster_commands
            .try_remove::<Aggro>()
            .try_remove::<Attacking>()
            .try_remove::<AttackingTimer>()
            .try_remove::<TargetPos>();
        if let Some(path) = last_known_path {
            monster_commands.try_insert(Walking {
                target_translation: last_known_position,
                path: Some(path),
            });
            debug!(
                "Monster {monster:?} lost aggro and is pursuing last known position {last_known_position:?}"
            );
        } else {
            monster_commands.try_remove::<Walking>();
            debug!("Monster {monster:?} released aggro at its last known or unreachable position");
        }
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

fn synchronize_attack_mode(attacking: &mut Attacking, timer: &mut Timer, auto_attack: bool) {
    if attacking.auto_attack == auto_attack {
        return;
    }
    attacking.auto_attack = auto_attack;
    timer.set_mode(if auto_attack {
        TimerMode::Repeating
    } else {
        TimerMode::Once
    });
}

fn on_death_give_experience(
    trigger: On<DeathEvent>,
    rewards: Query<&ExperienceReward, With<Monster>>,
    mut players: Query<
        (
            &mut BaseProgression,
            &mut JobProgression,
            &mut SkillTree,
            &mut CharacterStats,
            &Equipment,
            &mut Health,
            &mut Mana,
        ),
        With<Player>,
    >,
) {
    let death_event = trigger.event();
    let Ok(reward) = rewards.get(death_event.entity) else {
        return;
    };
    let Some(killer) = death_event.killer else {
        return;
    };
    let Ok((
        mut progression,
        mut job_progression,
        mut skill_tree,
        mut stats,
        equipment,
        mut health,
        mut mana,
    )) = players.get_mut(killer)
    else {
        return;
    };

    let previous_level = progression.level;
    let previous_job_level = job_progression.level;
    let base_gain = progression.grant_experience(reward.base);
    let job_gain = job_progression.grant_experience(reward.job);
    let attribute_points_awarded = stats.grant_base_levels(previous_level, base_gain.levels_gained);
    skill_tree.grant_job_levels(job_gain.levels_gained);
    if base_gain.levels_gained > 0 {
        let derived = equipment_derived_stats(&stats, progression.level, equipment);
        health.max = derived.max_health;
        health.current = health.max;
        mana.max = derived.max_mana;
        mana.current = mana.max;
    }
    info!(
        "Player {killer:?} gained {} base XP, {} job XP, and {} attribute points (Base Lv. {}, Job Lv. {})",
        base_gain.amount,
        job_gain.amount,
        attribute_points_awarded,
        progression.level,
        job_progression.level
    );
    if base_gain.levels_gained > 0 {
        info!(
            "Player {killer:?} gained {} base level(s): {} -> {}",
            base_gain.levels_gained, previous_level, progression.level
        );
    }
    if job_gain.levels_gained > 0 {
        info!(
            "Player {killer:?} gained {} job level(s): {} -> {}",
            job_gain.levels_gained, previous_job_level, job_progression.level
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

fn should_receive_damage_number(
    viewer_entity: Entity,
    damaged_entity: Entity,
    line_of_sight: &LineOfSight,
) -> bool {
    viewer_entity == damaged_entity || line_of_sight.0.contains(&damaged_entity)
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
            .map(|_| roll_basic_attack_damage(&mut rng, 0, false))
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
    fn starting_map_monster_damage_uses_the_beginner_range() {
        let mut rng = StdRng::seed_from_u64(12);
        let rolls: HashSet<u32> = (0..1_024)
            .map(|_| roll_basic_attack_damage(&mut rng, 0, true))
            .collect();

        assert_eq!(rolls, HashSet::from([2, 3, 4]));
    }

    #[test]
    fn might_and_base_level_raise_basic_attack_damage() {
        let stats = CharacterStats {
            might: 3,
            ..default()
        };
        let progression = BaseProgression {
            level: 2,
            experience: 0,
        };
        assert_eq!(
            basic_attack_damage_bonus(Some(&stats), Some(&progression), None),
            5
        );
        assert_eq!(basic_attack_damage_bonus(None, None, None), 0);
    }

    #[test]
    fn finesse_and_agility_supply_character_hit_and_flee() {
        let stats = CharacterStats {
            finesse: 7,
            agility: 5,
            ..default()
        };
        let progression = BaseProgression {
            level: 10,
            experience: 0,
        };

        assert_eq!(
            combat_ratings(Some(&stats), Some(&progression), None, None),
            CombatRatings { hit: 17, flee: 15 }
        );
    }

    #[test]
    fn equipment_attack_and_defenses_modify_combat_values() {
        use crate::shared::gameplay::items::{BASIC_SWORD, CLOTH_ARMOR};

        let stats = CharacterStats::default();
        let progression = BaseProgression::default();
        let mut equipment = Equipment::default();
        equipment.set(EquipmentSlot::MainHand, Some(BASIC_SWORD));
        equipment.set(EquipmentSlot::Armor, Some(CLOTH_ARMOR));

        assert_eq!(
            basic_attack_damage_bonus(Some(&stats), Some(&progression), Some(&equipment)),
            5
        );
        let derived = equipment_derived_stats(&stats, progression.level, &equipment);
        assert_eq!(
            mitigate_damage(10, DamageOrigin::BasicAttack, Some(derived)),
            5
        );
        assert_eq!(
            mitigate_damage(3, DamageOrigin::BasicAttack, Some(derived)),
            1
        );
        assert_eq!(
            mitigate_damage(0, DamageOrigin::BasicAttack, Some(derived)),
            0
        );
    }

    #[test]
    fn hit_chance_uses_rating_difference_and_stays_bounded() {
        assert_eq!(
            basic_attack_hit_chance(
                CombatRatings { hit: 10, flee: 0 },
                CombatRatings { hit: 0, flee: 2 }
            ),
            93
        );
        assert_eq!(
            basic_attack_hit_chance(
                CombatRatings { hit: 999, flee: 0 },
                CombatRatings { hit: 0, flee: 0 }
            ),
            95
        );
        assert_eq!(
            basic_attack_hit_chance(
                CombatRatings { hit: 0, flee: 0 },
                CombatRatings { hit: 0, flee: 999 }
            ),
            20
        );
    }

    #[test]
    fn only_positive_player_damage_applies_walk_delay() {
        assert!(should_apply_damage_walk_delay(5, true, false));
        assert!(!should_apply_damage_walk_delay(0, true, false));
        assert!(!should_apply_damage_walk_delay(5, false, false));
        assert!(!should_apply_damage_walk_delay(5, true, true));
    }

    #[test]
    fn spawn_protection_blocks_positive_player_damage_only() {
        assert!(spawn_protection_blocks_damage(5, true, true));
        assert!(!spawn_protection_blocks_damage(0, true, true));
        assert!(!spawn_protection_blocks_damage(5, false, true));
        assert!(!spawn_protection_blocks_damage(5, true, false));
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
    fn aggressive_monster_does_not_detect_a_spawn_protected_player() {
        let mut app = App::new();
        app.add_systems(Update, acquire_aggressive_monster_targets);
        app.world_mut().spawn((
            Player { id: 1 },
            Transform::from_xyz(2.0, 1.0, 0.0),
            Health {
                current: 40,
                max: 40,
            },
            SpawnProtection::default(),
        ));
        let unprotected = app
            .world_mut()
            .spawn((
                Player { id: 2 },
                Transform::from_xyz(4.0, 1.0, 0.0),
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

        assert_eq!(
            app.world().get::<Aggro>(monster).unwrap().enemy,
            unprotected
        );
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
    fn attack_mode_can_upgrade_to_repeating_and_downgrade_to_once() {
        let enemy = Entity::PLACEHOLDER;
        let mut attacking = Attacking {
            enemy,
            auto_attack: false,
        };
        let mut timer = attack_cycle_timer(0.8, false);

        synchronize_attack_mode(&mut attacking, &mut timer, true);
        assert!(attacking.auto_attack);
        assert_eq!(timer.mode(), TimerMode::Repeating);

        synchronize_attack_mode(&mut attacking, &mut timer, false);
        assert!(!attacking.auto_attack);
        assert_eq!(timer.mode(), TimerMode::Once);
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
            .spawn((
                Player { id: 1 },
                BaseProgression::default(),
                JobProgression::default(),
                SkillTree::default(),
                CharacterStats::default(),
                Equipment::default(),
                Health {
                    current: 1,
                    max: 40,
                },
                Mana {
                    current: 1,
                    max: 10,
                },
            ))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                ExperienceReward { base: 120, job: 80 },
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
        assert_eq!(
            app.world().get::<JobProgression>(killer),
            Some(&JobProgression {
                class: crate::shared::gameplay::progression::CharacterClass::Novice,
                level: 2,
                experience: 40,
            })
        );
        assert_eq!(
            app.world()
                .get::<SkillTree>(killer)
                .unwrap()
                .available_points(),
            1
        );
        assert_eq!(
            app.world()
                .get::<CharacterStats>(killer)
                .unwrap()
                .available_points,
            STARTING_ATTRIBUTE_POINTS + CharacterStats::attribute_points_for_next_base_level(1)
        );
        let health = app.world().get::<Health>(killer).unwrap();
        assert_eq!((health.current, health.max), (45, 45));
        let mana = app.world().get::<Mana>(killer).unwrap();
        assert_eq!((mana.current, mana.max), (12, 12));
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

    #[test]
    fn player_receives_own_damage_number_without_self_in_line_of_sight() {
        let mut world = World::new();
        let viewer = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        let empty_line_of_sight = LineOfSight::default();

        assert!(should_receive_damage_number(
            viewer,
            viewer,
            &empty_line_of_sight
        ));
        assert!(!should_receive_damage_number(
            viewer,
            other,
            &empty_line_of_sight
        ));
    }

    #[test]
    fn monster_aggro_range_includes_the_boundary_but_not_targets_beyond_it() {
        assert!(!should_release_monster_aggro(
            Vec3::ZERO,
            Some(Vec3::new(MONSTER_AGGRO_DETECTION_RANGE, 30.0, 0.0))
        ));
        assert!(should_release_monster_aggro(
            Vec3::ZERO,
            Some(Vec3::new(MONSTER_AGGRO_DETECTION_RANGE + 0.01, 0.0, 0.0))
        ));
        assert!(should_release_monster_aggro(Vec3::ZERO, None));
    }

    #[test]
    fn pursuit_repaths_only_after_the_target_changes_navigation_cells() {
        let walking = Walking {
            target_translation: Vec3::new(4.1, 1.0, -2.1),
            path: None,
        };

        assert!(!pursuit_target_cell_changed(
            &walking,
            Vec3::new(4.49, 8.0, -2.49)
        ));
        assert!(pursuit_target_cell_changed(
            &walking,
            Vec3::new(4.51, 1.0, -2.1)
        ));
    }

    #[test]
    fn monster_repath_requires_its_staggered_timer_to_be_due() {
        assert!(!should_recalculate_pursuit_path(true, false, true, false));
        assert!(should_recalculate_pursuit_path(true, true, true, false));
        assert!(should_recalculate_pursuit_path(true, true, false, true));
        assert!(!should_recalculate_pursuit_path(true, true, false, false));
        assert!(should_recalculate_pursuit_path(false, false, true, false));
    }

    #[test]
    fn monster_repath_phases_are_spread_across_the_six_hz_interval() {
        let phases: Vec<f32> = (0..MONSTER_REPATH_PHASES)
            .map(monster_repath_stagger_seconds)
            .collect();

        assert_eq!(phases[0], 0.0);
        assert!(phases.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(phases.last().copied().unwrap() < MONSTER_REPATH_INTERVAL_SECONDS);
    }

    #[test]
    fn out_of_range_monster_walks_to_its_last_known_target_position() {
        let mut app = App::new();
        app.insert_resource(Map::default());
        app.add_systems(Update, release_out_of_range_monster_aggro);

        let player = app
            .world_mut()
            .spawn((
                Player { id: 1 },
                Transform::from_xyz(MONSTER_AGGRO_DETECTION_RANGE + 1.0, 0.0, 0.0),
                Health {
                    max: 100,
                    current: 100,
                },
            ))
            .id();
        let monster = app
            .world_mut()
            .spawn((
                Monster {
                    hp: 100,
                    kind: MonsterKind::Pig,
                },
                Transform::default(),
                Aggro {
                    enemy: player,
                    auto_attack: true,
                    enemy_translation: Vec3::X * 5.0,
                },
                Attacking {
                    enemy: player,
                    auto_attack: true,
                },
                AttackingTimer(Timer::from_seconds(1.0, TimerMode::Repeating)),
                Walking {
                    target_translation: Vec3::X,
                    path: None,
                },
                TargetPos { position: Vec3::X },
                GameVelocity(Vec3::X),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<GameVelocity>(monster).unwrap().0,
            Vec3::ZERO
        );
        assert!(app.world().get::<Aggro>(monster).is_none());
        assert!(app.world().get::<Attacking>(monster).is_none());
        assert!(app.world().get::<AttackingTimer>(monster).is_none());
        let walking = app.world().get::<Walking>(monster).unwrap();
        assert_eq!(walking.target_translation, Vec3::X * 5.0);
        assert!(walking
            .path
            .as_ref()
            .is_some_and(|(steps, _)| steps.len() > 1));
        assert!(app.world().get::<TargetPos>(monster).is_none());
    }
}

fn on_player_death(
    trigger: On<DeathEvent>,
    mut players: Query<
        (
            &LineOfSight,
            &mut BaseProgression,
            Option<&mut GameVelocity>,
            Option<&mut KinematicCharacterController>,
        ),
        With<Player>,
    >,
    viewers: Query<(Entity, &Player, &LineOfSight)>,
    server: Option<ResMut<RenetServer>>,
    mut commands: Commands,
) {
    let dead_entity = trigger.event().entity;
    let Ok((player_line_of_sight, mut progression, velocity, controller)) =
        players.get_mut(dead_entity)
    else {
        return;
    };

    let experience_lost = progression.apply_death_penalty();
    if let Some(mut velocity) = velocity {
        velocity.0 = Vec3::ZERO;
    }
    if let Some(mut controller) = controller {
        controller.translation = None;
    }

    commands
        .entity(dead_entity)
        .try_insert((Dead, PlayerInput::default()))
        .try_remove::<Sitting>()
        .try_remove::<Aggro>()
        .try_remove::<Attacking>()
        .try_remove::<AttackingTimer>()
        .try_remove::<Walking>()
        .try_remove::<TargetPos>()
        .try_remove::<crate::server::gameplay::items::PendingItemPickup>()
        .try_remove::<crate::server::gameplay::spells::AuthoritativeCast>();

    let message = bincode::serialize(&ServerMessages::PlayerDied {
        entity: dead_entity,
        experience_lost,
    })
    .expect("player death message should serialize");
    if let Some(mut server) = server {
        for (viewer_entity, viewer, line_of_sight) in &viewers {
            if should_receive_player_action(
                viewer_entity,
                dead_entity,
                line_of_sight,
                player_line_of_sight,
            ) {
                server.send_message(viewer.id, ServerChannel::ServerMessages, message.clone());
            }
        }
    }
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.add_systems(
            FixedUpdate,
            (
                sync_derived_resource_maxima.run_if(in_state(ServerState::InGame)),
                network_change_attacking_state.run_if(in_state(ServerState::InGame)),
                network_send_delta_health_system.run_if(in_state(ServerState::InGame)),
                network_send_progression_system.run_if(in_state(ServerState::InGame)),
                (
                    release_monsters_targeting_protected_players
                        .run_if(in_state(ServerState::InGame)),
                    release_out_of_range_monster_aggro.run_if(in_state(ServerState::InGame)),
                    acquire_aggressive_monster_targets.run_if(in_state(ServerState::InGame)),
                    recalculate_path.run_if(in_state(ServerState::InGame)),
                    aggro_rapier3d
                        .run_if(in_state(ServerState::InGame))
                        .before(crate::server::gameplay::pathing::apply_rapier3d_velocity_system),
                    attack.run_if(in_state(ServerState::InGame)),
                )
                    .chain(),
            ),
        )
        .add_observer(provoke_monster_on_damage)
        .add_observer(provoke_spell_reactive_monster)
        .add_observer(on_death_stop_attackers)
        .add_observer(on_health_change)
        .add_observer(on_player_death)
        .add_observer(on_death_give_experience)
        .add_observer(on_death_despawn_monsters);

        fn aggro_rapier3d(
            mut aggroed_entities: Query<
                (Entity, &Transform, &Aggro, Option<&mut Attacking>),
                (Or<(With<Player>, With<NPC>, With<Monster>)>),
            >,
            //attacked_entities: Query<(Entity, &Transform), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut commands: Commands,
            //spatial_query: SpatialQuery,
            // rapier_context: Res<RapierContext>,
            read_rapier_context: ReadRapierContext,
            map_query: Query<&MapEntity>,
        ) {
            if let Ok(rapier_context) = read_rapier_context.single() {
                for (entity, attacker_transform, aggroed, is_attacking) in
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
                        if let Some(attacking) = is_attacking {
                            if attacking.enemy == aggroed.enemy {
                                continue;
                            }
                            commands
                                .entity(entity)
                                .try_remove::<Attacking>()
                                .try_remove::<AttackingTimer>();
                            continue;
                        }
                        //info!("ATACARRRRRRRRRRRR");
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

                    commands
                        .entity(entity)
                        .try_remove::<Attacking>()
                        .try_remove::<AttackingTimer>();

                    // Pursuit movement is exclusively created and refreshed by
                    // `recalculate_path`, keeping A* in one throttled system.
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
                (
                    Entity,
                    &Aggro,
                    &mut Attacking,
                    &AttackSpeed,
                    &mut AttackingTimer,
                ),
                (Or<(With<Player>, With<NPC>, With<Monster>)>),
            >,
            mut commands: Commands,
            time: Res<Time>,
            combatants: Query<(
                Option<&CharacterStats>,
                Option<&BaseProgression>,
                Option<&Equipment>,
                Option<&Monster>,
                Option<&StartingMapMonster>,
            )>,
        ) {
            let mut rng = rand::thread_rng();
            for (entity, aggro, mut attacking, attack_speed, mut attacking_timer) in
                attacking_entities.iter_mut()
            {
                synchronize_attack_mode(&mut attacking, &mut attacking_timer.0, aggro.auto_attack);
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
                let (attacker_ratings, damage_bonus, starting_map_monster) = combatants
                    .get(entity)
                    .map(
                        |(stats, progression, equipment, monster, starting_map_monster)| {
                            (
                                combat_ratings(stats, progression, equipment, monster),
                                basic_attack_damage_bonus(stats, progression, equipment),
                                starting_map_monster.is_some(),
                            )
                        },
                    )
                    .unwrap_or((CombatRatings { hit: 1, flee: 1 }, 0, false));
                let defender_ratings = combatants
                    .get(attacking.enemy)
                    .map(|(stats, progression, equipment, monster, _)| {
                        combat_ratings(stats, progression, equipment, monster)
                    })
                    .unwrap_or(CombatRatings { hit: 1, flee: 1 });
                let hit_chance = basic_attack_hit_chance(attacker_ratings, defender_ratings);
                let damage = if roll_basic_attack_hit(&mut rng, hit_chance) {
                    roll_basic_attack_damage(&mut rng, damage_bonus, starting_map_monster)
                } else {
                    0
                };
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
                (
                    Entity,
                    Option<&mut Walking>,
                    &Transform,
                    &mut Aggro,
                    Option<&Monster>,
                    Option<&Attacking>,
                    Option<&mut MonsterRepathSchedule>,
                ),
                (Or<(With<Player>, With<NPC>, With<Monster>)>),
            >,
            enemies: Query<&Transform, Or<(With<Player>, With<NPC>, With<Monster>)>>,
            time: Res<Time>,
            map: Res<Map>,
            mut commands: Commands,
        ) {
            let map_changed = map.is_changed();
            for (
                entity,
                walking,
                attacker_transform,
                mut aggroed,
                monster,
                attacking,
                repath_schedule,
            ) in attackers.iter_mut()
            {
                let Ok(enemy_transform) = enemies.get(aggroed.enemy) else {
                    continue;
                };
                let target_translation = enemy_transform.translation;
                aggroed.enemy_translation = target_translation;
                if attacking.is_some() {
                    continue;
                }
                let target_cell_changed = walking
                    .as_ref()
                    .is_none_or(|walking| pursuit_target_cell_changed(walking, target_translation));
                let is_monster = monster.is_some();
                let mut repath_schedule = repath_schedule;

                let (timer_due, map_change_pending) = if is_monster {
                    let Some(schedule) = repath_schedule.as_deref_mut() else {
                        commands
                            .entity(entity)
                            .try_insert(MonsterRepathSchedule::new(entity, map_changed));
                        continue;
                    };
                    schedule.map_change_pending |= map_changed;
                    let timer_due = schedule.timer.tick(time.delta()).just_finished();
                    (timer_due, schedule.map_change_pending)
                } else {
                    (true, map_changed)
                };

                if !should_recalculate_pursuit_path(
                    is_monster,
                    timer_due,
                    target_cell_changed,
                    map_change_pending,
                ) {
                    continue;
                }

                let path = get_path_between_translations(
                    attacker_transform.translation,
                    target_translation,
                    &map,
                );
                if let Some(mut walking) = walking {
                    walking.path = path;
                    walking.target_translation = target_translation;
                } else {
                    commands.entity(entity).try_insert(Walking {
                        target_translation,
                        path,
                    });
                }
                if let Some(mut schedule) = repath_schedule {
                    schedule.map_change_pending = false;
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
            mut query: Query<(
                Entity,
                &mut Health,
                Option<&Player>,
                Option<&Transform>,
                Option<&Walking>,
                Option<&DamageWalkDelayImmunity>,
                Option<&mut GameVelocity>,
                Option<&mut KinematicCharacterController>,
                Option<&CharacterStats>,
                Option<&BaseProgression>,
                Option<&Equipment>,
                Option<&SpawnProtection>,
            )>,
            mut commands: Commands,
            mut server: ResMut<RenetServer>,
            players: Query<(Entity, &Player, &LineOfSight)>,
            time: Res<Time>,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let health_change: &HealthChange = trigger.event();
            let id: Entity = health_change.entity;

            if let Ok((
                entity,
                mut health,
                damaged_player,
                transform,
                walking,
                walk_delay_immunity,
                velocity,
                controller,
                stats,
                progression,
                equipment,
                spawn_protection,
            )) = query.get_mut(id)
            {
                if health.current == 0 {
                    return;
                }
                if spawn_protection_blocks_damage(
                    health_change.damage,
                    damaged_player.is_some(),
                    spawn_protection.is_some(),
                ) {
                    return;
                }
                info!("Entity  {:?} damaged.", id.index());
                let derived = stats.map(|stats| {
                    let level = progression.map_or(1, |progression| progression.level);
                    equipment.map_or_else(
                        || stats.derived(level),
                        |equipment| equipment_derived_stats(stats, level, equipment),
                    )
                });
                let applied_damage =
                    mitigate_damage(health_change.damage, health_change.origin, derived);

                if should_apply_damage_walk_delay(
                    applied_damage,
                    damaged_player.is_some(),
                    walk_delay_immunity.is_some(),
                ) {
                    let pending_destination = walking.map(|walking| walking.target_translation);
                    if let Some(mut velocity) = velocity {
                        velocity.0 = Vec3::ZERO;
                    }
                    if let Some(mut controller) = controller {
                        controller.translation = None;
                    }
                    commands
                        .entity(entity)
                        .try_remove::<Walking>()
                        .try_remove::<TargetPos>()
                        .try_insert(DamageWalkDelay::new(pending_destination))
                        .try_insert(DamageWalkDelayImmunity::default());

                    if let (Some(player), Some(transform)) = (damaged_player, transform) {
                        let movement_message =
                            bincode::serialize(&ServerMessages::MovementInterrupted {
                                entity,
                                translation: transform.translation.into(),
                                server_time: time.elapsed().as_millis(),
                            })
                            .expect("movement interruption should serialize");
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            movement_message,
                        );
                    }
                }

                let message = bincode::serialize(&ServerMessages::DamageNumber {
                    entity,
                    amount: i32::try_from(applied_damage).unwrap_or(i32::MAX),
                })
                .unwrap();
                for (viewer_entity, player, line_of_sight) in &players {
                    if should_receive_damage_number(viewer_entity, entity, line_of_sight) {
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            message.clone(),
                        );
                    }
                }

                if health.current <= applied_damage {
                    health.current = 0;
                    commands.trigger(DeathEvent {
                        entity: health_change.entity,
                        killer: health_change.source,
                    });
                } else {
                    health.current -= applied_damage;
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
            players: Query<(Entity, &Player, &LineOfSight)>,
            entities: Query<(Entity, &Health), Changed<Health>>,
            //time: Res<Time>,
        ) {
            for (entity, health) in &entities {
                let message = ServerMessages::HealthChange {
                    entity,
                    amount: 5,
                    max: health.max,
                    current: health.current,
                };
                let sync_message = bincode::serialize(&message).unwrap();

                for (viewer_entity, player, line_of_sight) in &players {
                    if should_receive_damage_number(viewer_entity, entity, line_of_sight) {
                        server.send_message(
                            player.id,
                            ServerChannel::ServerMessages,
                            sync_message.clone(),
                        );
                    }
                }
            }
        }

        pub fn network_send_progression_system(
            mut server: ResMut<RenetServer>,
            viewers: Query<(Entity, &Player, &LineOfSight)>,
            changed_progression: Query<
                (Entity, &BaseProgression, &JobProgression),
                Or<(Changed<BaseProgression>, Changed<JobProgression>)>,
            >,
        ) {
            for (entity, progression, job_progression) in &changed_progression {
                let message = bincode::serialize(&ServerMessages::ProgressionChanged {
                    entity,
                    progression: *progression,
                    job_progression: *job_progression,
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
