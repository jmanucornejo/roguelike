use super::pathing::get_path_between_translations;
use super::spatial::NearestNeighbourComponent;
use crate::server::network::replication::PrevState;
use crate::shared::constants::WATER_LEVEL;
use crate::shared::gameplay::components::{
    player_character_controller, Aggro, Attacking, Facing, GameVelocity, Health, Map, Monster,
    MonsterAggression, MonsterKind, Pos, SpriteId, Walking, AGGRESSIVE_MONSTER_PLACEHOLDER_SPRITE,
    PASSIVE_MONSTER_PLACEHOLDER_SPRITE, SPELL_REACTIVE_MONSTER_PLACEHOLDER_SPRITE,
};
use crate::shared::gameplay::entities::{AttackSpeed, Player};
use crate::shared::gameplay::events::DeathEvent;
use crate::shared::gameplay::progression::ExperienceReward;
use crate::shared::states::ServerState;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy_sprite3d::*;
use rand::Rng;
use std::collections::HashSet;

pub struct MonstersPlugin;

const MAP_MIN_CELL: i32 = -150;
const MAP_MAX_CELL: i32 = 150;
const MONSTER_SPAWN_ATTEMPTS: usize = 128;
const MAX_RESPAWN_ATTEMPTS_PER_UPDATE: usize = 4;
const MONSTER_COLLIDER_HALF_HEIGHT: f32 = 1.0;
const TERRAIN_RAY_ORIGIN_Y: f32 = 128.0;
const TERRAIN_RAY_DISTANCE: f32 = 256.0;
const ROAM_MIN_DISTANCE: i32 = 2;
const ROAM_VISION_RANGE: i32 = 8;
const ROAM_DESTINATION_ATTEMPTS: usize = 32;
const IDLE_MIN_SECONDS: f32 = 5.0;
const IDLE_MAX_SECONDS: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
struct MonsterPopulationDefinition {
    kind: MonsterKind,
    aggression: MonsterAggression,
    target_count: usize,
    respawn_min_seconds: u64,
    respawn_max_seconds: u64,
}

// This is the population definition for the current map. When maps become
// separate assets, this table can move into each map's data file unchanged.
const CURRENT_MAP_POPULATION: [MonsterPopulationDefinition; 3] = [
    MonsterPopulationDefinition {
        kind: MonsterKind::Pig,
        aggression: MonsterAggression::Passive,
        target_count: 40,
        respawn_min_seconds: 0,
        respawn_max_seconds: 60,
    },
    MonsterPopulationDefinition {
        kind: MonsterKind::Pig,
        aggression: MonsterAggression::Aggressive,
        target_count: 1,
        respawn_min_seconds: 0,
        respawn_max_seconds: 60,
    },
    MonsterPopulationDefinition {
        kind: MonsterKind::Pig,
        aggression: MonsterAggression::SpellReactive,
        target_count: 1,
        respawn_min_seconds: 0,
        respawn_max_seconds: 60,
    },
];

#[derive(Component)]
pub struct MonsterParent;

#[derive(AssetCollection, Resource, Debug)]
struct TestAssets {
    #[asset(texture_atlas_layout(
        tile_size_x = 24,
        tile_size_y = 24,
        columns = 7,
        rows = 1,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "gabe-idle-run.png")]
    sprite: Handle<Image>,
}

#[derive(Debug, PartialEq, Component, Clone)]
pub struct MonsterMovement {
    pub move_timer: Timer,
    pub speed: f32,
}

#[derive(Event)]
struct SpawnMonster {
    kind: MonsterKind,
    aggression: MonsterAggression,
    translation: Vec3,
}

#[derive(Debug)]
struct PendingMonsterRespawn {
    kind: MonsterKind,
    aggression: MonsterAggression,
    timer: Timer,
    spawn_near_player: bool,
}

#[derive(Default, Resource)]
struct MonsterPopulation {
    pending: Vec<PendingMonsterRespawn>,
}

impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MonsterPopulation>()
            .add_systems(OnEnter(ServerState::Initializing), spawn_monster_parent)
            .add_plugins(Sprite3dPlugin)
            .add_loading_state(
                LoadingState::new(ServerState::Initializing)
                    .load_collection::<TestAssets>()
                    .continue_to_state(ServerState::InGame),
            )
            .add_systems(
                Update,
                maintain_monster_population.run_if(in_state(ServerState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                roam_idle_monsters.run_if(in_state(ServerState::InGame)),
            )
            .add_observer(spawn_monster)
            .add_observer(schedule_monster_respawn);
    }
}

