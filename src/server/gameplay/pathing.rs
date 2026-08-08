use crate::shared::constants::*;
use crate::shared::gameplay::maps::MAP_DEFINITIONS;
use crate::shared::gameplay::{components::*, entities::MapEntity};
use crate::world::setup_server_level;
use bevy::{camera::primitives::MeshAabb, prelude::*};
use bevy_rapier3d::prelude::*;
use pathfinding::prelude::{astar, bfs};
use std::{collections::HashSet, time::Duration};

pub struct PathingPlugin;

#[derive(Component, Debug)]
pub struct TargetPos {
    pub position: Vec3,
}

pub const DAMAGE_WALK_DELAY_DURATION: Duration = Duration::from_millis(200);
pub const DAMAGE_WALK_DELAY_IMMUNITY_DURATION: Duration = Duration::from_millis(400);

#[derive(Component, Debug)]
pub struct DamageWalkDelay {
    timer: Timer,
    pending_destination: Option<Vec3>,
}

impl DamageWalkDelay {
    pub fn new(pending_destination: Option<Vec3>) -> Self {
        Self {
            timer: Timer::new(DAMAGE_WALK_DELAY_DURATION, TimerMode::Once),
            pending_destination,
        }
    }

    pub fn queue_destination(&mut self, destination: Vec3) {
        self.pending_destination = Some(destination);
    }

    pub fn cancel_pending_destination(&mut self) {
        self.pending_destination = None;
    }
}

#[derive(Component, Debug)]
pub struct DamageWalkDelayImmunity(Timer);

impl Default for DamageWalkDelayImmunity {
    fn default() -> Self {
        Self(Timer::new(
            DAMAGE_WALK_DELAY_IMMUNITY_DURATION,
            TimerMode::Once,
        ))
    }
}

const WATER_NAV_COLUMNS_PER_TICK: i32 = 8;
const WATER_HEIGHT_RAY_ORIGIN: f32 = 128.0;
const WATER_HEIGHT_RAY_DISTANCE: f32 = 256.0;
const WAYPOINT_REACHED_DISTANCE: f32 = 0.05;

#[derive(Resource)]
struct WaterNavigationBuilder {
    map_index: usize,
    next_x: i32,
    started: bool,
    finished: bool,
}

impl Default for WaterNavigationBuilder {
    fn default() -> Self {
        Self {
            map_index: 0,
            next_x: MAP_DEFINITIONS[0].navigation_min[0],
            started: false,
            finished: false,
        }
    }
}

fn stop_walking_system(
    mut removals: RemovedComponents<Walking>,
    mut movers: Query<(
        Option<&mut GameVelocity>,
        Option<&mut KinematicCharacterController>,
    )>,
    mut commands: Commands,
) {
    for entity in removals.read() {
        // `Walking` is also reported as removed when an entity is despawned.
        // In that case the entity no longer exists by the time this system runs.
        if let Ok((velocity, controller)) = movers.get_mut(entity) {
            if let Some(mut velocity) = velocity {
                velocity.0 = Vec3::ZERO;
            }
            if let Some(mut controller) = controller {
                controller.translation = None;
            }
            trace!("Entity {:?} stopped walking", entity);
        }
        commands.entity(entity).try_remove::<TargetPos>();
    }
}

fn tick_damage_walk_delays(
    time: Res<Time>,
    map: Res<Map>,
    mut delayed: Query<(Entity, &Transform, &mut DamageWalkDelay, Option<&Dead>)>,
    mut immunities: Query<(Entity, &mut DamageWalkDelayImmunity)>,
    mut commands: Commands,
) {
    for (entity, transform, mut delay, dead) in &mut delayed {
        if !delay.timer.tick(time.delta()).just_finished() {
            continue;
        }

        let pending_destination = delay.pending_destination.take();
        commands.entity(entity).try_remove::<DamageWalkDelay>();
        if dead.is_some() {
            continue;
        }

        let Some(destination) = pending_destination else {
            continue;
        };
        if let Some(path) = get_path_between_translations(transform.translation, destination, &map)
        {
            commands.entity(entity).try_insert(Walking {
                target_translation: destination,
                path: Some(path),
            });
        }
    }

    for (entity, mut immunity) in &mut immunities {
        if immunity.0.tick(time.delta()).just_finished() {
            commands
                .entity(entity)
                .try_remove::<DamageWalkDelayImmunity>();
        }
    }
}

