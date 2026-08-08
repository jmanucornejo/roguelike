use std::collections::HashMap;

use bevy::{asset::LoadState, light::NotShadowCaster, prelude::*};
use bevy_panorbit_camera::PanOrbitCameraSystemSet;
use bevy_sprite3d::prelude::*;

use crate::{
    client::presentation::{
        animations::{
            animation_world_direction, atlas_direction, AttackSpriteVisual, LastAnimationDirection,
            WalkingSpriteVisual,
        },
        damage_numbers::DamageNumberEvent,
    },
    shared::{
        gameplay::{
            components::{
                Animation, Billboard, Dead, Equipment, EquipmentSlot, Facing, GameVelocity, Sitting,
            },
            entities::Player,
            items::APPRENTICE_STAFF,
            progression::{CharacterClass, JobProgression},
        },
        states::ClientState,
    },
};

const CELL_SIZE: UVec2 = UVec2::new(128, 128);
const JOB_SPRITE_PIXELS_PER_METRE: f32 = 48.0;
const JOB_SPRITE_LOCAL_Y: f32 = -1.0;

#[derive(Component)]
pub(crate) struct JobAnimatedPlayer;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum JobAnimationKind {
    Walk,
    Idle,
    Sit,
    Death,
    Pickup,
    Cast,
    Hit,
    Attack1,
    Attack2,
    Legacy,
}

