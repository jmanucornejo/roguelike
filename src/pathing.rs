use bevy::prelude::*;
use crate::*;

pub struct PathingPlugin;


#[derive(Component, Debug)]
pub struct CurrentMovementState {
    pub position: Vec3
}



impl Plugin for PathingPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app
            .add_systems(
            Startup, (
                    setup_prohibited_areas.after(setup_level),            
                )            
            )
            .add_systems(
            FixedUpdate, (
                    apply_velocity_system,
                    get_velocity
                    //client_velocity.run_if(in_state(AppState::InGame)),
                )
            );


        pub fn setup_prohibited_areas(mut map: ResMut<Map>, mut buildings: Query<(Entity, &mut Building)>) {
   
            for (_entity, mut building) in buildings.iter_mut() {
                info!("Building {:?}!", building.blocked_paths);
                map.blocked_paths.append(&mut building.blocked_paths);
                info!("blocked_paths {:?}!", map);
            }
        
        }

        
        fn get_velocity(
            mut query: Query<(&mut Transform, &mut CurrentMovementState, &mut Velocity)>,

        ) {
            for (mut transform, state,  mut velocity) in &mut query {
                if(transform.translation  != state.position) {
                    velocity.0 = calculate_velocity(transform.translation, state.position);
                }      
            }
        }


        fn apply_velocity_system(mut query: Query<(&Velocity, &mut Transform, &CurrentMovementState)>, time: Res<Time>) {
            for (velocity, mut transform, target_pos) in query.iter_mut() {

                if(transform.translation.x != target_pos.position.x || transform.translation.z != target_pos.position.z) {

               
                    info!("current pos  {:?}!", transform.translation);
                    info!("target pos  {:?}!", target_pos.position);
                    let diff = velocity.0 * time.delta_seconds();
                    info!("diff  {:?}!", diff);
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
                        info!("se mueve vertical  {:?}!", diff.z);
                        transform.translation.z +=  diff.z;
                    }
                }
                //transform.translation += velocity.0 * time.delta_seconds();
            }
        }
        
     
    }


    
}


pub fn get_astar_successors(current_pos: &Pos, mut map: &ResMut<Map>) -> Vec<(Pos, u32)> {

    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    //info!("blocked_paths   {:?}!", blocked_paths);

  
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

    info!("possible_positions   {:?}!", possible_positions);

    possible_positions.into_iter().map(|p| (p, 1)).collect()

}


fn get_succesors(current_pos: &Pos, mut map: &ResMut<Map>) -> Vec<Pos> {

    let &Pos(x, z) = current_pos;

    let blocked_paths = &map.blocked_paths;
    //info!("blocked_paths   {:?}!", blocked_paths);
    let mut possible_positions =  vec![Pos(x+1,z+1), Pos(x+1,z), Pos(x+1,z-1), Pos(x,z+1),
    Pos(x,z-1), Pos(x-1,z-1), Pos(x-1,z+1), Pos(x-1,z)];

    possible_positions.retain(|pos| !blocked_paths.contains(&pos));


    info!("possible_positions   {:?}!", possible_positions);
    // se le agrega el peso
    possible_positions

}

pub fn calculate_velocity(origin: Vec3, destination: Vec3) -> Vec3 {

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