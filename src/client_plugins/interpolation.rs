
use bevy::prelude::*;
use crate::*;
use std::collections::VecDeque;
use std::ops::Mul;
use std::ops::Div;
use client_plugins::shared::*;
use shared::constants::*;
use shared::components::*;
use shared::states::ClientState;

#[cfg(not(feature = "absolute_interpolation"))]
#[derive(Component, Debug)]
pub struct PositionHistory {
    buffer: VecDeque<(IVec3, u128, bool)>, // (timestamp, delta position, processed)
    prev_position: Vec3, 
    next_position: Vec3
}

#[cfg(feature = "absolute_interpolation")]
#[derive(Component, Debug)]
pub struct PositionHistory {
    buffer: VecDeque<PositionSnapshot>, // (
    last_position: PositionSnapshot
}

/// A single position update from the server
#[derive(Debug, Clone)]
pub struct PositionSnapshot {
    pub position: Vec3,
    pub timestamp: u128, // In seconds (f64 for precision)
}

impl PositionSnapshot  {

    #[cfg(feature = "absolute_interpolation")]
    pub fn new(position: Vec3, timestamp:u128) -> Self {
        Self {           
            position: position,
            timestamp: timestamp
        }
    }
    
}

impl PositionHistory  {

    #[cfg(not(feature = "absolute_interpolation"))]
    pub fn new(position: Vec3) -> Self {
        Self {
            buffer: VecDeque::new(),
            prev_position: position,
            next_position: position
        }
    }

    #[cfg(feature = "absolute_interpolation")]
    pub fn new(position: Vec3, timestamp: u128) -> Self {
        Self {
            buffer: VecDeque::new(),
            last_position: PositionSnapshot::new(position, timestamp),
        }
    }

    #[cfg(not(feature = "absolute_interpolation"))]
    pub fn add_delta(&mut self,  delta_position: IVec3, timestamp: u128) {
        self.buffer.push_back((delta_position, timestamp, false));     
    }

    #[cfg(feature = "absolute_interpolation")]
    pub fn add_absolute_position(&mut self,  absolute_position: Vec3, timestamp: u128) {
        self.buffer.push_back(PositionSnapshot { position: absolute_position, timestamp: timestamp } );     
    }

    /* 
    #[cfg(feature = "absolute_interpolation")]
    pub fn interpolate_absolute_positions(&mut self, render_time: u128) -> Option<Vec3> {

        println!("buffer {:?}", self.buffer);
        if self.buffer.len() < 2 {
            return None;
        }

         // Find two states to interpolate between
         let mut previous = None;
         let mut next = None;
       
    
        for i in 0..self.buffer.len() - 1 {
            let (pos0, t0) = self.buffer[i];
            let (pos1, t1) = self.buffer[i + 1];
            if t0 <= render_time && render_time <= t1 {
                previous = Some((pos0.as_vec3().mul(TRANSLATION_PRECISION),t0));
                next = Some((pos1.as_vec3().mul(TRANSLATION_PRECISION), t1));                
                break; // se encontro la posición entre dos
            }
            else if(t1 <= render_time && i+1 == self.buffer.len() - 1 && self.prev_position != self.next_position) {
                self.prev_position = self.next_position;
                //println!("Se procesa fin de la cola {:?}, position {:?}", t1, self.next_position);
                
                return Some(self.next_position);
            }
        }

        // Perform interpolation based on the deltas
        if let (Some((pos0, t0)), Some((pos1, t1))) = (previous, next) {
            //println!("pos0 {:?}, pos1 {:?}, t0 {:?}, t1 {:?},processed0 {:?}, processed1 {:?}", pos0, pos1, t0, t1, processed0, processed1);

            let progress = (render_time - t0) as f32 / (t1 - t0) as f32;

            //println!("NO SE HA PROCESADO LA POSICION PREVIA ");
            self.prev_position = pos0;     
            self.next_position = pos1;   
           
             
            
            
            let current_position = self.prev_position.lerp(self.next_position, progress);

            //println!("Moved to  {:?} from  {:?} -> {:?} progress {:?} time {:?}",current_position, self.prev_position , self.next_position, progress, render_time);

            return Some(current_position);
        }


    
        None

 
    }*/


    /*
    #[cfg(feature = "absolute_interpolation")]
    pub fn interpolate_absolute_positions2(&mut self, render_time: u128) -> Option<Vec3> {

        println!("buffer {:?}", self.buffer);
        if self.buffer.len() < 2 {
            return self.buffer.front().map(|(pos, _)| pos.as_vec3().mul(TRANSLATION_PRECISION));
        }
    
        for i in 0..self.buffer.len() - 1 {
            let (pos0, t0) = self.buffer[i];
            let (pos1, t1) = self.buffer[i + 1];
    
            // Interpolate if render_time is between t0 and t1 (inclusive)
            if t0 <= render_time && render_time <= t1 {
                let v0 = pos0.as_vec3().mul(TRANSLATION_PRECISION);
                let v1 = pos1.as_vec3().mul(TRANSLATION_PRECISION);
                let progress = (render_time - t0) as f32 / (t1 - t0) as f32;
                return Some(v0.lerp(v1, progress));
            }
        }
    
        // If render_time is after the last known timestamp, return the last position
        if let Some((last_pos, _)) = self.buffer.back() {
            return Some(last_pos.as_vec3().mul(TRANSLATION_PRECISION));
        }
    
        None

 
    }
    */