const MODERN_ANIMATIONS: [JobAnimationKind; 9] = [
    JobAnimationKind::Walk,
    JobAnimationKind::Idle,
    JobAnimationKind::Sit,
    JobAnimationKind::Death,
    JobAnimationKind::Pickup,
    JobAnimationKind::Cast,
    JobAnimationKind::Hit,
    JobAnimationKind::Attack1,
    JobAnimationKind::Attack2,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobSpriteSource {
    Modern,
    Legacy,
}

#[derive(Clone)]
struct JobSpritePackage {
    source: JobSpriteSource,
    images: Vec<(JobAnimationKind, Handle<Image>)>,
}

impl JobSpritePackage {
    fn is_loaded(&self, asset_server: &AssetServer) -> bool {
        self.images.iter().all(|(_, image)| {
            matches!(
                asset_server.get_load_state(image.id()),
                Some(LoadState::Loaded)
            )
        })
    }

    fn failed_path(&self, asset_server: &AssetServer) -> Option<String> {
        self.images.iter().find_map(|(_, image)| {
            let Some(LoadState::Failed(error)) = asset_server.get_load_state(image.id()) else {
                return None;
            };
            Some(error.to_string())
        })
    }
}

#[derive(Resource, Default)]
struct JobSpriteAssetCache {
    packages: HashMap<CharacterClass, JobSpritePackage>,
}

impl JobSpriteAssetCache {
    fn package(&mut self, class: CharacterClass, asset_server: &AssetServer) -> JobSpritePackage {
        self.packages
            .entry(class)
            .or_insert_with(|| load_job_package(class, asset_server))
            .clone()
    }
}

#[derive(Resource)]
struct JobAnimationLayouts {
    layouts: HashMap<JobAnimationKind, Handle<TextureAtlasLayout>>,
}

impl FromWorld for JobAnimationLayouts {
    fn from_world(world: &mut World) -> Self {
        let mut assets = world.resource_mut::<Assets<TextureAtlasLayout>>();
        let mut layouts = HashMap::new();
        for kind in MODERN_ANIMATIONS {
            let (columns, rows) = sheet_grid(kind);
            layouts.insert(
                kind,
                assets.add(TextureAtlasLayout::from_grid(
                    CELL_SIZE, columns, rows, None, None,
                )),
            );
        }
        layouts.insert(
            JobAnimationKind::Legacy,
            assets.add(TextureAtlasLayout::from_grid(CELL_SIZE, 8, 8, None, None)),
        );
        Self { layouts }
    }
}

impl JobAnimationLayouts {
    fn get(&self, kind: JobAnimationKind) -> Handle<TextureAtlasLayout> {
        self.layouts
            .get(&kind)
            .expect("every job animation must have an atlas layout")
            .clone()
    }
}

#[derive(Component)]
struct PendingJobVisual(CharacterClass);

#[derive(Component)]
struct JobVisualClass(CharacterClass);

#[derive(Component)]
struct JobVisualSource(JobSpriteSource);

#[derive(Component)]
struct JobVisualRoot;

#[derive(Component)]
struct JobSpriteVisual {
    owner: Entity,
    kind: JobAnimationKind,
}

#[derive(Component)]
struct JobAnimationPlayback {
    class: CharacterClass,
    kind: JobAnimationKind,
    frame: usize,
    elapsed: f32,
    logged_sitting_state: Option<(u8, u8, usize, bool)>,
}

impl JobAnimationPlayback {
    fn new(class: CharacterClass) -> Self {
        Self {
            class,
            kind: JobAnimationKind::Idle,
            frame: 0,
            elapsed: 0.0,
            logged_sitting_state: None,
        }
    }

    fn reset(&mut self, class: CharacterClass, kind: JobAnimationKind) {
        self.class = class;
        self.kind = kind;
        self.frame = 0;
        self.elapsed = 0.0;
        self.logged_sitting_state = None;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobOneShotKind {
    Hit,
    Pickup,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct JobAnimationOverride(JobOneShotKind);

impl JobAnimationOverride {
    pub(crate) fn pickup() -> Self {
        Self(JobOneShotKind::Pickup)
    }

    fn hit() -> Self {
        Self(JobOneShotKind::Hit)
    }

    fn animation(self) -> JobAnimationKind {
        match self.0 {
            JobOneShotKind::Hit => JobAnimationKind::Hit,
            JobOneShotKind::Pickup => JobAnimationKind::Pickup,
        }
    }
}

pub(crate) struct JobAnimationsPlugin;

impl Plugin for JobAnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JobSpriteAssetCache>()
            .init_resource::<JobAnimationLayouts>()
            .add_systems(
                Update,
                (
                    prepare_job_visuals,
                    finish_job_visuals,
                    hide_replaced_legacy_visuals,
                )
                    .chain()
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(
                PostUpdate,
                animate_job_sprites
                    .run_if(in_state(ClientState::InGame))
                    .after(PanOrbitCameraSystemSet)
                    .before(TransformSystems::Propagate),
            )
            .add_observer(play_hit_animation);
    }
}

fn prepare_job_visuals(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: ResMut<JobSpriteAssetCache>,
    players: Query<
        (
            Entity,
            &JobProgression,
            Option<&JobVisualClass>,
            Option<&PendingJobVisual>,
        ),
        (
            With<Player>,
            With<JobAnimatedPlayer>,
            Or<(Added<JobAnimatedPlayer>, Changed<JobProgression>)>,
        ),
    >,
) {
    for (entity, progression, current, pending) in &players {
        if current.is_some_and(|current| current.0 == progression.class)
            || pending.is_some_and(|pending| pending.0 == progression.class)
        {
            continue;
        }
        cache.package(progression.class, &asset_server);
        commands
            .entity(entity)
            .insert(PendingJobVisual(progression.class));
    }
}

fn finish_job_visuals(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cache: Res<JobSpriteAssetCache>,
    layouts: Res<JobAnimationLayouts>,
    players: Query<(Entity, &PendingJobVisual), (With<Player>, With<JobAnimatedPlayer>)>,
    roots: Query<(Entity, &ChildOf), With<JobVisualRoot>>,
    mut legacy_visuals: Query<
        (&ChildOf, &mut Visibility),
        Or<(With<WalkingSpriteVisual>, With<AttackSpriteVisual>)>,
    >,
) {
    for (player_entity, pending) in &players {
        let Some(package) = cache.packages.get(&pending.0) else {
            continue;
        };
        if let Some(error) = package.failed_path(&asset_server) {
            error!("Could not load sprites for {}: {error}", pending.0.name());
            commands.entity(player_entity).remove::<PendingJobVisual>();
            continue;
        }
        if !package.is_loaded(&asset_server) {
            continue;
        }

        for (root_entity, parent) in &roots {
            if parent.parent() == player_entity {
                commands.entity(root_entity).despawn();
            }
        }

        commands
            .entity(player_entity)
            .with_children(|player_children| {
                player_children
                    .spawn((
                        Transform::default(),
                        Visibility::Inherited,
                        JobVisualRoot,
                        Name::new(format!("{} job visuals", pending.0.name())),
                    ))
                    .with_children(|visuals| {
                        for (kind, image) in &package.images {
                            visuals.spawn((
                                Transform::from_xyz(0.0, JOB_SPRITE_LOCAL_Y, 0.0),
                                Sprite3d {
                                    pixels_per_metre: JOB_SPRITE_PIXELS_PER_METRE,
                                    alpha_mode: AlphaMode::Blend,
                                    unlit: true,
                                    pivot: Some(Vec2::new(0.5, 0.0)),
                                    ..default()
                                },
                                Sprite {
                                    image: image.clone(),
                                    texture_atlas: Some(TextureAtlas {
                                        layout: layouts.get(*kind),
                                        index: 0,
                                    }),
                                    ..default()
                                },
                                if *kind == JobAnimationKind::Idle
                                    || *kind == JobAnimationKind::Legacy
                                {
                                    Visibility::Inherited
                                } else {
                                    Visibility::Hidden
                                },
                                Billboard,
                                JobSpriteVisual {
                                    owner: player_entity,
                                    kind: *kind,
                                },
                                NotShadowCaster,
                                Name::new(format!("{} {:?}", pending.0.name(), kind)),
                            ));
                        }
                    });
            });

        for (parent, mut visibility) in &mut legacy_visuals {
            if parent.parent() == player_entity {
                *visibility = Visibility::Hidden;
            }
        }

        commands
            .entity(player_entity)
            .insert((
                JobVisualClass(pending.0),
                JobVisualSource(package.source),
                JobAnimationPlayback::new(pending.0),
            ))
            .remove::<PendingJobVisual>();
        info!("Loaded {} job sprite animations", pending.0.name());
    }
}

fn hide_replaced_legacy_visuals(
    job_players: Query<(), (With<Player>, With<JobVisualClass>)>,
    mut legacy_visuals: Query<
        (&ChildOf, &mut Visibility),
        Or<(With<WalkingSpriteVisual>, With<AttackSpriteVisual>)>,
    >,
) {
    for (parent, mut visibility) in &mut legacy_visuals {
        if job_players.contains(parent.parent()) {
            *visibility = Visibility::Hidden;
        }
    }
}

#[allow(clippy::type_complexity)]
fn animate_job_sprites(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<&Transform, (With<Camera3d>, Without<Billboard>)>,
    transforms: Query<&Transform>,
    mut players: Query<
        (
            Entity,
            &JobProgression,
            &JobVisualSource,
            &mut JobAnimationPlayback,
            &mut Animation,
            &GameVelocity,
            &Facing,
            Option<&mut LastAnimationDirection>,
            Option<&Dead>,
            Option<&Sitting>,
            Option<&Equipment>,
            Option<Ref<JobAnimationOverride>>,
        ),
        (With<Player>, With<JobAnimatedPlayer>),
    >,
    mut visuals: Query<(&JobSpriteVisual, &mut Sprite, &mut Visibility)>,
) {
    let Ok(camera_transform) = camera.single() else {
        return;
    };

    for (
        entity,
        progression,
        source,
        mut playback,
        mut animation,
        velocity,
        facing,
        mut last_direction,
        dead,
        sitting,
        equipment,
        one_shot,
    ) in &mut players
    {
        let desired_kind = desired_animation(
            source.0,
            &animation,
            velocity.0,
            dead.is_some(),
            sitting.is_some(),
            equipment,
            one_shot.as_deref(),
        );
        let override_restarted = one_shot
            .as_ref()
            .is_some_and(|one_shot| one_shot.is_changed());
        let finish_current_action = should_finish_current_action(playback.kind, desired_kind);
        if playback.class != progression.class
            || playback.kind != desired_kind && !finish_current_action
            || override_restarted
        {
            playback.reset(progression.class, desired_kind);
        }

        let previous_direction = last_direction.as_deref().map(|direction| direction.0);
        let world_direction = match &*animation {
            Animation::Attacking { enemy, .. } => transforms
                .get(entity)
                .ok()
                .zip(transforms.get(*enemy).ok())
                .map(|(attacker, target)| {
                    let difference = target.translation - attacker.translation;
                    Vec3::new(difference.x, 0.0, difference.z).normalize_or_zero()
                })
                .filter(|direction| *direction != Vec3::ZERO)
                .or(previous_direction)
                .unwrap_or_else(|| {
                    animation_world_direction(&animation, Vec3::ZERO, None, facing.0)
                }),
            _ => animation_world_direction(&animation, velocity.0, previous_direction, facing.0),
        };
        if world_direction != Vec3::ZERO {
            if let Some(last_direction) = last_direction.as_deref_mut() {
                last_direction.0 = world_direction;
            } else {
                commands
                    .entity(entity)
                    .insert(LastAnimationDirection(world_direction));
            }
        }

        let visible_direction = atlas_direction(camera_transform, world_direction);
        if playback.kind == JobAnimationKind::Sit {
            let (sitting_index, flip_x) =
                sprite_frame(source.0, JobAnimationKind::Sit, visible_direction, 0);
            let rendered_index = sprite3d_atlas_index(sitting_index, JobAnimationKind::Sit, flip_x);
            let sitting_state = (facing.0, visible_direction, sitting_index, flip_x);
            if playback.logged_sitting_state != Some(sitting_state) {
                info!(
                    "Sitting sprite selected: entity={entity:?}, facing={}, camera_direction={} ({}), source_index={}, rendered_atlas_index={}, flip_x={}, world_direction=({:.3}, {:.3}, {:.3})",
                    facing.0,
                    visible_direction,
                    direction_clock(visible_direction),
                    sitting_index,
                    rendered_index,
                    flip_x,
                    world_direction.x,
                    world_direction.y,
                    world_direction.z,
                );
                playback.logged_sitting_state = Some(sitting_state);
            }
        }
        let attack_period = match &*animation {
            Animation::Attacking { attack_speed, .. } => Some(*attack_speed),
            _ => None,
        };
        let repeat_action = match playback.kind {
            JobAnimationKind::Attack1 | JobAnimationKind::Attack2 => {
                desired_kind == playback.kind
                    && matches!(
                        &*animation,
                        Animation::Attacking {
                            auto_attack: true,
                            ..
                        }
                    )
            }
            _ => false,
        };
        let frame_count = animation_frame_count(playback.kind, visible_direction);
        let hold_cast_frame =
            playback.kind == JobAnimationKind::Cast && desired_kind == JobAnimationKind::Cast;
        let completion = advance_playback(
            &mut playback,
            time.delta_secs(),
            frame_count,
            attack_period,
            repeat_action,
            hold_cast_frame,
        );

        for (visual, mut sprite, mut visibility) in &mut visuals {
            if visual.owner != entity {
                continue;
            }
            let visible_kind = if source.0 == JobSpriteSource::Legacy {
                JobAnimationKind::Legacy
            } else {
                playback.kind
            };
            if visual.kind != visible_kind {
                *visibility = Visibility::Hidden;
                continue;
            }

            *visibility = Visibility::Inherited;
            let (index, flip_x) =
                sprite_frame(source.0, playback.kind, visible_direction, playback.frame);
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = sprite3d_atlas_index(index, playback.kind, flip_x);
            }
            sprite.flip_x = flip_x;
        }

        if completion {
            match playback.kind {
                JobAnimationKind::Hit | JobAnimationKind::Pickup => {
                    commands.entity(entity).remove::<JobAnimationOverride>();
                }
                JobAnimationKind::Attack1 | JobAnimationKind::Attack2 => {
                    if matches!(
                        &*animation,
                        Animation::Attacking {
                            auto_attack: false,
                            ..
                        }
                    ) {
                        *animation = Animation::Idle;
                        playback.reset(progression.class, JobAnimationKind::Idle);
                    }
                }
                _ => {}
            }
            if playback.kind != desired_kind
                && matches!(
                    playback.kind,
                    JobAnimationKind::Attack1 | JobAnimationKind::Attack2 | JobAnimationKind::Cast
                )
            {
                playback.reset(progression.class, desired_kind);
            }
        }
    }
}

fn play_hit_animation(
    trigger: On<DamageNumberEvent>,
    mut commands: Commands,
    players: Query<(), (With<Player>, With<JobAnimatedPlayer>, Without<Dead>)>,
) {
    let damage = trigger.event();
    if damage.amount > 0 && players.contains(damage.entity) {
        commands
            .entity(damage.entity)
            .insert(JobAnimationOverride::hit());
    }
}

fn desired_animation(
    _source: JobSpriteSource,
    animation: &Animation,
    velocity: Vec3,
    dead: bool,
    sitting: bool,
    equipment: Option<&Equipment>,
    one_shot: Option<&JobAnimationOverride>,
) -> JobAnimationKind {
    if dead {
        return JobAnimationKind::Death;
    }
    if let Some(one_shot) = one_shot {
        return one_shot.animation();
    }
    if sitting || matches!(animation, Animation::Sitting) {
        return JobAnimationKind::Sit;
    }
    match animation {
        Animation::Attacking { .. } => {
            if equipment.is_some_and(|equipment| {
                equipment.item(EquipmentSlot::MainHand) == Some(APPRENTICE_STAFF)
            }) {
                JobAnimationKind::Attack2
            } else {
                JobAnimationKind::Attack1
            }
        }
        Animation::Casting => JobAnimationKind::Cast,
        _ if velocity.x * velocity.x + velocity.z * velocity.z > f32::EPSILON => {
            JobAnimationKind::Walk
        }
        _ => JobAnimationKind::Idle,
    }
}

fn should_finish_current_action(current: JobAnimationKind, desired: JobAnimationKind) -> bool {
    current != desired
        && matches!(
            current,
            JobAnimationKind::Attack1 | JobAnimationKind::Attack2 | JobAnimationKind::Cast
        )
        && matches!(desired, JobAnimationKind::Idle | JobAnimationKind::Walk)
}

fn advance_playback(
    playback: &mut JobAnimationPlayback,
    delta_seconds: f32,
    frame_count: usize,
    attack_period: Option<f32>,
    repeat_action: bool,
    hold_cast_frame: bool,
) -> bool {
    if hold_cast_frame {
        playback.frame = 0;
        playback.elapsed = 0.0;
        return false;
    }
    if frame_count <= 1 {
        return false;
    }
    playback.frame %= frame_count;
    let frame_duration = match playback.kind {
        JobAnimationKind::Attack1 | JobAnimationKind::Attack2 => {
            attack_period.unwrap_or(0.7).max(0.001) / frame_count as f32
        }
        JobAnimationKind::Walk | JobAnimationKind::Legacy => 0.1,
        JobAnimationKind::Idle => 0.18,
        JobAnimationKind::Death => 0.15,
        JobAnimationKind::Pickup => 0.1,
        JobAnimationKind::Cast => 0.12,
        JobAnimationKind::Hit => 0.035,
        JobAnimationKind::Sit => return false,
    };

    playback.elapsed += delta_seconds.max(0.0);
    let advances = (playback.elapsed / frame_duration).floor() as usize;
    if advances == 0 {
        return false;
    }
    playback.elapsed -= advances as f32 * frame_duration;

    let repeat = matches!(
        playback.kind,
        JobAnimationKind::Walk | JobAnimationKind::Idle | JobAnimationKind::Legacy
    ) || matches!(
        playback.kind,
        JobAnimationKind::Attack1 | JobAnimationKind::Attack2 | JobAnimationKind::Cast
    ) && repeat_action;
    let next = playback.frame.saturating_add(advances);
    if repeat {
        playback.frame = next % frame_count;
        return false;
    }
    if next >= frame_count {
        playback.frame = frame_count - 1;
        true
    } else {
        playback.frame = next;
        false
    }
}

fn sprite_frame(
    source: JobSpriteSource,
    kind: JobAnimationKind,
    direction: u8,
    frame: usize,
) -> (usize, bool) {
    if source == JobSpriteSource::Legacy {
        let frame = if kind == JobAnimationKind::Sit {
            7
        } else if kind == JobAnimationKind::Walk {
            frame % 7
        } else {
            0
        };
        return (direction as usize * 8 + frame, false);
    }

    let flip_x = matches!(direction % 8, 5 | 6 | 7);
    match kind {
        JobAnimationKind::Walk => {
            let row = usize::from(direction % 8);
            (row * 8 + frame % 8, false)
        }
        JobAnimationKind::Idle => {
            let row = idle_row(direction);
            let frame_count = animation_frame_count(JobAnimationKind::Idle, direction);
            (row * 6 + frame % frame_count, flip_x)
        }
        JobAnimationKind::Sit => sitting_pose(direction),
        JobAnimationKind::Death => directional_two_row_frame(direction, frame, 4, flip_x),
        JobAnimationKind::Pickup => directional_two_row_frame(direction, frame, 3, flip_x),
        JobAnimationKind::Cast => directional_two_row_frame(direction, frame, 4, flip_x),
        JobAnimationKind::Hit => (frame % 6, flip_x),
        JobAnimationKind::Attack1 => directional_two_row_frame(direction, frame, 7, flip_x),
        JobAnimationKind::Attack2 => directional_two_row_frame(direction, frame, 6, flip_x),
        JobAnimationKind::Legacy => unreachable!("legacy sprites use the legacy source"),
    }
}

fn sitting_pose(direction: u8) -> (usize, bool) {
    // The five painted cells turn from front toward screen-left:
    // 6:00, 7:30, 9:00, 10:30, and 12:00. Mirror those cells for the
    // screen-right half so a camera orbit produces one continuous full turn.
    match direction % 8 {
        0 => (4, false), // 12:00
        1 => (3, true),  // 1:30
        2 => (2, true),  // 3:00
        3 => (1, true),  // 4:30
        4 => (0, false), // 6:00
        5 => (1, false), // 7:30
        6 => (2, false), // 9:00
        7 => (3, false), // 10:30
        _ => unreachable!(),
    }
}

fn direction_clock(direction: u8) -> &'static str {
    match direction % 8 {
        0 => "12:00",
        1 => "1:30",
        2 => "3:00",
        3 => "4:30",
        4 => "6:00",
        5 => "7:30",
        6 => "9:00",
        7 => "10:30",
        _ => unreachable!(),
    }
}

/// `bevy_sprite3d` flips UVs across the complete image. For an atlas, that
/// would make column N sample the opposite column instead of mirroring N in
/// place. Select that opposite mesh column first so the material-level flip
/// lands back on the requested source cell.
fn sprite3d_atlas_index(source_index: usize, kind: JobAnimationKind, flip_x: bool) -> usize {
    if !flip_x {
        return source_index;
    }

    let columns = sheet_grid(kind).0 as usize;
    let row = source_index / columns;
    let column = source_index % columns;
    row * columns + (columns - 1 - column)
}

fn directional_two_row_frame(
    direction: u8,
    frame: usize,
    columns: usize,
    flip_x: bool,
) -> (usize, bool) {
    let back_facing = matches!(direction % 8, 0 | 1 | 7);
    (usize::from(back_facing) * columns + frame % columns, flip_x)
}

fn idle_row(direction: u8) -> usize {
    match direction % 8 {
        3..=5 => 0,
        2 | 6 => 1,
        _ => 2,
    }
}

fn animation_frame_count(kind: JobAnimationKind, _direction: u8) -> usize {
    match kind {
        JobAnimationKind::Walk | JobAnimationKind::Legacy => 8,
        // Keep the directional idle artwork as a static standing pose for now.
        JobAnimationKind::Idle => 1,
        JobAnimationKind::Sit => 1,
        JobAnimationKind::Death | JobAnimationKind::Cast => 4,
        JobAnimationKind::Pickup => 3,
        JobAnimationKind::Hit | JobAnimationKind::Attack2 => 6,
        JobAnimationKind::Attack1 => 7,
    }
}

fn sheet_grid(kind: JobAnimationKind) -> (u32, u32) {
    match kind {
        JobAnimationKind::Walk | JobAnimationKind::Legacy => (8, 8),
        JobAnimationKind::Idle => (6, 3),
        JobAnimationKind::Sit => (5, 1),
        JobAnimationKind::Death | JobAnimationKind::Cast => (4, 2),
        JobAnimationKind::Pickup => (3, 2),
        JobAnimationKind::Hit => (6, 1),
        JobAnimationKind::Attack1 => (7, 2),
        JobAnimationKind::Attack2 => (6, 2),
    }
}

fn load_job_package(class: CharacterClass, asset_server: &AssetServer) -> JobSpritePackage {
    if let Some(slug) = modern_job_slug(class) {
        return JobSpritePackage {
            source: JobSpriteSource::Modern,
            images: MODERN_ANIMATIONS
                .into_iter()
                .map(|kind| (kind, asset_server.load(modern_job_path(slug, "male", kind))))
                .collect(),
        };
    }

    let path = legacy_job_path(class)
        .expect("every placeholder class must have a modern or legacy job sprite");
    JobSpritePackage {
        source: JobSpriteSource::Legacy,
        images: vec![(JobAnimationKind::Legacy, asset_server.load(path))],
    }
}

fn modern_job_path(slug: &str, gender: &str, kind: JobAnimationKind) -> String {
    let animation = match kind {
        JobAnimationKind::Walk => "walk",
        JobAnimationKind::Idle => "idle",
        JobAnimationKind::Sit => "sit",
        JobAnimationKind::Death => "death",
        JobAnimationKind::Pickup => "pickup",
        JobAnimationKind::Cast => "cast",
        JobAnimationKind::Hit => "hit",
        JobAnimationKind::Attack1 => "attack1",
        JobAnimationKind::Attack2 => "attack2",
        JobAnimationKind::Legacy => unreachable!("modern jobs do not use legacy sheets"),
    };
    format!("spritesheets/jobs/{slug}-{gender}-{animation}.png")
}

fn modern_job_slug(class: CharacterClass) -> Option<&'static str> {
    Some(match class {
        CharacterClass::Novice => "chazki",
        CharacterClass::Swordsman => "quipucamayoc",
        CharacterClass::Mage => "amauta",
        CharacterClass::Archer => "haravicu",
        CharacterClass::Acolyte => "curaca",
        CharacterClass::Awqaq => "awqaq",
        CharacterClass::Conquistador => "conquistador",
        CharacterClass::Encomendero => "encomendero",
        CharacterClass::Corregidor => "corregidor",
        CharacterClass::Virrey => "virrey",
        CharacterClass::Oidor => "oidor",
        CharacterClass::Escribano => "escribano",
        CharacterClass::Alguacil => "alguacil",
        CharacterClass::Visitador => "visitador",
        CharacterClass::Doctrinero => "doctrinero",
        CharacterClass::Fraile => "fraile",
        CharacterClass::Hacendado => "hacendado",
        CharacterClass::Estanciero => "estanciero",
        CharacterClass::Minero => "minero",
        CharacterClass::Azoguero => "azoguero",
        CharacterClass::Arriero => "arriero",
        CharacterClass::Mercader => "mercader",
        CharacterClass::Pulpero => "pulpero",
        CharacterClass::Artesano => "artesano",
        CharacterClass::MaestroDeOficio => "maestro_de_oficio",
        CharacterClass::SoldadoDePresidio => "soldado_de_presidio",
        CharacterClass::Marinero => "marinero",
        CharacterClass::Mayordomo => "mayordomo",
        CharacterClass::Capataz => "capataz",
        CharacterClass::Merchant
        | CharacterClass::Thief
        | CharacterClass::QollqaKamayuq
        | CharacterClass::ChacraKamayuq
        | CharacterClass::LlamaMichiq
        | CharacterClass::Mitmaq
        | CharacterClass::Yana
        | CharacterClass::RunaSimiKamayuq => return None,
    })
}