fn spawn_monster_parent(mut commands: Commands) {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        MonsterParent,
        Name::new("Monster Parent"),
    ));
}

fn spawn_monster(
    trigger: On<SpawnMonster>,
    parent: Query<Entity, With<MonsterParent>>,
    assets: Res<TestAssets>,
    mut commands: Commands,
) {
    let spawn = trigger.event();
    let texture_atlas = TextureAtlas {
        layout: assets.layout.clone(),
        index: 3,
    };
    let (name, max_health): (&str, u32) = match spawn.kind {
        MonsterKind::Pig => ("Pig", 100),
        MonsterKind::Orc => ("Orc", 180),
    };

    let mut monster_commands = commands.spawn((
        Transform::from_translation(spawn.translation),
        Sprite3d {
            pixels_per_metre: 32.0,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        },
        Sprite {
            image: assets.sprite.clone(),
            texture_atlas: Some(texture_atlas),
            ..default()
        },
        Monster {
            hp: max_health as i32,
            kind: spawn.kind,
        },
        MonsterMovement {
            move_timer: new_idle_timer(&mut rand::thread_rng()),
            speed: 5.0,
        },
        Name::new(name),
        Collider::capsule_y(0.5, 0.5),
        RigidBody::KinematicPositionBased,
        GameVelocity::default(),
        Facing(0),
        SpriteId(match spawn.aggression {
            MonsterAggression::Passive => PASSIVE_MONSTER_PLACEHOLDER_SPRITE,
            MonsterAggression::Aggressive => AGGRESSIVE_MONSTER_PLACEHOLDER_SPRITE,
            MonsterAggression::SpellReactive => SPELL_REACTIVE_MONSTER_PLACEHOLDER_SPRITE,
        }),
        PrevState {
            translation: spawn.translation,
            rotation: Facing(0),
        },
        NearestNeighbourComponent,
    ));
    monster_commands.insert((
        spawn.aggression,
        Health {
            max: max_health,
            current: max_health,
        },
        AttackSpeed(0.5),
        ExperienceReward::for_monster_kind(&spawn.kind),
        LockedAxes::ROTATION_LOCKED,
        ActiveCollisionTypes::KINEMATIC_STATIC,
        TransformInterpolation::default(),
        player_character_controller(),
    ));

    if let Ok(parent) = parent.single() {
        monster_commands.insert(ChildOf(parent));
    }
}

fn schedule_monster_respawn(
    trigger: On<DeathEvent>,
    monsters: Query<(&Monster, &MonsterAggression)>,
    mut population: ResMut<MonsterPopulation>,
) {
    let Ok((monster, aggression)) = monsters.get(trigger.event().entity) else {
        return;
    };
    let Some(definition) = population_definition(monster.kind, *aggression) else {
        return;
    };
    let delay = random_respawn_seconds(definition, &mut rand::thread_rng());
    population.pending.push(PendingMonsterRespawn {
        kind: monster.kind,
        aggression: *aggression,
        timer: Timer::from_seconds(delay as f32, TimerMode::Once),
        spawn_near_player: false,
    });
    info!(
        "Scheduled {:?} respawn in {} second(s)",
        monster.kind, delay
    );
}