    #[cfg(not(feature = "absolute_interpolation"))]
    pub fn interpolate_delta_positions(&mut self, render_time: u128) -> Option<Vec3> {
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
            if t0 <= render_time && render_time <= t1 {
                previous = Some((delta0.as_vec3().mul(TRANSLATION_PRECISION),t0,processed0));
                next = Some((delta1.as_vec3().mul(TRANSLATION_PRECISION), t1, processed1));
                
                // Se cambia processed a true;
                self.buffer[i] = (delta0, t0, true);
                break;
            }
            else if(t1 <= render_time && i+1 == self.buffer.len() - 1 && self.prev_position != self.next_position) {
                self.prev_position = self.next_position;
                self.buffer[i + 1] = (delta1, t1, true);
                //println!("Se procesa fin de la cola {:?}, position {:?}", t1, self.next_position);
                
                return Some(self.next_position);
            }
        }
        
        // Perform interpolation based on the deltas
        if let (Some((delta0, t0, processed0)), Some((delta1, t1, processed1))) = (previous, next) {
            //println!("delta0 {:?}, delta1 {:?}, t0 {:?}, t1 {:?},processed0 {:?}, processed1 {:?}", delta0, delta1, t0, t1, processed0, processed1);

            let progress = (render_time - t0) as f32 / (t1 - t0) as f32;


            if processed0 == false {
                //println!("NO SE HA PROCESADO LA POSICION PREVIA ");
                self.prev_position = self.prev_position + delta0;     
                self.next_position = self.prev_position + delta1;   
            }
            if processed1== false  {
               
            }          
            
            
            let current_position = self.prev_position.lerp(self.next_position, progress);

            //println!("Moved to  {:?} from  {:?} -> {:?} progress {:?} time {:?}",current_position, self.prev_position , self.next_position, progress, render_time);

            return Some(current_position);
        }

