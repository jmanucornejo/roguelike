

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
        ) {
            for (entity, attacking_transform,  mut attacking) in attacking_entities.iter() {                   
             
                let attack_range = 2.0;
                // If in range, attack.
                if(is_in_attack_range(attack_range, attacking_transform.translation, attacking.enemy_translation) && is_in_view()) {
                    //info!("Ta cerca. Parar movimiento y empezar a pegar! ");
                    //commands.entity(entity).remove::<TargetPos>();
                }   
                else { // walk until in range.
                    info!("1. Ta parado en: {:?},  {:?}", Pos(
                        attacking_transform.translation.x.round() as i32, 
                        attacking_transform.translation.z.round() as i32
                    ),  attacking_transform.translation);
                    info!("2. Ta lejos. Acercarse!, path: {:?}", attacking.path);
                    // Si hay camino, se intenta acercar.
                    if let Some((steps_vec, steps_left)) = attacking.path.clone() {

                        let current_cell_index: Option<usize>  =  steps_vec.iter().position(|&r| r ==  Pos(
                            attacking_transform.translation.x.round() as i32, 
                            attacking_transform.translation.z.round() as i32
                        ));
                        
                        info!("3. current_cell_index: {:?}", current_cell_index);
                        if let Some(current_index) = current_cell_index {

                            let target_cell_index: usize = (steps_left - attack_range as u32).try_into().expect("No se puedo cambiar de 32 bit a lo necesario");

                            // Aún no llega al mínimo requerido para validar ataque.
                            if(current_index < target_cell_index) {                            

                                if let Some(final_pos) = steps_vec.get(current_index+1) {
                                    info!("4. Final Pos: {:?}!", final_pos);    
                                    // Se cambia el punto objetivo.
                                    commands.entity(entity).insert(TargetPos {
                                        position: Vec3 { x: final_pos.0 as f32, y: 2.0, z: final_pos.1 as f32},
                                    });         
                                }
                
                            }

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


pub fn is_in_view() -> bool {
    info!("Ta a la vista ");
    return true;
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