impl Plugin for PathingPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.init_resource::<WaterNavigationBuilder>()
            .add_systems(
                Startup,
                (
                    setup_prohibited_areas.after(setup_server_level),
                    setup_prohibited_cells.after(setup_server_level),
                    setup_gravity.after(setup_server_level),
                ),
            )
            .add_systems(
                FixedUpdate,
                build_water_navigation.after(PhysicsSet::Writeback),
            )
            /*.add_systems(Update, (
                    get_avian3d_velocity.after(PhysicsSet::Sync),
                    apply_avian3d_velocity_system.after(get_velocity).after(PhysicsSet::Sync),
                )
            )*/
            /* .add_systems(
            FixedUpdate, (
                    get_velocity,
                    apply_velocity_system.after(get_velocity),
                    //.after(PhysicsSet::Writeback)
                    //.after(TransformSystem::TransformPropagate),
                    //read_result_system.after(apply_rapier3d_velocity_system),
                    walking_system
                    //client_velocity.run_if(in_state(AppState::InGame)),
                )
            );*/
            .add_systems(
                FixedUpdate,
                (
                    tick_damage_walk_delays,
                    walking_system,
                    stop_walking_system,
                    get_velocity,
                    apply_rapier3d_velocity_system,
                )
                    .chain()
                    .before(PhysicsSet::SyncBackend),
            );

        fn setup_gravity(mut rapier_config: Query<&mut RapierConfiguration>) {
            if let Ok(mut rapier_config) = rapier_config.single_mut() {
                rapier_config.gravity = Vec3::new(0.0, -9.81, 0.0);
            }

            /*rapier_config.timestep_mode = TimestepMode::Fixed {
                dt: 1.0 / 240.0,
                substeps: 1
            } */
            /*rapier_config.timestep_mode = TimestepMode::Interpolated {
                dt: 1./240., time_scale: 1.0, substeps: 2
            } */
            /*rapier_config.timestep_mode = TimestepMode::Fixed {
                dt: Duration::from_micros(15625).as_secs_f32(),
                substeps: 1,
            }*/
        }

        /*
        Esto podria pre-hacerse y simplemente dejar el array listo en el mapa. Para ahorrar calcularlo cada vez.. total. El mapa nunca cambia.
        */
        pub fn setup_prohibited_cells(
            //map_entities: Query<(&MapEntity, &Handle<Mesh>, &Collider, &Transform)>,
            map_entities: Query<(&MapEntity, &Mesh3d, &Collider, &Transform)>,
            mut meshes: ResMut<Assets<Mesh>>,
            mut map: ResMut<Map>,
        ) {
            for (map_entity, mesh_handle, collider, transform) in map_entities.iter() {
                let mesh = meshes.get_mut(mesh_handle).unwrap();

                let aabb = mesh.compute_aabb();

                if let Some(aabb) = aabb {
                    let starting_point_x = (transform.translation.x - aabb.half_extents.x) + 0.5;
                    let ending_point_x = (transform.translation.x + aabb.half_extents.x) + 0.5;
                    let starting_point_z = (transform.translation.z - aabb.half_extents.z) + 0.5;
                    let ending_point_z = (transform.translation.z + aabb.half_extents.z) + 0.5;

                    for x in starting_point_x as i32..ending_point_x as i32 {
                        for z in starting_point_z as i32..ending_point_z as i32 {
                            let pos = Pos(x, z);
                            if !map.blocked_paths.contains(&pos) {
                                println!("Se agrega {:?} a blocked paths", pos);
                                map.blocked_paths.insert(pos);
                            }
                        }
                    }
                    println!(" blocked paths {:?} ", map.blocked_paths);
                }

                /*println!("mesh aabb  {:?}, ",aabb) ;

                let rotation = Rotation::default();

                let aabb = collider.aabb(transform.translation, *rotation);
                let range = aabb.min.x;
                println!("collideraabb  {:?}, ",aabb) ;*/
                //if(collider.hal)
            }
        }

        pub fn setup_prohibited_areas(
            mut map: ResMut<Map>,
            mut buildings: Query<(Entity, &mut Building)>,
        ) {
            for (_entity, mut building) in buildings.iter_mut() {
                info!("Building {:?}!", building.blocked_paths);
                map.blocked_paths.extend(building.blocked_paths.drain(..));
                info!("blocked_paths {:?}!", map);
            }
        }

        pub fn walking_system(
            mut walking_entities: Query<(
                Entity,
                &Transform,
                &mut Walking,
                Option<&DamageWalkDelay>,
            )>,
            mut commands: Commands,
            //map: Res<Map>
        ) {
            for (entity, transform, mut walking, damage_walk_delay) in walking_entities.iter_mut() {
                if damage_walk_delay.is_some() {
                    commands
                        .entity(entity)
                        .try_remove::<Walking>()
                        .try_remove::<TargetPos>();
                    continue;
                }

                /*info!("1. Ta parado en: {:?},  {:?}", Pos(
                    transform.translation.x.round() as i32,
                    transform.translation.z.round() as i32
                ),  transform.translation);
                info!("2. Ta lejos. Acercarse!, path: {:?}", walking.path);*/

                let Some((steps, next_waypoint)) = walking.path.as_mut() else {
                    continue;
                };
                if steps.len() <= 1 {
                    commands.entity(entity).try_remove::<Walking>();
                    continue;
                }

                // A* includes the current cell as path[0]. Begin with path[1],
                // then advance only after reaching a waypoint instead of when
                // rounding happens to enter its cell.
                if *next_waypoint == 0 {
                    *next_waypoint = 1;
                }
                let mut waypoint_index = *next_waypoint as usize;
                if let Some(waypoint) = steps.get(waypoint_index) {
                    let offset = Vec2::new(
                        transform.translation.x - waypoint.0 as f32,
                        transform.translation.z - waypoint.1 as f32,
                    );
                    if offset.length_squared()
                        <= WAYPOINT_REACHED_DISTANCE * WAYPOINT_REACHED_DISTANCE
                    {
                        waypoint_index += 1;
                        *next_waypoint = u32::try_from(waypoint_index).unwrap_or(u32::MAX);
                    }
                }

                let Some(next) = steps.get(waypoint_index) else {
                    commands.entity(entity).try_remove::<Walking>();
                    continue;
                };
                commands.entity(entity).try_insert(TargetPos {
                    position: Vec3::new(next.0 as f32, transform.translation.y, next.1 as f32),
                });
            }
        }

        /*
        pub fn get_avian3d_velocity(
            mut query: Query<(&mut Transform, &mut TargetPos, &mut LinearVelocity)>,
            time: Res<Time>

        ) {
            for (mut transform, target_pos,  mut linear_velocity) in &mut query {
                if(transform.translation.x  != target_pos.position.x || transform.translation.z  != target_pos.position.z ) {
                    info!("target_pos.position {:?}! transform {:?}! ", target_pos.position, transform.translation);
                    let velocity = calculate_velocity(transform.translation, target_pos.position);

                    linear_velocity.x = velocity.x;
                    linear_velocity.z = velocity.z;


                }

            }
        }*/
    }
}