        None

 
    }

    #[cfg(not(feature = "absolute_interpolation"))]
    pub fn clean_delta_buffer(&mut self,  render_time: u128) -> Option<Vec3>  {
        // Remove old states beyond the buffer duration
        while let Some((delta, oldest_timestamp, processed)) = self.buffer.front() {
            if render_time > *oldest_timestamp && render_time - oldest_timestamp > 500 // Esto es limpieza, no es el interpolation. Es el tiempo q se espera para procesar
            {
                 
                // Ya fue procesado, se elimina.
                if *processed == true {
                    //println!("Ya fue procesado, se elimina {:?}", oldest_timestamp);
                    self.buffer.pop_front();                   
                }
                else {
                    println!("No fue procesad!!!!!!!!!!!!!!, se elimina pero se actualiza translation, time: {:?}, render_time: {:?}", oldest_timestamp, render_time);     
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

    #[cfg(feature = "absolute_interpolation")]
    pub fn clean_absolute_buffer(&mut self,  render_time: u128) {
        // Remove old states beyond the buffer duration
        while let Some(snapshot) = self.buffer.front() {
            if render_time > snapshot.timestamp && render_time - snapshot.timestamp > 500 // Esto es limpieza, no es el interpolation. Es el tiempo q se espera para procesar
            {                 

                self.buffer.pop_front();         
                // Ya fue procesado, se elimina.          
               
            } 
            break;
        }        
        
    }
}



pub struct InterpolationPlugin;

impl Plugin for InterpolationPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app          
            .insert_resource(PrevClock::default());

        #[cfg(not(feature = "absolute_interpolation"))]
        app.add_systems(FixedUpdate,    (
            interpolate_positions_with_deltas.run_if(in_state(ClientState::InGame)),
            clean_delta_buffer.run_if(in_state(ClientState::InGame))
        ));
        
        #[cfg(feature = "absolute_interpolation")]
        app.add_systems(FixedUpdate, (
            interpolate_positions_with_absolutes.run_if(in_state(ClientState::InGame)).after(client_plugins::client_clock_sync::set_render_time),  
            //clean_absolute_buffer.run_if(in_state(ClientState::InGame))
        ));

        #[cfg(feature = "absolute_interpolation")]
        fn clean_absolute_buffer(  
            client_time: Res<Time>,
            clock_offset: Res<ClockOffset>,
            mut query: Query<(&mut PositionHistory, &mut Transform)>
        ) {
            let clock_offset = clock_offset.0;
            // let clock_offset = clock_sync.offset as u128; // version 2.0 testing.

            let estimated_server_time = client_time.elapsed().as_millis() + clock_offset;

            if clock_offset == 0 || estimated_server_time < INTERPOLATE_BUFFER {
                return;
            }

            let render_time =  estimated_server_time - INTERPOLATE_BUFFER; 

          
            for (mut history, mut transform) in query.iter_mut() {

                history.clean_absolute_buffer(render_time);
            }
        }

        #[cfg(not(feature = "absolute_interpolation"))]
        fn clean_delta_buffer(  
            client_time: Res<Time>,
            clock_offset: Res<ClockOffset>,
            mut query: Query<(&mut PositionHistory, &mut Transform)>,
            clock_sync: Res<crate::client_plugins::client_clock_sync::ClockSync>,
        ) {
            let clock_offset = clock_offset.0;
            // let clock_offset = clock_sync.offset as u128; // version 2.0 testing.

            let estimated_server_time = client_time.elapsed().as_millis() + clock_offset;

            if clock_offset == 0 || estimated_server_time < INTERPOLATE_BUFFER {
                return;
            }

            let render_time =  estimated_server_time - INTERPOLATE_BUFFER; 


            for (mut history, mut transform) in query.iter_mut() {
               
                if let Some(delta) = history.clean_delta_buffer(render_time) {
                    println!("Se cambia el transform porque llegó tarde un paquete y no se procesó. {:?} ", delta);
                    transform.translation += delta;
                    continue;
                }
            }
        }

        #[cfg(not(feature = "absolute_interpolation"))]
        fn interpolate_positions_with_deltas(
            render_time: Res<RenderTime>,
            client_time: Res<Time>,
            clock_offset: Res<ClockOffset>,
            clock_sync: Res<crate::client_plugins::client_clock_sync::ClockSync>,
            mut prev_clock: ResMut<PrevClock>,
            mut query: Query<(&mut PositionHistory, &mut Transform, &mut GameVelocity)>,
        ) {

            if render_time.0 == 0 {
                println!("Aún no tenemos un render time de la simulación.  {:?} ", render_time.0 );
                return;
            }
           
            let render_time =  render_time.0; 
          

            for (mut history, mut transform, mut velocity) in query.iter_mut() {
            
                if let Some(interpolated_position) = history.interpolate_delta_positions(render_time) {
                    //println!("prev_clock.0 {:?}", prev_clock.0);

                    if render_time < prev_clock.0 {
                        continue;
                    }
                     
                 
                    let diff = transform.translation - interpolated_position;
                    velocity.0 = diff / (render_time - prev_clock.0) as f32;    
                    
                    prev_clock.0 = render_time;      
                    
                    //let speed = diff.x / (render_time - prev_clock.0) as f32;     
                    //println!("velocidad {:?}, transform {:?}, targettime {:?}", speed, interpolated_position, render_time);
                    transform.translation = interpolated_position;
                    continue;
                }
                else {
                    velocity.0 = Vec3::ZERO;    
                }
            }
        }
        
        #[cfg(feature = "absolute_interpolation")]
        fn interpolate_positions_with_absolutes(   
            client_time: Res<Time>,
            render_time: Res<RenderTime>,
            mut query: Query<(&mut Transform, &mut PositionHistory, &mut GameVelocity)>
        ) {           

            if render_time.0 == 0 {
                println!("Aún no tenemos un render time de la simulación.  {:?} ", render_time.0 );
                return;
            }
            
            let render_time =  render_time.0; 
            
        
            for (mut transform, mut history, mut velocity) in query.iter_mut() {
                // Clean up old snapshots     
             
                while history.buffer.len() >= 2 && history.buffer[1].timestamp < render_time {
                    history.buffer.pop_front();
                }
        
                if history.buffer.len() < 2 {    
                    if let Some(last) = history.buffer.back()  {
                        //println!("last.timestamp  {:?}, history.last_position.timestamp  {:?}", last.timestamp, history.last_position.timestamp );        
                        if last.timestamp >  history.last_position.timestamp && last.timestamp < render_time { // hay una última posición que es mayor al render time.
                            println!("history.buffer  {:?} ", history.buffer);        
                            println!("transform.translation  {:?} ", transform.translation);  
                            println!("history.last_position  {:?} ", history.last_position);  
                            transform.translation = last.position;
                            history.buffer.pop_front();
                        
                        }
                       
                        velocity.0 = Vec3::ZERO;   
                    
                    }
                    continue; // Not enough data to interpolate
                }
        
                let a = &history.buffer[0];
                let b = &history.buffer[1];
        
                let t0 = a.timestamp;
                let t1 = b.timestamp;
                
                //if t0 <= render_time && render_time <= t1 
                if render_time < t0  {
                    continue;
                }
                // println!("Render time:  {:?}, T0: {:?}, T1: {:?} ", render_time, t0, t1);

                let progress = (render_time - t0) as f32 / (t1 - t0) as f32;
                // let progress = ((render_time - t0) / (t1 - t0)).clamp(0.0, 1.0) as f32;

                let interpolated_position = a.position.lerp(b.position, progress);

                let diff = transform.translation - interpolated_position;
                velocity.0 = diff / (render_time as f32 - history.last_position.timestamp as f32);  
        
                transform.translation = interpolated_position;
                history.last_position = PositionSnapshot { position: transform.translation, timestamp: render_time };

              
                
            }
        }

       
    }

    
}