fn maintain_monster_population(
    time: Res<Time>,
    map: Res<Map>,
    read_rapier_context: ReadRapierContext,
    monsters: Query<(&Monster, &MonsterAggression, &Transform)>,
    players: Query<&Transform, With<Player>>,
    mut population: ResMut<MonsterPopulation>,
    mut commands: Commands,
) {
    for definition in CURRENT_MAP_POPULATION {
        let live = monsters
            .iter()
            .filter(|(monster, aggression, _)| {
                monster.kind == definition.kind && **aggression == definition.aggression
            })
            .count();
        let scheduled = population
            .pending
            .iter()
            .filter(|pending| {
                pending.kind == definition.kind && pending.aggression == definition.aggression
            })
            .count();
        for _ in (live + scheduled)..definition.target_count {
            population.pending.push(PendingMonsterRespawn {
                kind: definition.kind,
                aggression: definition.aggression,
                timer: Timer::from_seconds(0.0, TimerMode::Once),
                spawn_near_player: definition.aggression != MonsterAggression::Passive,
            });
        }
    }

    let Ok(rapier_context) = read_rapier_context.single() else {
        return;
    };
    let mut occupied: HashSet<Pos> = monsters
        .iter()
        .map(|(_, _, transform)| world_cell(transform.translation))
        .collect();
    for pending in &mut population.pending {
        pending.timer.tick(time.delta());
    }

    let mut rng = rand::thread_rng();
    let mut index = 0;
    let mut attempts = 0;
    while index < population.pending.len() && attempts < MAX_RESPAWN_ATTEMPTS_PER_UPDATE {
        if !population.pending[index].timer.is_finished() {
            index += 1;
            continue;
        }
        attempts += 1;

        let pending = &population.pending[index];
        let preferred_player = pending
            .spawn_near_player
            .then(|| players.iter().next().map(|transform| transform.translation))
            .flatten();
        if pending.spawn_near_player && preferred_player.is_none() {
            index += 1;
            continue;
        }
        let mut surface_height = |cell: Pos| {
            let origin = Vec3::new(cell.0 as f32, TERRAIN_RAY_ORIGIN_Y, cell.1 as f32);
            rapier_context
                .cast_ray(
                    origin,
                    Vec3::NEG_Y,
                    TERRAIN_RAY_DISTANCE,
                    true,
                    QueryFilter::only_fixed().exclude_sensors(),
                )
                .map(|(_, time_of_impact)| TERRAIN_RAY_ORIGIN_Y - time_of_impact)
        };
        let translation = if let Some(player_translation) = preferred_player {
            random_valid_spawn_translation_near(
                &map,
                &occupied,
                world_cell(player_translation),
                &mut rng,
                &mut surface_height,
            )
        } else {
            random_valid_spawn_translation(&map, &occupied, &mut rng, &mut surface_height)
        };
        let Some(translation) = translation else {
            // Async terrain colliders may not be ready yet. Keep the completed
            // respawn queued and retry on the next update.
            index += 1;
            continue;
        };

        let pending = population.pending.swap_remove(index);
        occupied.insert(world_cell(translation));
        commands.trigger(SpawnMonster {
            kind: pending.kind,
            aggression: pending.aggression,
            translation,
        });
    }
}

fn roam_idle_monsters(
    mut monsters: Query<
        (Entity, &mut MonsterMovement, &Transform, Option<&Walking>),
        (With<Monster>, Without<Aggro>, Without<Attacking>),
    >,
    time: Res<Time>,
    mut commands: Commands,
    map: Res<Map>,
) {
    let mut rng = rand::thread_rng();
    for (entity, mut movement, transform, walking) in &mut monsters {
        // The timer represents idle time, so travelling does not consume it.
        if walking.is_some() {
            continue;
        }

        movement.move_timer.tick(time.delta());
        if !movement.move_timer.is_finished() {
            continue;
        }

        // Reset now, but do not tick again until Walking is removed at arrival.
        movement.move_timer = new_idle_timer(&mut rng);
        if let Some((path, destination)) =
            random_reachable_roam_path(transform.translation, &map, &mut rng)
        {
            debug!(
                "Monster {:?} roaming from {:?} to {:?}",
                entity, transform.translation, destination
            );
            commands.entity(entity).try_insert(Walking {
                target_translation: destination,
                path: Some(path),
            });
        }
    }
}

fn random_reachable_roam_path(
    origin: Vec3,
    map: &Map,
    rng: &mut impl Rng,
) -> Option<((Vec<Pos>, u32), Vec3)> {
    let origin_cell = world_cell(origin);
    let min_distance_squared = ROAM_MIN_DISTANCE * ROAM_MIN_DISTANCE;
    let vision_range_squared = ROAM_VISION_RANGE * ROAM_VISION_RANGE;

    for _ in 0..ROAM_DESTINATION_ATTEMPTS {
        let offset_x = rng.gen_range(-ROAM_VISION_RANGE..=ROAM_VISION_RANGE);
        let offset_z = rng.gen_range(-ROAM_VISION_RANGE..=ROAM_VISION_RANGE);
        let distance_squared = offset_x * offset_x + offset_z * offset_z;
        if distance_squared < min_distance_squared || distance_squared > vision_range_squared {
            continue;
        }

        let destination_cell = Pos(origin_cell.0 + offset_x, origin_cell.1 + offset_z);
        if !(MAP_MIN_CELL..=MAP_MAX_CELL).contains(&destination_cell.0)
            || !(MAP_MIN_CELL..=MAP_MAX_CELL).contains(&destination_cell.1)
            || map.blocked_paths.contains(&destination_cell)
        {
            continue;
        }

        let destination = Vec3::new(
            destination_cell.0 as f32,
            origin.y,
            destination_cell.1 as f32,
        );
        if let Some(path) = get_path_between_translations(origin, destination, map) {
            if path.0.len() > 1 {
                return Some((path, destination));
            }
        }
    }

    None
}