fn build_water_navigation(
    read_rapier_context: ReadRapierContext,
    map_roots: Query<Entity, With<MapEntity>>,
    children: Query<&Children>,
    colliders: Query<(), With<Collider>>,
    mut map: ResMut<Map>,
    mut builder: ResMut<WaterNavigationBuilder>,
) {
    if builder.finished {
        return;
    }

    let Ok(rapier_context) = read_rapier_context.single() else {
        return;
    };

    if !builder.started {
        let terrain_colliders: HashSet<Entity> = map_roots
            .iter()
            .flat_map(|root| children.iter_descendants(root))
            .filter(|entity| colliders.contains(*entity))
            .collect();

        if terrain_colliders.is_empty() {
            return;
        }

        let terrain_is_in_physics_world = MAP_DEFINITIONS.iter().all(|definition| {
            let origin = Vec3::from_array(definition.server_origin);
            let quarter_x =
                (definition.navigation_max[0] - definition.navigation_min[0]) as f32 * 0.25;
            let quarter_z =
                (definition.navigation_max[1] - definition.navigation_min[1]) as f32 * 0.25;
            [
                Vec2::ZERO,
                Vec2::new(quarter_x, quarter_z),
                Vec2::new(-quarter_x, -quarter_z),
                Vec2::new(quarter_x, -quarter_z),
                Vec2::new(-quarter_x, quarter_z),
            ]
            .into_iter()
            .any(|sample| {
                rapier_context
                    .cast_ray(
                        Vec3::new(
                            origin.x + sample.x,
                            origin.y + WATER_HEIGHT_RAY_ORIGIN,
                            origin.z + sample.y,
                        ),
                        Vec3::NEG_Y,
                        WATER_HEIGHT_RAY_DISTANCE,
                        true,
                        QueryFilter::only_fixed().exclude_sensors(),
                    )
                    .is_some_and(|(entity, _)| terrain_colliders.contains(&entity))
            })
        });

        if !terrain_is_in_physics_world {
            return;
        }
        builder.started = true;
        info!(
            "Building navigation cells for {} maps",
            MAP_DEFINITIONS.len()
        );
    }

    let definition = &MAP_DEFINITIONS[builder.map_index];
    let origin = Vec3::from_array(definition.server_origin);
    let end_x = (builder.next_x + WATER_NAV_COLUMNS_PER_TICK - 1).min(definition.navigation_max[0]);
    for local_x in builder.next_x..=end_x {
        for local_z in definition.navigation_min[1]..=definition.navigation_max[1] {
            let x = origin.x.round() as i32 + local_x;
            let z = origin.z.round() as i32 + local_z;
            let surface_height = rapier_context
                .cast_ray(
                    Vec3::new(x as f32, origin.y + WATER_HEIGHT_RAY_ORIGIN, z as f32),
                    Vec3::NEG_Y,
                    WATER_HEIGHT_RAY_DISTANCE,
                    true,
                    QueryFilter::only_fixed().exclude_sensors(),
                )
                .map(|(_, time_of_impact)| origin.y + WATER_HEIGHT_RAY_ORIGIN - time_of_impact);

            let blocked = match (surface_height, definition.water_level) {
                (None, _) => true,
                (Some(height), Some(water_level)) => height < origin.y + water_level,
                (Some(_), None) => false,
            };
            if blocked {
                map.blocked_paths.insert(Pos(x, z));
            }
        }
    }

    builder.next_x = end_x + 1;
    if builder.next_x > definition.navigation_max[0] {
        info!("Finished navigation mask for '{}'", definition.name);
        builder.map_index += 1;
        if builder.map_index >= MAP_DEFINITIONS.len() {
            builder.finished = true;
            info!(
                "Finished navigation masks with {} total blocked cells",
                map.blocked_paths.len()
            );
        } else {
            builder.next_x = MAP_DEFINITIONS[builder.map_index].navigation_min[0];
        }
    }
}

