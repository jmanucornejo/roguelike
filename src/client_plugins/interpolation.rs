
use bevy::prelude::*;
use crate::*;
use std::collections::VecDeque;
use std::ops::Mul;


#[derive(Default, Resource)]
pub struct ClockOffset(pub u128);


#[derive(Default, Resource)]
pub struct ServerTime(pub u128);

#[derive(Component, Debug)]
pub struct PositionHistory {
    buffer: VecDeque<(IVec3, u128, bool)>, // (timestamp, delta position, processed)
    buffer_duration: u128,          // Duration of the buffer in seconds
    prev_position: Vec3, 
    next_position: Vec3
}


impl PositionHistory  {

    pub fn new(position: Vec3) -> Self {
        Self {
            buffer: VecDeque::new(),
            buffer_duration: 200,
            prev_position: position,
            next_position: position
        }
    }

    pub fn add_delta(&mut self,  delta_position: IVec3, timestamp: u128) {
        self.buffer.push_back((delta_position, timestamp, false));     
    }

    pub fn interpolate_delta_positions(&mut self, target_timestamp: u128) -> Option<Vec3> {
        if self.buffer.len() < 2 {
            return None; // Not enough data to interpolate
        }

        // Find two states to interpolate between
        let mut previous = None;
        let mut next = None;


      
        // Iterate through the buffer to find the appropriate deltas to interpolate between
        for i in 0..self.buffer.len() - 1 {
            let (delta0, t0, processed0) = self.buffer[i];
            let (delta1, t1, processed1) = self.buffer[i + 1];
            if t0 <= target_timestamp && target_timestamp <= t1 {
                previous = Some((delta0.as_vec3().mul(TRANSLATION_PRECISION),t0,processed0));
                next = Some((delta1.as_vec3().mul(TRANSLATION_PRECISION), t1, processed1));
 
                self.buffer[i] = (delta0, t0, true);
                break;
            }
            else if(t1 <= target_timestamp && i+1 == self.buffer.len() - 1 && self.prev_position != self.next_position) {
                self.prev_position = self.next_position;
                self.buffer[i + 1] = (delta1, t1, true);
                println!("Se procesa fin de la cola {:?}", t1);
                return Some(self.next_position);
            }
        }

        // Perform interpolation based on the deltas
        if let (Some((delta0, t0, processed0)), Some((delta1, t1, processed1))) = (previous, next) {
            println!("delta0 {:?}, delta1 {:?}, t0 {:?}, t1 {:?},processed0 {:?}, processed1 {:?}", delta0, delta1, t0, t1, processed0, processed1);

            if(processed0 == false) {
                self.prev_position = self.prev_position + delta0;     
                self.next_position = self.prev_position + delta1;   
            }
            if(processed1== false) {
               
            }          
         
            let progress = (target_timestamp - t0) as f32 / (t1 - t0) as f32;

            let current_position = self.prev_position.lerp(self.next_position, progress);

            println!("Moved to  {:?} from  {:?} -> {:?} progress {:?}",current_position, self.prev_position , self.next_position, progress);

            return Some(current_position);
        }


        if let (Some((delta0, t0, processed0)), None) = (previous, next) {
            println!("llegamos al final., no hay nada {:?}", delta0);
        }

        None

 
    }

    pub fn clean_buffer(&mut self,  game_timestamp: u128) -> Option<Vec3>  {
        // Remove old states beyond the buffer duration
        while let Some((delta, oldest_timestamp, processed)) = self.buffer.front() {
            if game_timestamp > *oldest_timestamp && game_timestamp - oldest_timestamp > 400 {

                // Ya fue procesado, se elimina.
                if(*processed == true) {
                    println!("Ya fue procesado, se elimina {:?}", oldest_timestamp);
                    self.buffer.pop_front();                   
                }
                else {
                    println!("No fue procesad!!!!!!!!!!!!!!!!!!!!!!!!!!!!, se elimina pero se actualiza translation{:?}", oldest_timestamp);     
                    let delta_vec3 = delta.as_vec3().mul(TRANSLATION_PRECISION);                
                    self.buffer.pop_front();                   
                    return Some(delta_vec3);
                }
               
            } else {
                break;
            }
        }
        None
        
    }
}



pub struct InterpolationPlugin;

impl Plugin for InterpolationPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app          
            .add_systems(
                FixedUpdate, (
                    interpolate_positions_with_deltas.run_if(in_state(AppState::InGame)),
                    clean_buffer.run_if(in_state(AppState::InGame)),
                )
            );
        
        fn clean_buffer(  
            client_time: Res<Time>,
            clock_offset: Res<ClockOffset>,
            mut query: Query<(&mut PositionHistory, &mut Transform)>,
        ) {

            if( clock_offset.0 == 0 || client_time.elapsed().as_millis() + clock_offset.0 < INTERPLOATE_BUFFER) {
                return;
            }

            let target_time =  client_time.elapsed().as_millis() + clock_offset.0 - INTERPLOATE_BUFFER; 


            for (mut history, mut transform) in query.iter_mut() {
            
                if let Some(delta) = history.clean_buffer(target_time) {
                    println!("Se cambia el transform porque llegó tarde un paquete y no se procesó. {:?} ", delta);
                    transform.translation += delta;
                    continue;
                }
            }
        }

        fn interpolate_positions_with_deltas(
            server_time_res: Res<ServerTime>,
            client_time: Res<Time>,
            clock_offset: Res<ClockOffset>,
            mut query: Query<(&mut PositionHistory, &mut Transform)>,
        ) {

            if( server_time_res.0 == 0 || client_time.elapsed().as_millis() + clock_offset.0 < INTERPLOATE_BUFFER) {
                    println!("Aún no se define la hora del servidor.  {:?} ", server_time_res.0 );
                return;
            }

            let target_time =  client_time.elapsed().as_millis() + clock_offset.0 - INTERPLOATE_BUFFER; 

            for (mut history, mut transform) in query.iter_mut() {
            
                if let Some(interpolated_position) = history.interpolate_delta_positions(target_time) {
                    println!("Se cambia el transform {:?} ", interpolated_position);
                    transform.translation = interpolated_position;
                    continue;
                }
            }
        }
        

    }

    
}