fn legacy_job_path(class: CharacterClass) -> Option<&'static str> {
    Some(match class {
        CharacterClass::Merchant => "spritesheets/willac_umu/willac_umu_headless.png",
        CharacterClass::Thief => "spritesheets/aclla/aclla_headless.png",
        CharacterClass::QollqaKamayuq => "spritesheets/qollqa_kamayuq/qollqa_kamayuq_headless.png",
        CharacterClass::ChacraKamayuq => "spritesheets/chacra_kamayuq/chacra_kamayuq_headless.png",
        CharacterClass::LlamaMichiq => "spritesheets/llama_michiq/llama_michiq_headless.png",
        CharacterClass::Mitmaq => "spritesheets/mitmaq/mitmaq_headless.png",
        CharacterClass::Yana => "spritesheets/yana/yana_headless.png",
        CharacterClass::RunaSimiKamayuq => {
            "spritesheets/runa_simi_kamayuq/runa_simi_kamayuq_headless.png"
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn every_placeholder_class_has_a_job_sprite_source() {
        for class in CharacterClass::PLACEHOLDERS {
            assert!(
                modern_job_slug(class).is_some() || legacy_job_path(class).is_some(),
                "{} has no sprite mapping",
                class.name()
            );
        }
        assert_eq!(
            CharacterClass::PLACEHOLDERS
                .into_iter()
                .filter(|class| modern_job_slug(*class).is_some())
                .count(),
            29
        );
    }

    #[test]
    fn modern_paths_match_the_flat_jobs_manifest_convention() {
        assert_eq!(
            modern_job_path("chazki", "male", JobAnimationKind::Idle),
            "spritesheets/jobs/chazki-male-idle.png"
        );
        assert_eq!(
            modern_job_path("virrey", "female", JobAnimationKind::Attack2),
            "spritesheets/jobs/virrey-female-attack2.png"
        );
    }

    #[test]
    fn every_runtime_job_sheet_exists() {
        let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for class in CharacterClass::PLACEHOLDERS {
            if let Some(slug) = modern_job_slug(class) {
                for kind in MODERN_ANIMATIONS {
                    let path = asset_root.join(modern_job_path(slug, "male", kind));
                    assert!(path.is_file(), "missing {}", path.display());
                }
            } else {
                let path = asset_root.join(legacy_job_path(class).unwrap());
                assert!(path.is_file(), "missing {}", path.display());
            }
        }
    }

    #[test]
    fn action_grids_match_the_supplied_128_pixel_cell_sheets() {
        assert_eq!(sheet_grid(JobAnimationKind::Walk), (8, 8));
        assert_eq!(sheet_grid(JobAnimationKind::Idle), (6, 3));
        assert_eq!(sheet_grid(JobAnimationKind::Sit), (5, 1));
        assert_eq!(sheet_grid(JobAnimationKind::Attack1), (7, 2));
        assert_eq!(sheet_grid(JobAnimationKind::Attack2), (6, 2));
    }

    #[test]
    fn modern_walk_rows_follow_the_camera_relative_direction() {
        assert_eq!(
            sprite_frame(JobSpriteSource::Modern, JobAnimationKind::Walk, 4, 0),
            (32, false)
        );
        assert_eq!(
            sprite_frame(JobSpriteSource::Modern, JobAnimationKind::Walk, 0, 0),
            (0, false)
        );
    }

    #[test]
    fn sitting_maps_every_camera_relative_clock_direction_explicitly() {
        let expected = [
            (4, false),
            (3, true),
            (2, true),
            (1, true),
            (0, false),
            (1, false),
            (2, false),
            (3, false),
        ];

        for (direction, expected_frame) in expected.into_iter().enumerate() {
            assert_eq!(
                sprite_frame(
                    JobSpriteSource::Modern,
                    JobAnimationKind::Sit,
                    direction as u8,
                    0
                ),
                expected_frame
            );
        }
    }

    #[test]
    fn sprite3d_flip_keeps_the_requested_sitting_cell() {
        assert_eq!(sprite3d_atlas_index(3, JobAnimationKind::Sit, true), 1);
        assert_eq!(sprite3d_atlas_index(2, JobAnimationKind::Sit, true), 2);
        assert_eq!(sprite3d_atlas_index(3, JobAnimationKind::Sit, false), 3);
    }

    #[test]
    fn idle_is_a_static_directional_standing_pose() {
        for direction in 0..8 {
            let (index, _) = sprite_frame(
                JobSpriteSource::Modern,
                JobAnimationKind::Idle,
                direction,
                5,
            );
            assert_eq!(index % 6, 0);
            assert_eq!(animation_frame_count(JobAnimationKind::Idle, direction), 1);
        }
    }

    #[test]
    fn attack_and_cast_finish_before_returning_to_neutral_animation() {
        assert!(should_finish_current_action(
            JobAnimationKind::Attack1,
            JobAnimationKind::Idle
        ));
        assert!(should_finish_current_action(
            JobAnimationKind::Cast,
            JobAnimationKind::Idle
        ));
        assert!(!should_finish_current_action(
            JobAnimationKind::Attack1,
            JobAnimationKind::Death
        ));
    }

    #[test]
    fn one_shot_animation_holds_its_last_frame_and_completes() {
        let mut playback = JobAnimationPlayback::new(CharacterClass::Novice);
        playback.reset(CharacterClass::Novice, JobAnimationKind::Pickup);

        assert!(advance_playback(&mut playback, 0.3, 3, None, false, false));
        assert_eq!(playback.frame, 2);
    }

    #[test]
    fn cast_holds_frame_zero_then_plays_release_frames_once() {
        let mut playback = JobAnimationPlayback::new(CharacterClass::Novice);
        playback.reset(CharacterClass::Novice, JobAnimationKind::Cast);

        assert!(!advance_playback(&mut playback, 10.0, 4, None, false, true));
        assert_eq!(playback.frame, 0);

        for expected_frame in 1..=3 {
            assert!(!advance_playback(
                &mut playback,
                0.12,
                4,
                None,
                false,
                false
            ));
            assert_eq!(playback.frame, expected_frame);
        }
        assert!(advance_playback(&mut playback, 0.12, 4, None, false, false));
        assert_eq!(playback.frame, 3);
    }

    #[test]
    fn animation_system_has_valid_bevy_parameters() {
        let mut world = World::new();
        world.init_resource::<Time>();

        let mut schedule = Schedule::default();
        schedule.add_systems(animate_job_sprites);
        schedule.run(&mut world);
    }
}