fn surface_is_submerged(surface_height: Option<f32>) -> bool {
    surface_height.is_none_or(|height| height < WATER_LEVEL)
}

/*pub fn apply_avian3d_velocity_system(mut query: Query<(&mut LinearVelocity, &mut Transform, &TargetPos)>, time: Res<Time>) {
    for (mut linear_velocity, mut transform, target_pos) in query.iter_mut() {

        if(transform.translation.x != target_pos.position.x || transform.translation.z != target_pos.position.z) {
            info!("linear_velocity  {:?}!", linear_velocity);
            let diff = linear_velocity.0 * time.delta_secs();
            info!("diff  {:?}!", diff);

            if(target_pos.position.x > transform.translation.x &&  transform.translation.x + diff.x > target_pos.position.x) {
                transform.translation.x = target_pos.position.x;
                linear_velocity.x = 0.;
                info!("Se detiene x  {:?}!", linear_velocity);
            }
            else if target_pos.position.x < transform.translation.x &&  transform.translation.x + diff.x <= target_pos.position.x {
                transform.translation.x = target_pos.position.x;
                linear_velocity.x = 0.;
                info!("Se detiene x  {:?}!", linear_velocity);
            }

            if(target_pos.position.z > transform.translation.z &&  transform.translation.z + diff.z > target_pos.position.z) {
                transform.translation.z = target_pos.position.z;
                linear_velocity.z = 0.;
                info!("Se detiene z  {:?}!", linear_velocity);
            }
            else if(target_pos.position.z < transform.translation.z &&  transform.translation.z + diff.z < target_pos.position.z) {
                transform.translation.z = target_pos.position.z;
                linear_velocity.z = 0.;
                info!("Se detiene z  {:?}!", linear_velocity);
            }
        }
        //transform.translation += velocity.0 * time.delta_secs();
    }
}*/

