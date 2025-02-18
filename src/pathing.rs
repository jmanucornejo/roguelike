use bevy::prelude::*;
use pathfinding::prelude::{astar, bfs};
use crate::*;

pub struct PathingPlugin;


#[derive(Component, Debug)]
pub struct TargetPos {
    pub position: Vec3
}

const GROUND_TIMER: f32 = 0.5;
const GRAVITY: f32 = -9.81;


impl Plugin for PathingPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app
            .add_systems(
            Startup, (
                    setup_prohibited_areas.after(setup_level),           
                    setup_prohibited_cells.after(setup_level),
                    setup_gravity.after(setup_level) 
                )            
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
                FixedUpdate, (      
                    walking_system,
                    stop_walking_system.before(get_velocity),             
                    get_velocity.after(stop_walking_system).before(PhysicsSet::StepSimulation),
                    apply_rapier3d_velocity_system.after(get_velocity).before(PhysicsSet::StepSimulation),                  
                
                    
                )
            );

            
        fn setup_gravity(mut rapier_config: ResMut<RapierConfiguration>) {
            rapier_config.gravity = Vec3::new(0.0, -9.81, 0.0);      
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
            map_entities: Query<(&MapEntity, &Handle<Mesh>, &Collider, &Transform)>,
            mut meshes: ResMut<Assets<Mesh>>,
            mut map: ResMut<Map>
        ) {
            for (map_entity, mesh_handle,  collider,  transform) in map_entities.iter() {

                let mesh = meshes.get_mut(mesh_handle).unwrap();

                let aabb = mesh.compute_aabb();

                if let Some(aabb) = aabb {

                    let starting_point_x = (transform.translation.x - aabb.half_extents.x)+0.5;
                    let ending_point_x = (transform.translation.x + aabb.half_extents.x)+0.5;
                    let starting_point_z = (transform.translation.z - aabb.half_extents.z)+0.5;
                    let ending_point_z = (transform.translation.z + aabb.half_extents.z)+0.5;
                 
                    for x in starting_point_x as i32..ending_point_x as i32 {
                        for z in starting_point_z as i32..ending_point_z as i32 {
                            let pos = Pos(x,z);                            
                            if !map.blocked_paths.contains(&pos) {
                                println!("Se agrega {:?} a blocked paths", pos) ;
                                map.blocked_paths.push(pos);
                            }                            
                        }
                    }
                    println!(" blocked paths {:?} ", map.blocked_paths) ;
                    
                
                }
            
                /*println!("mesh aabb  {:?}, ",aabb) ;
            
                let rotation = Rotation::default();

                let aabb = collider.aabb(transform.translation, *rotation);
                let range = aabb.min.x;
                println!("collideraabb  {:?}, ",aabb) ;*/
                //if(collider.hal)
            }
        }


        pub fn setup_prohibited_areas(mut map: ResMut<Map>, mut buildings: Query<(Entity, &mut Building)>) {
   
            for (_entity, mut building) in buildings.iter_mut() {
                info!("Building {:?}!", building.blocked_paths);
                map.blocked_paths.append(&mut building.blocked_paths);
                info!("blocked_paths {:?}!", map);
            }
        
        }

 
        pub fn stop_walking_system(
            mut removals: RemovedComponents<Walking>,
            mut commands: Commands
        ) {
            for entity in removals.read() {
                // do something with the entity
                commands.entity(entity).remove::<TargetPos>();
                eprintln!("Entity {:?} had the component removed.", entity);
            }
        }

        pub fn walking_system(
            mut walking_entities: Query<(Entity, &Player, &Transform, &Walking)>,
            mut commands: Commands,
            //map: Res<Map>
        ) {
            for (entity, player, transform, walking ) in walking_entities.iter_mut() {       

                /*info!("1. Ta parado en: {:?},  {:?}", Pos(
                    transform.translation.x.round() as i32, 
                    transform.translation.z.round() as i32
                ),  transform.translation);
                info!("2. Ta lejos. Acercarse!, path: {:?}", walking.path);*/

                if let Some((steps_vec, steps_left)) = walking.path.clone() {
     
                    if(walking.target_translation.x == transform.translation.x && walking.target_translation.z as f32 == transform.translation.z) {
                        info!("Se llegó al final, parar de caminar");
                        commands.entity(entity).remove::<Walking>();
                    }
                
                

                    let current_cell_index: Option<usize>  =  steps_vec.iter().position(|&r| r ==  Pos(
                        transform.translation.x.round() as i32, 
                        transform.translation.z.round() as i32
                    ));

                    //info!("3. current_cell_index: {:?}", current_cell_index);
                    if let Some(current_index) = current_cell_index {

                        let target_cell_index: usize = steps_left.try_into().expect("No se puedo cambiar de 32 bit a lo necesario");

                        // Aún no llega al mínimo requerido para validar ataque.
                        if(current_index < target_cell_index) {                        

                            if let Some(next_pos) = steps_vec.get(current_index+1) {
                                //info!("4. Final Pos: {:?}!", next_pos);    
                                // Se cambia el punto objetivo.
                                commands.entity(entity).insert(TargetPos {
                                    position: Vec3 { x: next_pos.0 as f32, y: 2.0, z: next_pos.1 as f32},
                                });         
                            }
            
                        }                        

                    }                            
            
                }
                
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

/*pub fn apply_avian3d_velocity_system(mut query: Query<(&mut LinearVelocity, &mut Transform, &TargetPos)>, time: Res<Time>) {
    for (mut linear_velocity, mut transform, target_pos) in query.iter_mut() {
    
        if(transform.translation.x != target_pos.position.x || transform.translation.z != target_pos.position.z) {
            info!("linear_velocity  {:?}!", linear_velocity);
            let diff = linear_velocity.0 * time.delta_seconds();
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
        //transform.translation += velocity.0 * time.delta_seconds();
    }
}*/

pub fn get_velocity(
    mut query: Query<(&mut Transform, &mut TargetPos, &mut GameVelocity)>

) {
    for (mut transform, target_pos,  mut velocity) in &mut query {
        if(transform.translation.x  != target_pos.position.x || transform.translation.z  != target_pos.position.z ) {
            //info!("target_pos.position {:?}! transform {:?}! ", target_pos.position, transform.translation);
            velocity.0 = calculate_velocity(transform.translation, target_pos.position);
                    
        }    
      
    }
}
pub fn apply_rapier3d_velocity_system(
    mut query: Query<(&GameVelocity, &mut Transform, &mut PrevState, &TargetPos, &mut KinematicCharacterController,  Option<&KinematicCharacterControllerOutput>)>, 
    time: Res<Time>,
    mut grounded_timer: Local<f32>,
) {
    for (velocity, mut transform, prev_state,  target_pos, mut controller, output) in query.iter_mut() {
        
        let mut movement = Vec3::default();
        let delta_time = time.delta_seconds();

        if output.map(|o| o.grounded).unwrap_or(false) {
            //info!("Esta en el piso !");
            *grounded_timer = GROUND_TIMER;
        }
        else {
            movement.y += GRAVITY * delta_time * controller.custom_mass.unwrap_or(1.0);
        }

        if(transform.translation.x != target_pos.position.x || transform.translation.z != target_pos.position.z) {
       
            let diff = velocity.0 * delta_time;        
            info!("current pos {:?}, target pos {:?}, diff {:?},last {:?}", transform.translation, target_pos.position, diff, time.elapsed().as_millis() );
                      
            if(target_pos.position.x >= transform.translation.x &&  transform.translation.x + diff.x >= target_pos.position.x) {                
                movement.x =  target_pos.position.x - transform.translation.x;
                //info!("Se paso hacia la derecha  {:?}!", movement.x );                           
            }
            else if target_pos.position.x <= transform.translation.x &&  transform.translation.x + diff.x <= target_pos.position.x {    
                movement.x =  target_pos.position.x - transform.translation.x;
                //info!("Se paso hacia la izq  {:?}!", movement.x);
            }
            else {
                //info!("No se paso horizontal {:?}!", diff.x);
                movement.x =  diff.x;
            }

            if(target_pos.position.z >= transform.translation.z &&  transform.translation.z + diff.z >= target_pos.position.z) {
                movement.z =  target_pos.position.z - transform.translation.z;
                //info!("Se paso hacia arriba  {:?}!", movement.z); 
            }
            else if(target_pos.position.z <= transform.translation.z &&  transform.translation.z + diff.z <= target_pos.position.z) {
                movement.z =  target_pos.position.z - transform.translation.z;
                //info!("Se paso hacia abajo  {:?}!", movement.z);
            }
            else {
                //info!("No se paso  vertical{:?}!", diff.x);
                movement.z =  diff.z;
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
        
           
            controller.translation = Some(movement);
        }
        else if(controller.translation !=  None){
            controller.translation = None;
        }
        //transform.translation += velocity.0 * time.delta_seconds();
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
            let diff = velocity.0 * time.delta_seconds();
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
        //transform.translation += velocity.0 * time.delta_seconds();
    }
}*/

#[allow(unused_parens)]
pub fn get_astar_successors(current_pos: &Pos, map: &Res<Map>) -> Vec<(Pos, u32)> {

    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    // info!("blocked_paths   {:?}!", blocked_paths);

  
     let mut possible_positions =  vec![];

   // Si no hay nada arriba, puede ir hacia arriba
    if(!blocked_paths.contains(&Pos(x,z+1))) {
        possible_positions.push(Pos(x,z+1));
    }
    // Si no hay nada derecha, puede ir hacia derecha
    if(!blocked_paths.contains(&Pos(x+1,z))) {
        possible_positions.push(Pos(x+1,z));
    }
    // Si no hay nada izquierda, puede ir hacia izquierda
    if(!blocked_paths.contains(&Pos(x-1,z))) {
        possible_positions.push(Pos(x-1,z));
    }
    // Si no hay nada abajo, puede ir hacia abajo
    if(!blocked_paths.contains(&Pos(x,z-1))) {
        possible_positions.push(Pos(x,z-1));
    }
    // Si tiene nada arriba ni a la izq, diagonal arriba izq.
    if(!blocked_paths.contains(&Pos(x,z+1)) && !blocked_paths.contains(&Pos(x-1,z)) && !blocked_paths.contains(&Pos(x-1,z+1))) {
        possible_positions.push(Pos(x-1,z+1));
    }
    // Si tiene nada arriba ni a la derecha, diagonal arriba derecha.
    if(!blocked_paths.contains(&Pos(x,z+1)) && !blocked_paths.contains(&Pos(x+1,z)) && !blocked_paths.contains(&Pos(x+1,z+1))) {
        possible_positions.push(Pos(x+1,z+1));
    }
    // Si tiene nada abajo ni a la izq, diagonal abajo izq.
    if(!blocked_paths.contains(&Pos(x,z-1)) && !blocked_paths.contains(&Pos(x-1,z)) && !blocked_paths.contains(&Pos(x-1,z-1))) {
        possible_positions.push(Pos(x-1,z-1));
    }
    // Si tiene nada abajo ni a la derecha, diagonal abajo derecha.
    if(!blocked_paths.contains(&Pos(x,z-1)) && !blocked_paths.contains(&Pos(x+1,z)) && !blocked_paths.contains(&Pos(x+1,z-1))) {
        possible_positions.push(Pos(x+1,z-1));
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

pub fn get_path_between_translations(origin_translation: Vec3, destination_translation: Vec3, map: &Res<Map>) -> Option<(Vec<Pos>, u32)> {

    let start: Pos = Pos(
        origin_translation.x.round() as i32, 
        origin_translation.z.round() as i32
    );

    let goal: Pos = Pos(
        destination_translation.x.round() as i32, 
        destination_translation.z.round() as i32
    );    

    // Tile bloqueado
    if(map.blocked_paths.contains(&goal)) {
        println!("Usuario quiere moverse a una celda prohibida.");
        return None
    }
          

    let astar_result = astar(
        &start,
        |p|  get_astar_successors(p, &map),
        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
        |p| *p==goal);

    return astar_result;

}

fn get_succesors(current_pos: &Pos, mut map: &ResMut<Map>) -> Vec<Pos> {

    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    //info!("blocked_paths   {:?}!", blocked_paths);
    let mut possible_positions =  vec![Pos(x+1,z+1), Pos(x+1,z), Pos(x+1,z-1), Pos(x,z+1),
    Pos(x,z-1), Pos(x-1,z-1), Pos(x-1,z+1), Pos(x-1,z)];

    possible_positions.retain(|pos| !blocked_paths.contains(&pos));


    //info!("possible_positions   {:?}!", possible_positions);
    // se le agrega el peso
    possible_positions

}

pub fn calculate_velocity(origin: Vec3, destination: Vec3) -> Vec3 {

    // info!("origin: {:?}, destination: {:?}!", origin, destination);   
    let mut velocity: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0
    };
    let distance_x = destination.x -origin.x;
    //info!("distance_x  x: {:?}!", distance_x);   

    if distance_x > 0.0 {
        velocity.x = PLAYER_MOVE_SPEED;
    }
    else if  distance_x < 0.0 {
        velocity.x = -PLAYER_MOVE_SPEED;
    }

    let distance_z = destination.z - origin.z;
    //info!("distance_z  x: {:?}!", distance_z);   

    if distance_z > 0.0 {
        velocity.z = PLAYER_MOVE_SPEED;
    }
    else if  distance_z < 0.0 {
        velocity.z = -PLAYER_MOVE_SPEED;
    }                            
   
    velocity
}
