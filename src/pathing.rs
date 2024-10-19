use bevy::prelude::*;
use pathfinding::prelude::{astar, bfs};
use crate::*;

pub struct PathingPlugin;


#[derive(Component, Debug)]
pub struct TargetPos {
    pub position: Vec3
}




impl Plugin for PathingPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app
            .add_systems(
            Startup, (
                    setup_prohibited_areas.after(setup_level),           
                    setup_prohibited_cells.after(setup_level) 
                )            
            )
            .add_systems(
            FixedUpdate, (                   
                    get_velocity,
                    apply_velocity_system.after(get_velocity),
                    walking_system
                    //client_velocity.run_if(in_state(AppState::InGame)),
                )
            );


            
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

        pub fn walking_system(
            mut walking_entities: Query<(Entity, &Transform,  &mut Walking), With<Player>>,
            mut commands: Commands,
            //map: Res<Map>
        ) {
            for (entity, transform,  mut walking) in walking_entities.iter_mut() {       

                /*info!("1. Ta parado en: {:?},  {:?}", Pos(
                    transform.translation.x.round() as i32, 
                    transform.translation.z.round() as i32
                ),  transform.translation);
                info!("2. Ta lejos. Acercarse!, path: {:?}", walking.path);*/

                if let Some((steps_vec, steps_left)) = walking.path.clone() {

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
                        else {
                            // Llegó al destino
                            commands.entity(entity).remove::<Walking>(); 
                        }

                    }                            
                    /*else if let Some(goal) = steps_vec.last()  {
                        info!("3 Se salió del camino!!");                        
                        walking.path = get_path_between_translations(transform.translation,  Vec3 { x: goal.0 as f32, y: 2.0, z: goal.1 as f32}, &map);    

                        if let Some((steps_vec, steps_left)) = walking.path.clone() {

                            let mut index = 1;
                    
                            if(steps_left == 0) {
                                index = 0;
                            }   
                    
                            if let Some(next_pos) = steps_vec.get(index) {

                                commands.entity(entity).insert(TargetPos {
                                    position: Vec3 { x: next_pos.0 as f32, y: 2.0, z: next_pos.1 as f32},
                                });                               
                         
                            }
                           
                        }   
                             
                    }
                    else {
                        panic!("Oh no something bad has happened!")
                    }*/
                }
            }  
        }

        pub fn get_velocity(
            mut query: Query<(&Transform, &mut TargetPos, &mut Velocity)>,

        ) {
            for (mut transform, state,  mut velocity) in &mut query {
                if(transform.translation.x  != state.position.x || transform.translation.z  != state.position.z ) {
                    //info!("state.position {:?}!", state.position);
                    velocity.0 = calculate_velocity(transform.translation, state.position);
                }    
              
            }
        }

    

       
    }


    
}

pub fn apply_velocity_system(mut query: Query<(&Velocity, &mut Transform, &TargetPos)>, time: Res<Time>) {
    for (velocity, mut transform, target_pos) in query.iter_mut() {

        if(transform.translation.x != target_pos.position.x || transform.translation.z != target_pos.position.z) {

            //info!("diff  {:?}!", diff);
            //info!("current pos  {:?}!", transform.translation);
            //info!("target pos  {:?}!", target_pos.position);
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
}


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

pub fn get_next_step(initial: Vec3, goal: Pos, map: &Res<Map>) -> Option<Vec3> {

    let start: Pos = Pos(
        initial.x.round() as i32, 
        initial.z.round() as i32
    );

    // Ya esta en el objetivo
    if(goal.0 as f32 == initial.x && goal.1  as f32 == initial.z) {
        return None
    }
    // Tile bloqueado
    if(map.blocked_paths.contains(&goal)) {
        return None
    }
               

    //info!("Start   {:?}!  Goal  {:?}!", start,goal);

    //let succesors = get_succesors(&start, &map);                        
    let astar_result = astar(
        &start,
        |p|  get_astar_successors(p, &map),
        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
        |p| *p==goal);


    //info!("*Star Result {:?}! ",astar_result);  
     
    //if let None = astar_result   

    if let Some((steps_vec, steps_left)) = astar_result {

        let mut index = 1;

        if(steps_left == 0) {
            index = 0;
        }   

        if let Some(final_pos) = steps_vec.get(index) {
        
            let &Pos(x, z) = final_pos;

            return Some(Vec3 { x: x as f32, y: 2.0, z: z as f32})

        }
       
    }   
   
    return None

}