pub fn get_velocity(mut query: Query<(&mut Transform, &mut TargetPos, &mut GameVelocity)>) {
    for (mut transform, target_pos, mut velocity) in &mut query {
        if (transform.translation.x != target_pos.position.x
            || transform.translation.z != target_pos.position.z)
        {
            //info!("target_pos.position {:?}! transform {:?}! ", target_pos.position, transform.translation);
            velocity.0 = calculate_velocity(transform.translation, target_pos.position);
        }
    }
}
pub fn apply_rapier3d_velocity_system(
    mut query: Query<(
        &GameVelocity,
        &Transform,
        &TargetPos,
        &mut KinematicCharacterController,
        Option<&KinematicCharacterControllerOutput>,
    )>,
    time: Res<Time>,
) {
    for (velocity, transform, target_pos, mut controller, output) in query.iter_mut() {
        let mut movement = Vec3::default();
        let delta_time = time.delta_secs();

        // Keep a downward component even while grounded. Rapier only performs
        // `snap_to_ground` when the requested translation points downward; a
        // zero Y request lets the capsule briefly leave a descending slope,
        // then gravity pulls it back on the following tick, which looks like
        // vertical vibration on the client.
        let vertical_speed = if output.map(|o| o.grounded).unwrap_or(false) {
            CHARACTER_GROUND_STICK_SPEED
        } else {
            CHARACTER_GRAVITY
        };
        movement.y = vertical_speed * delta_time * controller.custom_mass.unwrap_or(1.0);

        if (transform.translation.x != target_pos.position.x
            || transform.translation.z != target_pos.position.z)
        {
            let diff = velocity.0 * delta_time;
            // info!("current pos {:?}, target pos {:?}, diff {:?},last {:?}", transform.translation, target_pos.position, diff, time.elapsed().as_millis() );

            if (target_pos.position.x >= transform.translation.x
                && transform.translation.x + diff.x >= target_pos.position.x)
            {
                movement.x = target_pos.position.x - transform.translation.x;
                //info!("Se paso hacia la derecha  {:?}!", movement.x );
            } else if target_pos.position.x <= transform.translation.x
                && transform.translation.x + diff.x <= target_pos.position.x
            {
                movement.x = target_pos.position.x - transform.translation.x;
                //info!("Se paso hacia la izq  {:?}!", movement.x);
            } else {
                //info!("No se paso horizontal {:?}!", diff.x);
                movement.x = diff.x;
            }

            if (target_pos.position.z >= transform.translation.z
                && transform.translation.z + diff.z >= target_pos.position.z)
            {
                movement.z = target_pos.position.z - transform.translation.z;
                //info!("Se paso hacia arriba  {:?}!", movement.z);
            } else if (target_pos.position.z <= transform.translation.z
                && transform.translation.z + diff.z <= target_pos.position.z)
            {
                movement.z = target_pos.position.z - transform.translation.z;
                //info!("Se paso hacia abajo  {:?}!", movement.z);
            } else {
                //info!("No se paso  vertical{:?}!", diff.x);
                movement.z = diff.z;
            }

            /*if output.map(|o| o.grounded).unwrap_or(false) {
                info!("Esta en el piso !");
                *grounded_timer = GROUND_TIMER;
            }
            // If we are grounded we can jump
            if *grounded_timer > 0.0 {
                *grounded_timer -= delta_time;
            }
            movement.y += GRAVITY * delta_time * controller.custom_mass.unwrap_or(1.0);*/
        }

        // Always run the character controller, even while idle. This lets a
        // freshly loaded character settle onto the terrain instead of remaining
        // suspended or intersecting the map until the first accepted path.
        controller.translation = Some(movement);
    }
}

fn read_result_system(controllers: Query<(Entity, &KinematicCharacterControllerOutput)>) {
    for (entity, output) in controllers.iter() {
        println!(
            "Entity {:?} moved by {:?} and touches the ground: {:?}",
            entity, output.effective_translation, output.grounded
        );
    }
}