fn new_idle_timer(rng: &mut impl Rng) -> Timer {
    Timer::from_seconds(
        rng.gen_range(IDLE_MIN_SECONDS..=IDLE_MAX_SECONDS),
        TimerMode::Once,
    )
}

fn population_definition(
    kind: MonsterKind,
    aggression: MonsterAggression,
) -> Option<MonsterPopulationDefinition> {
    CURRENT_MAP_POPULATION
        .iter()
        .copied()
        .find(|definition| definition.kind == kind && definition.aggression == aggression)
}

fn random_respawn_seconds(definition: MonsterPopulationDefinition, rng: &mut impl Rng) -> u64 {
    rng.gen_range(definition.respawn_min_seconds..=definition.respawn_max_seconds)
}

fn random_valid_spawn_translation(
    map: &Map,
    occupied: &HashSet<Pos>,
    rng: &mut impl Rng,
    mut surface_height: impl FnMut(Pos) -> Option<f32>,
) -> Option<Vec3> {
    for _ in 0..MONSTER_SPAWN_ATTEMPTS {
        let cell = Pos(
            rng.gen_range(MAP_MIN_CELL..=MAP_MAX_CELL),
            rng.gen_range(MAP_MIN_CELL..=MAP_MAX_CELL),
        );
        if let Some(translation) =
            valid_spawn_translation_for_cell(map, occupied, cell, surface_height(cell))
        {
            return Some(translation);
        }
    }
    None
}

fn random_valid_spawn_translation_near(
    map: &Map,
    occupied: &HashSet<Pos>,
    center: Pos,
    rng: &mut impl Rng,
    mut surface_height: impl FnMut(Pos) -> Option<f32>,
) -> Option<Vec3> {
    for _ in 0..MONSTER_SPAWN_ATTEMPTS {
        let offset = Pos(rng.gen_range(-6..=6), rng.gen_range(-6..=6));
        let distance_squared = offset.0 * offset.0 + offset.1 * offset.1;
        if !(4..=36).contains(&distance_squared) {
            continue;
        }
        let cell = Pos(center.0 + offset.0, center.1 + offset.1);
        if let Some(translation) =
            valid_spawn_translation_for_cell(map, occupied, cell, surface_height(cell))
        {
            return Some(translation);
        }
    }
    None
}

fn valid_spawn_translation_for_cell(
    map: &Map,
    occupied: &HashSet<Pos>,
    cell: Pos,
    surface_height: Option<f32>,
) -> Option<Vec3> {
    if map.blocked_paths.contains(&cell) || occupied.contains(&cell) {
        return None;
    }
    let floor_y = surface_height?;
    if floor_y < WATER_LEVEL {
        return None;
    }
    Some(Vec3::new(
        cell.0 as f32,
        floor_y + MONSTER_COLLIDER_HALF_HEIGHT,
        cell.1 as f32,
    ))
}

