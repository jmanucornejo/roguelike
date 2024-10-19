

use bevy::prelude::*;
use pathfinding::prelude::astar;
use pathing::{get_astar_successors, get_path_between_translations, get_next_step, TargetPos};
use crate::*;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app                     
            .add_systems(
                FixedUpdate, ( 
                    recalculate_path.before(crate::pathing::apply_velocity_system),
                    attack                 
                )
            );

        fn attack(
            attacking_entities: Query<(Entity, &Transform, &mut Attacking), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            attacked_entities: Query<(Entity, &Transform), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut commands: Commands,
            spatial_query: SpatialQuery,
            map_query: Query<&MapEntity>,
        ) {
            for (entity, attacking_transform,  mut attacking) in attacking_entities.iter() {                   
             
                let attack_range:f32 = 10.;
                // If in range, attack.
                info!("1. Se ataca entity {:?}", attacking.enemy);
                if(is_in_attack_range(attack_range, attacking_transform.translation, attacking.enemy_translation) && is_in_view(&spatial_query, attacking_transform.translation, attacking.enemy_translation, attacking.enemy, &map_query)) {
                    //info!("Ta cerca. Parar movimiento y empezar a pegar! ");
                    //commands.entity(entity).remove::<TargetPos>();
                }   
                else { // walk until in range.
                     
                    info!("2. Ta parado en: {:?},  {:?}", Pos(
                        attacking_transform.translation.x.round() as i32, 
                        attacking_transform.translation.z.round() as i32
                    ),  attacking_transform.translation);
                    info!("3. Ta lejos. Acercarse!, path: {:?}", attacking.path);
                   
                    // Si hay camino, se intenta acercar.
                    if let Some((steps_vec, steps_left)) = attacking.path.clone() {

                        let current_cell_index: Option<usize>  =  steps_vec.iter().position(|&r| r ==  Pos(
                            attacking_transform.translation.x.round() as i32, 
                            attacking_transform.translation.z.round() as i32
                        ));
                        
                        info!("3. current_cell_index: {:?}", current_cell_index);
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
                                    info!("5. Final Pos: {:?}!", final_pos);    
                                    // Se cambia el punto objetivo.
                                    commands.entity(entity).insert(TargetPos {
                                        position: Vec3 { x: final_pos.0 as f32, y: 2.0, z: final_pos.1 as f32},
                                    });         
                                }
                
                            //}

                        }      
                    }   
                }
            }
        }

        
        pub fn recalculate_path(
            mut attackers: Query<(&Transform, &mut Attacking), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            enemies: Query<(&Transform), ( Or<(With<Player>, With<NPC>, With<Monster>)>, Changed<Transform>)>,
            map: Res<Map>
        ) {

            for (attacking_transform,  mut attacking) in attackers.iter_mut() {   

                let mut enemy_translation_changed = false;
                // Caso 1. El enemigo objetivo se ha movido.
                if let Ok( (enemy_transform)) = enemies.get(attacking.enemy) {

                    if(attacking.enemy_translation != enemy_transform.translation) {
                        attacking.enemy_translation = enemy_transform.translation;
                        enemy_translation_changed = true;
                    }
                 
                }

                 // Caso 2. El mapa ha cambiado. Esto podría pasar si implementamos por ejemplo magias como "Icewall" que puedan bloquear el camino temporalmente.
                if (map.is_changed() || enemy_translation_changed) {
                    attacking.path = get_path_between_translations(attacking_transform.translation, attacking.enemy_translation, &map);    
                } 
                
            }
            
        }
      
            
    } 


}


pub fn is_in_view(spatial_query: &SpatialQuery, origin_translation: Vec3, target_translation: Vec3, target_entity: Entity, map_query: &Query<&MapEntity>) -> bool {

    let xyz = (target_translation - origin_translation).normalize_or_zero();

    let direction = Dir3::from_xyz(xyz.x, xyz.y, xyz.z);    

    let direction = if let Ok(direction) = direction {  direction  }  else { return false; };

    /*info!("origen {:?}, destino {:?}, dirección {:?}", origin_translation, target_translation, direction);
    if let Some(first_hit) = spatial_query.cast_ray(
        origin_translation,                    // Origin
        direction,                       // Direction
        15.,                         // Maximum time of impact (travel distance)
        false,                          // Does the ray treat colliders as "solid"
        SpatialQueryFilter::default(), // Query filter
    ) {     
      
        println!("First hit: {:?}", first_hit);
        /*println!(
            "Hit entity {:?} at {} with normal {}",
            first_hit.entity,
            ray.origin + *ray.direction * first_hit.time_of_impact,
            first_hit.normal,
        );*/     

        
    }

    let mut hits = vec![];

    // Cast ray and get all hits
    spatial_query.ray_hits_callback(
        origin_translation,                    // Origin
        direction,                       // Direction
        15.0,                         // Maximum time of impact (travel distance)
        false,                          // Does the ray treat colliders as "solid"
        SpatialQueryFilter::default(), // Query filter
        |hit| {                        // Callback function
            hits.push(hit);
            true
        },
    );


    println!("hits: {:?}", hits);*/

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
}

pub fn is_in_attack_range(attack_range: f32, attacker_translation: Vec3, attacked_translation: Vec3) -> bool {

    let distance = (attacker_translation - attacked_translation).round();
 
    if(distance.x.abs() <= attack_range && distance.z.abs() <= attack_range) {
        //info!("esta en attack range");
        return true;
    }
  
    info!("0. distance {:?}", distance);
    
    return false;

}