/*pub fn apply_velocity_system(mut query: Query<(&GameVelocity, &mut Transform, &TargetPos)>, time: Res<Time>) {
    for (velocity, mut transform, target_pos) in query.iter_mut() {

        if(transform.translation.x != target_pos.position.x || transform.translation.z != target_pos.position.z) {

            //info!("diff  {:?}!", diff);
            //info!("current pos  {:?}!", transform.translation);
            info!("target pos  {:?}!", target_pos.position);
            //info!("diff  {:?}!", diff);
            let diff = velocity.0 * time.delta_secs();
            //info!("diff  {:?}!", diff);

            if(target_pos.position.x >= transform.translation.x &&  transform.translation.x + diff.x >= target_pos.position.x) {
                transform.translation.x = target_pos.position.x;
            }
            else if target_pos.position.x <= transform.translation.x &&  transform.translation.x + diff.x <= target_pos.position.x {
                transform.translation.x = target_pos.position.x;
            }
            else {
                transform.translation.x +=  diff.x;
            }

            if(target_pos.position.z >= transform.translation.z &&  transform.translation.z + diff.z >= target_pos.position.z) {
                transform.translation.z = target_pos.position.z;
            }
            else if(target_pos.position.z <= transform.translation.z &&  transform.translation.z + diff.z <= target_pos.position.z) {
                transform.translation.z = target_pos.position.z;
            }
            else {
                //info!("se mueve vertical  {:?}!", diff.z);
                transform.translation.z +=  diff.z;
            }
        }
        //transform.translation += velocity.0 * time.delta_secs();
    }
}*/

#[allow(unused_parens)]
pub fn get_astar_successors(current_pos: &Pos, map: &Map) -> Vec<(Pos, u32)> {
    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    // info!("blocked_paths   {:?}!", blocked_paths);

    let mut possible_positions = vec![];

    // Si no hay nada arriba, puede ir hacia arriba
    if (!blocked_paths.contains(&Pos(x, z + 1))) {
        possible_positions.push(Pos(x, z + 1));
    }
    // Si no hay nada derecha, puede ir hacia derecha
    if (!blocked_paths.contains(&Pos(x + 1, z))) {
        possible_positions.push(Pos(x + 1, z));
    }
    // Si no hay nada izquierda, puede ir hacia izquierda
    if (!blocked_paths.contains(&Pos(x - 1, z))) {
        possible_positions.push(Pos(x - 1, z));
    }
    // Si no hay nada abajo, puede ir hacia abajo
    if (!blocked_paths.contains(&Pos(x, z - 1))) {
        possible_positions.push(Pos(x, z - 1));
    }
    // Si tiene nada arriba ni a la izq, diagonal arriba izq.
    if (!blocked_paths.contains(&Pos(x, z + 1))
        && !blocked_paths.contains(&Pos(x - 1, z))
        && !blocked_paths.contains(&Pos(x - 1, z + 1)))
    {
        possible_positions.push(Pos(x - 1, z + 1));
    }
    // Si tiene nada arriba ni a la derecha, diagonal arriba derecha.
    if (!blocked_paths.contains(&Pos(x, z + 1))
        && !blocked_paths.contains(&Pos(x + 1, z))
        && !blocked_paths.contains(&Pos(x + 1, z + 1)))
    {
        possible_positions.push(Pos(x + 1, z + 1));
    }
    // Si tiene nada abajo ni a la izq, diagonal abajo izq.
    if (!blocked_paths.contains(&Pos(x, z - 1))
        && !blocked_paths.contains(&Pos(x - 1, z))
        && !blocked_paths.contains(&Pos(x - 1, z - 1)))
    {
        possible_positions.push(Pos(x - 1, z - 1));
    }
    // Si tiene nada abajo ni a la derecha, diagonal abajo derecha.
    if (!blocked_paths.contains(&Pos(x, z - 1))
        && !blocked_paths.contains(&Pos(x + 1, z))
        && !blocked_paths.contains(&Pos(x + 1, z - 1)))
    {
        possible_positions.push(Pos(x + 1, z - 1));
    }

    // Si es que quisieras que se pueda diagonales.
    /*let mut possible_positions =  vec![
        Pos(x+1,z+1),
        Pos(x+1,z),
        Pos(x+1,z-1),
        Pos(x,z+1),
        Pos(x,z-1),
        Pos(x-1,z-1),
        Pos(x-1,z+1),
        Pos(x-1,z)
    ];

     possible_positions.retain(|pos| !blocked_paths.contains(&pos));

     */

    //info!("possible_positions   {:?}!", possible_positions);

    possible_positions.into_iter().map(|p| (p, 1)).collect()
}