fn world_cell(translation: Vec3) -> Pos {
    Pos(translation.x.round() as i32, translation.z.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn current_map_has_a_fixed_pig_population() {
        let pigs = population_definition(MonsterKind::Pig, MonsterAggression::Passive).unwrap();
        assert_eq!(pigs.target_count, 40);
        assert_eq!(pigs.respawn_min_seconds, 0);
        assert_eq!(pigs.respawn_max_seconds, 60);

        let aggressive =
            population_definition(MonsterKind::Pig, MonsterAggression::Aggressive).unwrap();
        assert_eq!(aggressive.target_count, 1);

        let spell_reactive =
            population_definition(MonsterKind::Pig, MonsterAggression::SpellReactive).unwrap();
        assert_eq!(spell_reactive.target_count, 1);
    }

    #[test]
    fn respawn_rolls_stay_inside_the_configured_window() {
        let definition =
            population_definition(MonsterKind::Pig, MonsterAggression::Passive).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1_024 {
            let seconds = random_respawn_seconds(definition, &mut rng);
            assert!((0..=60).contains(&seconds));
        }
    }

    #[test]
    fn spawn_selection_rejects_blocked_occupied_missing_and_submerged_cells() {
        let mut map = Map::default();
        let mut occupied = HashSet::new();
        let allowed = Pos(12, -8);
        let blocked = Pos(1, 1);
        let occupied_cell = Pos(2, 2);
        map.blocked_paths.insert(blocked);
        occupied.insert(occupied_cell);

        assert_eq!(
            valid_spawn_translation_for_cell(&map, &occupied, blocked, Some(2.5)),
            None
        );
        assert_eq!(
            valid_spawn_translation_for_cell(&map, &occupied, occupied_cell, Some(2.5)),
            None
        );
        assert_eq!(
            valid_spawn_translation_for_cell(&map, &occupied, allowed, None),
            None
        );
        assert_eq!(
            valid_spawn_translation_for_cell(&map, &occupied, allowed, Some(WATER_LEVEL - 0.01)),
            None
        );
        let translation =
            valid_spawn_translation_for_cell(&map, &occupied, allowed, Some(2.5)).unwrap();
        assert_eq!(translation, Vec3::new(12.0, 3.5, -8.0));
    }

    #[test]
    fn test_population_spawn_is_kept_near_player() {
        let map = Map::default();
        let occupied = HashSet::new();
        let player_cell = Pos(20, -10);
        let mut rng = StdRng::seed_from_u64(13);

        let spawn =
            random_valid_spawn_translation_near(&map, &occupied, player_cell, &mut rng, |_| {
                Some(2.0)
            })
            .expect("a nearby valid spawn should be found");
        let spawn_cell = world_cell(spawn);
        let dx = spawn_cell.0 - player_cell.0;
        let dz = spawn_cell.1 - player_cell.1;
        let distance_squared = dx * dx + dz * dz;

        assert!((4..=36).contains(&distance_squared));
        assert_eq!(spawn.y, 2.0 + MONSTER_COLLIDER_HALF_HEIGHT);
    }

    #[test]
    fn roaming_builds_a_reachable_multi_step_path_from_the_current_cell() {
        let map = Map::default();
        let origin = Vec3::new(0.0, 1.0, 0.0);
        let mut rng = StdRng::seed_from_u64(99);

        let (path, destination) = random_reachable_roam_path(origin, &map, &mut rng).unwrap();
        let destination_cell = world_cell(destination);
        let distance_squared =
            destination_cell.0 * destination_cell.0 + destination_cell.1 * destination_cell.1;

        assert_eq!(path.0.first(), Some(&Pos(0, 0)));
        assert_eq!(path.0.last(), Some(&destination_cell));
        assert!(path.0.len() >= 2);
        assert_eq!(path.1, 0);
        assert!(
            (ROAM_MIN_DISTANCE * ROAM_MIN_DISTANCE..=ROAM_VISION_RANGE * ROAM_VISION_RANGE)
                .contains(&distance_squared)
        );
        assert_eq!(
            path.0.iter().copied().collect::<HashSet<_>>().len(),
            path.0.len()
        );
        for cells in path.0.windows(2) {
            let dx = (cells[1].0 - cells[0].0).abs();
            let dz = (cells[1].1 - cells[0].1).abs();
            assert!(dx <= 1 && dz <= 1 && (dx != 0 || dz != 0));
        }
    }

    #[test]
    fn roaming_starts_only_for_idle_non_aggro_monsters() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Map::default())
            .add_systems(Update, roam_idle_monsters);

        let spawn_monster = |world: &mut World| {
            world
                .spawn((
                    Monster {
                        hp: 100,
                        kind: MonsterKind::Pig,
                    },
                    MonsterMovement {
                        move_timer: Timer::from_seconds(0.0, TimerMode::Once),
                        speed: 5.0,
                    },
                    Transform::from_xyz(0.0, 1.0, 0.0),
                ))
                .id()
        };
        let idle = spawn_monster(app.world_mut());
        let aggroed = spawn_monster(app.world_mut());
        let enemy = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(aggroed).insert(Aggro {
            enemy,
            auto_attack: true,
            enemy_translation: Vec3::X,
        });
        let attacking = spawn_monster(app.world_mut());
        app.world_mut().entity_mut(attacking).insert(Attacking {
            enemy,
            auto_attack: true,
        });

        app.update();

        assert!(app.world().get::<Walking>(idle).is_some());
        assert!(app.world().get::<Walking>(aggroed).is_none());
        assert!(app.world().get::<Walking>(attacking).is_none());
    }
}
