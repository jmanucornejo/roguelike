

use bevy::prelude::*;
use pathing::{get_path_between_translations, TargetPos};
use crate::{shared::{channels::ServerChannel, components::*, messages::ServerMessages}, *};
// use avian3d::{parry::shape, prelude::*};
use shared::states::ServerState;

#[derive(Debug, Serialize, Deserialize)]
pub enum DamageType {
    Normal,
    Critical,
}

#[derive(Event)]
pub struct DamageTick {
    pub entity: Entity,
    pub damage: u32,
    pub damage_type: DamageType
}

#[derive(Event)]
struct AttackAnimation {
    entity: Entity,
    enemy: Entity,
    attack_speed: f32,
    auto_attack: bool            
}



pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app                     
            .add_systems(
                FixedUpdate, ( 
                    network_change_attacking_state.run_if(in_state(ServerState::InGame)),
                    network_send_delta_health_system.run_if(in_state(ServerState::InGame)),
                    recalculate_path.before(crate::pathing::apply_rapier3d_velocity_system),
                    aggro_rapier3d.run_if(in_state(ServerState::InGame)).before(crate::pathing::apply_rapier3d_velocity_system),    
                    attack.run_if(in_state(ServerState::InGame)),
                )
            )
            .add_observer(on_damage);
            //.observe(on_attack_animation);


        fn aggro_rapier3d(
            mut aggroed_entities: Query<(Entity, &Transform, &mut Aggro, Option<&mut Attacking>, Option<&mut Walking>), (Or<(With<Player>, With<NPC>, With<Monster>)>)>,
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
          
                for (entity, attacker_transform,  aggroed, is_attacking, mut is_walking) in aggroed_entities.iter_mut() {                   
                    
                    let attack_range:f32 = 1.;
                    // If in range, attack.         
                    if(is_in_attack_range(attack_range, attacker_transform.translation, aggroed.enemy_translation) 
                    && is_in_view_rapier3d(&rapier_context, attacker_transform.translation, aggroed.enemy_translation, aggroed.enemy, &map_query)
                    // && is_attacking.is_none()
                    ) {
                        
                
                
                        if is_attacking.is_some() {
                            info!("walking? {:?}", is_walking);  
                            continue;
                        }
                        info!("ATACARRRRRRRRRRRR");  
                        // STOP WALKING. ALREADY NEAR TARGET.
                        is_walking = None;
                                
                        let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
                        timer.pause(); // Timer pausado hasta que este en rango de ataque;         

                        /*commands.trigger(AttackAnimation { 
                            entity: entity,
                            enemy: aggroed.enemy,
                            attack_speed: 0.5,
                            auto_attack: aggroed.auto_attack
                        });   */   

                        commands.entity(entity)
                        .insert(AttackingTimer(timer))
                        .insert(Attacking {
                            enemy: aggroed.enemy,
                            auto_attack: aggroed.auto_attack,
                            //enemy_translation: aggroed.enemy_translation,
                        // timer: timer                                
                        }).remove::<Walking>().remove::<TargetPos>();              
                        
                        continue;

                    }   
                
                    if let Some(walking) = is_walking {

                        if(walking.target_translation == aggroed.enemy_translation) {
                            //info!("Already walking. {:?}", walking);  
                            continue;
                        }
                    
                    }
                        
                    
                    info!("No esta en attack range ni puede ver al enemigo. No está caminando. Se cambia a caminando.");
                    let path =  get_path_between_translations(attacker_transform.translation, aggroed.enemy_translation, &map);
                    info!("Se calcula camino nuevo hacia el enemigo que está en {:?} {:?}",  aggroed.enemy_translation, path);
                            
                    commands.entity(entity).insert(Walking {
                        target_translation: aggroed.enemy_translation,
                        path: path,                               
                    })
                    .remove::<Attacking>()
                    .remove::<AttackingTimer>();
                
                
                
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
            mut attacking_entities: Query<(Entity, &mut Attacking, &mut AttackingTimer), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            mut commands: Commands,
            time: Res<Time>,
        ) {  

            for (entity,  attacking, mut attacking_timer) in attacking_entities.iter_mut() {    
                // Los timers de atraque empiezan detenidos. 
                // Se inicia cuando ya esta en rango y las validaciones son exitosas
                if attacking_timer.0.paused() {
                    info!("El timer está parado. No se ha empezado a atacar aún.");
                    let attack_speed = 0.5;
                    if(attacking.auto_attack == false) {
                        attacking_timer.0 = Timer::from_seconds(attack_speed, TimerMode::Once);
                    }
                    else {
                        attacking_timer.0 = Timer::from_seconds(attack_speed, TimerMode::Repeating);
                    }                   
                    continue;
                }
                
                // con el aspd que inicio el timer, se empieza a correr el tiempo.
                // Cuando llega al final, se envía el evento de ataque.
                attacking_timer.0.tick(time.delta());
                
                if(!attacking_timer.0.just_finished()) {
                    continue;
                }                      
                
                info!("Finalizó el timer. Timer: {:?}", attacking_timer.0);
                commands.trigger(DamageTick { 
                    entity: attacking.enemy,
                    damage: 5,
                    damage_type: DamageType::Normal
                });      

                if(attacking.auto_attack == false) {
                    commands.entity(entity).remove::<Aggro>().remove::<Attacking>().remove::<AttackingTimer>();
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
                    commands.trigger(DamageTick { 
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
            mut attackers: Query<(&mut Walking, &Transform, &mut Aggro), ( Or<(With<Player>, With<NPC>, With<Monster>)>)>,
            enemies: Query<(&Transform), ( Or<(With<Player>, With<NPC>, With<Monster>)>, Changed<Transform>)>,
            map: Res<Map>
        ) {

            for (mut walking, attacker_transform,  mut aggroed) in attackers.iter_mut() {   

                let mut enemy_translation_changed = false;
                // Caso 1. El enemigo objetivo se ha movido.
                if let Ok( (enemy_transform)) = enemies.get(aggroed.enemy) {

                    if(aggroed.enemy_translation != enemy_transform.translation) {
                        aggroed.enemy_translation = enemy_transform.translation;
                        enemy_translation_changed = true;
                    }
                 
                }

                 // Caso 2. El mapa ha cambiado. Esto podría pasar si implementamos por ejemplo magias como "Icewall" que puedan bloquear el camino temporalmente.
                if (map.is_changed() || enemy_translation_changed) {
                    
                    walking.path = get_path_between_translations(attacker_transform.translation, aggroed.enemy_translation, &map);    
                    info!("Cambio el translation del enemigo: {:?}",walking.path);
                    /*
                    aggroed.path = get_path_between_translations(attacker_transform.translation, aggroed.enemy_translation, &map);  */  
                } 
                
            }
            
        }
      
       
        /*fn on_attack_animation(
            trigger: Trigger<AttackAnimation>, 
            mut query: Query<(Entity, &mut Health)>,
            mut commands: Commands,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let attack_animation: &AttackAnimation = trigger.event();
            let id: Entity = damage_tick.entity;

            if let Ok((entity, mut health)) = query.get_mut(id) {
                info!("Entity  {:?} damaged.", id.index());
                if(health.current <= damage_tick.damage) {

                    commands.entity(entity).despawn();
                    info!("Muere la entidad:  {:?} ", entity);
                    // Si es jugador, mantenrlo muerto en el piso.
                    // Si es monstruo, debe soltar ítems.
                    
                }
                else {
                    health.current -= damage_tick.damage;
                    info!("Health  {:?} ", health);
                }
           
            }          
    
        }*/

        fn on_damage(
            trigger: Trigger<DamageTick>, 
            mut query: Query<(Entity, &mut Health)>,
            mut commands: Commands,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let damage_tick: &DamageTick = trigger.event();
            let id: Entity = damage_tick.entity;

            if let Ok((entity, mut health)) = query.get_mut(id) {
                info!("Entity  {:?} damaged.", id.index());
                if(health.current <= damage_tick.damage) {

                    commands.entity(entity).despawn();
                    info!("Muere la entidad:  {:?} ", entity);
                    // Si es jugador, mantenrlo muerto en el piso.
                    // Si es monstruo, debe soltar ítems.
                    
                }
                else {
                    health.current -= damage_tick.damage;
                    info!("Health  {:?} ", health);
                }
           
            }          
    
        }

        pub fn network_change_attacking_state(
            mut server: ResMut<RenetServer>, 
            players: Query<(&Player, &LineOfSight)>,
            mut entities: Query<(Entity, &Attacking, &AttackSpeed),  Or<(Changed<Attacking>, Changed<AttackSpeed>)> >
        ) {


            for (player, line_of_sight) in players.iter() {
            
                for entity in line_of_sight.0.iter() {           

                    if let Ok( (entity, attacking, attack_speed)) = entities.get_mut(*entity) {
                        
                        let message= ServerMessages::Attack {
                            entity,
                            enemy: attacking.enemy,
                            attack_speed: attack_speed.0,
                            auto_attack:  attacking.auto_attack
                        };

                        let sync_message = bincode::serialize(&message).unwrap();
                        // Send message to only one client
                        server.send_message(player.id, ServerChannel::ServerMessages, sync_message);                    
        
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

                    if let Ok( (entity, health)) = entities.get_mut(*entity) {
                        
                        let message= ServerMessages::HealthChange {
                            entity,
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
    } 


}

pub fn is_in_view_rapier3d(
    rapier_context: &RapierContext, 
    origin_translation: Vec3, 
    target_translation: Vec3, 
    target_entity: Entity, 
    map_query: &Query<&MapEntity>
) -> bool {

    let direction = (target_translation - origin_translation).normalize_or_zero();
  
    let predicate = |handle| {
        // We can use a query to bevy inside the predicate.
        map_query
            .contains(handle) || handle == target_entity
           
    };

    if let Some((entity, _time_of_impact)) = rapier_context.cast_ray(
        origin_translation, 
        direction, 
        bevy_rapier3d::prelude::Real::MAX, 
        true, 
        QueryFilter::default().predicate( &predicate)) {

        if(entity == target_entity) {
            //println!("PUEDO VER AL OBJETIVO: {:?}", entity);
            return true;
        }      
        else {
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

pub fn is_in_attack_range(attack_range: f32, attacker_translation: Vec3, attacked_translation: Vec3) -> bool {

    // let distance = (attacker_translation - attacked_translation).round();
    let distance = attacker_translation - attacked_translation;
    info!("Distancia {:?}", distance);
    if(distance.x.abs() <= attack_range && distance.z.abs() <= attack_range) {
        info!("esta en attack range");
        return true;
    }  
    
    return false;

}