pub fn get_path_between_translations(
    origin_translation: Vec3,
    destination_translation: Vec3,
    map: &Map,
) -> Option<(Vec<Pos>, u32)> {
    let start: Pos = Pos(
        origin_translation.x.round() as i32,
        origin_translation.z.round() as i32,
    );

    let goal: Pos = Pos(
        destination_translation.x.round() as i32,
        destination_translation.z.round() as i32,
    );

    // Tile bloqueado
    if (map.blocked_paths.contains(&goal)) {
        println!("Usuario quiere moverse a una celda prohibida.");
        return None;
    }

    let astar_result = astar(
        &start,
        |p| get_astar_successors(p, map),
        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
        |p| *p == goal,
    );

    astar_result.map(|(path, _cost)| (path, 0))
}

pub fn calculate_velocity(origin: Vec3, destination: Vec3) -> Vec3 {
    // info!("origin: {:?}, destination: {:?}!", origin, destination);
    let mut velocity: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let distance_x = destination.x - origin.x;
    //info!("distance_x  x: {:?}!", distance_x);

    if distance_x > 0.0 {
        velocity.x = PLAYER_MOVE_SPEED;
    } else if distance_x < 0.0 {
        velocity.x = -PLAYER_MOVE_SPEED;
    }

    let distance_z = destination.z - origin.z;
    //info!("distance_z  x: {:?}!", distance_z);

    if distance_z > 0.0 {
        velocity.z = PLAYER_MOVE_SPEED;
    } else if distance_z < 0.0 {
        velocity.z = -PLAYER_MOVE_SPEED;
    }

    velocity
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn fixed_rapier_schedule_updates_the_authoritative_transform() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
        ))
        .init_resource::<Assets<Mesh>>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ))
        .insert_resource(TimestepMode::Fixed {
            dt: 1.0 / 60.0,
            substeps: 1,
        })
        .add_systems(
            FixedUpdate,
            (get_velocity, apply_rapier3d_velocity_system)
                .chain()
                .before(PhysicsSet::SyncBackend),
        );

        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.0, 0.0),
                RigidBody::KinematicPositionBased,
                Collider::capsule_y(0.5, 0.5),
                KinematicCharacterController::default(),
                GameVelocity::default(),
                TargetPos {
                    position: Vec3::new(2.0, 1.0, 0.0),
                },
            ))
            .id();

        for _ in 0..10 {
            app.update();
        }

        assert!(
            app.world()
                .entity(entity)
                .get::<Transform>()
                .unwrap()
                .translation
                .x
                > 0.0
        );
    }

    #[test]
    fn real_server_player_bundle_walks_along_its_path() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PathingPlugin,
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
        ))
        .init_resource::<Assets<Mesh>>()
        .insert_resource(Map::default())
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ))
        .insert_resource(TimestepMode::Fixed {
            dt: 1.0 / 60.0,
            substeps: 1,
        });

        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.0, 0.0),
                LockedAxes::ROTATION_LOCKED,
                Collider::capsule_y(0.5, 0.5),
                ActiveCollisionTypes::KINEMATIC_STATIC,
                RigidBody::KinematicPositionBased,
                TransformInterpolation::default(),
                player_character_controller(),
                GravityScale(1.0),
                GameVelocity::default(),
                Facing(0),
                TargetPos {
                    position: Vec3::new(0.0, 1.0, 0.0),
                },
                Walking {
                    target_translation: Vec3::new(2.0, 1.0, 0.0),
                    path: Some((vec![Pos(0, 0), Pos(1, 0), Pos(2, 0)], 0)),
                },
            ))
            .id();

        for _ in 0..10 {
            app.update();
        }

        assert!(
            app.world()
                .entity(entity)
                .get::<Transform>()
                .unwrap()
                .translation
                .x
                > 0.0
        );
    }

    #[test]
    fn idle_server_player_settles_vertically() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
        ))
        .init_resource::<Assets<Mesh>>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ))
        .insert_resource(TimestepMode::Fixed {
            dt: 1.0 / 60.0,
            substeps: 1,
        })
        .add_systems(
            FixedUpdate,
            apply_rapier3d_velocity_system.before(PhysicsSet::SyncBackend),
        );

        let spawn = Vec3::new(-10.0, 5.0, 0.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_translation(spawn),
                RigidBody::KinematicPositionBased,
                Collider::capsule_y(0.5, 0.5),
                player_character_controller(),
                GameVelocity::default(),
                TargetPos { position: spawn },
            ))
            .id();

        for _ in 0..10 {
            app.update();
        }

        assert!(
            app.world()
                .entity(entity)
                .get::<Transform>()
                .unwrap()
                .translation
                .y
                < spawn.y
        );
    }

    #[test]
    fn grounded_character_keeps_a_downward_request_for_slope_snapping() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f64(1.0 / 60.0));
        world.insert_resource(time);

        let entity = world
            .spawn((
                Transform::from_xyz(0.0, 1.0, 0.0),
                GameVelocity(Vec3::X),
                TargetPos { position: Vec3::X },
                KinematicCharacterController::default(),
                KinematicCharacterControllerOutput {
                    grounded: true,
                    ..default()
                },
            ))
            .id();

        world
            .run_system_once(apply_rapier3d_velocity_system)
            .unwrap();

        let movement = world
            .get::<KinematicCharacterController>(entity)
            .and_then(|controller| controller.translation)
            .expect("the controller should receive a movement request");
        assert!((movement.y - CHARACTER_GROUND_STICK_SPEED / 60.0).abs() < 0.000_1);
    }

    #[test]
    fn navigation_blocks_submerged_and_outside_map_cells() {
        assert!(surface_is_submerged(Some(WATER_LEVEL - 0.01)));
        assert!(surface_is_submerged(None));
        assert!(!surface_is_submerged(Some(WATER_LEVEL)));
        assert!(!surface_is_submerged(Some(WATER_LEVEL + 1.0)));
    }

    #[test]
    fn walking_cleanup_ignores_an_entity_that_was_despawned() {
        let mut app = App::new();
        app.add_systems(Update, stop_walking_system);

        let entity = app
            .world_mut()
            .spawn((
                Walking {
                    target_translation: Vec3::ZERO,
                    path: None,
                },
                TargetPos {
                    position: Vec3::ZERO,
                },
            ))
            .id();

        // Initialize the RemovedComponents reader, then reproduce disconnect cleanup.
        app.update();
        app.world_mut().despawn(entity);
        app.update();

        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn walking_cleanup_clears_pending_motion() {
        let mut app = App::new();
        app.add_systems(Update, stop_walking_system);

        let mut controller = KinematicCharacterController::default();
        controller.translation = Some(Vec3::X);
        let entity = app
            .world_mut()
            .spawn((
                Walking {
                    target_translation: Vec3::X,
                    path: None,
                },
                TargetPos { position: Vec3::X },
                GameVelocity(Vec3::X),
                controller,
            ))
            .id();

        app.update();
        app.world_mut().entity_mut(entity).remove::<Walking>();
        app.update();

        let entity = app.world().entity(entity);
        assert!(entity.get::<TargetPos>().is_none());
        assert_eq!(entity.get::<GameVelocity>().unwrap().0, Vec3::ZERO);
        assert_eq!(
            entity
                .get::<KinematicCharacterController>()
                .unwrap()
                .translation,
            None
        );
    }

    #[test]
    fn damage_walk_delay_resumes_the_queued_destination() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Map::default())
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ))
            .add_systems(Update, tick_damage_walk_delays);

        let destination = Vec3::new(2.0, 1.0, 0.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.0, 0.0),
                DamageWalkDelay::new(Some(destination)),
                DamageWalkDelayImmunity::default(),
            ))
            .id();

        app.update();
        assert!(app.world().get::<Walking>(entity).is_none());

        for _ in 0..4 {
            app.update();
        }

        let walking = app.world().get::<Walking>(entity).unwrap();
        assert_eq!(walking.target_translation, destination);
        assert!(app.world().get::<DamageWalkDelay>(entity).is_none());
        assert!(app.world().get::<DamageWalkDelayImmunity>(entity).is_some());

        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get::<DamageWalkDelayImmunity>(entity).is_none());
    }
}
