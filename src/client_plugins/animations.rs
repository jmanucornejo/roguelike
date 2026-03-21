use bevy::prelude::*;

use crate::*;
use shared::states::ClientState;
use shared::components::*;
use shared::resources::*;
use shared::events::*;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_sprite3d::*;

pub struct AnimationsPlugin;





impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app           
            .add_systems(
                Update, (               
                    billboard.run_if(in_state(ClientState::InGame)),
                    set_camera_facing.run_if(in_state(ClientState::InGame)),
                    set_entities_facing.run_if(in_state(ClientState::InGame))               
                )
            )
            .add_systems(
                FixedUpdate, (
                    sprite_movement.run_if(in_state(ClientState::InGame)),
                )
            )
            .add_observer(on_death_animate);

     
        

        fn billboard(
            mut camera_query: Query< (&Transform, &PanOrbitCamera),  (With<Camera>, Without<Billboard>, Changed<Transform>) >,
            //mut player_query: Query<&mut Transform, (With<ControlledPlayer>, Without<Monster>)>,
            mut entities_query: Query<&mut Transform, With<Billboard>>
        ) {

        
            if let Ok((camera_transform, pan_cam)) = camera_query.single_mut() {
        
                for mut entity_transform in entities_query.iter_mut() {     
                
                    if let Some(yaw) = pan_cam.yaw {
                        entity_transform.rotation =  Quat::from_rotation_y(yaw);    
                    }
                    
                    //println!("Entity rotation {} camera rotation at translation  {:?}",  entity_transform.rotation, camera_transform.rotation);   
                    //println!("Pitch {:?}", pan_cam.pitch);  
                    if let Some(pitch) = pan_cam.pitch {

                        let pitch_cosine = pitch.clamp(-1.0, 1.0); 
                        let stretch_y = 1.0 / pitch_cosine;

                        //entity_transform.scale.y = 1. + pitch ;   

                    
                        let camera_forward = camera_transform.forward();

                        // Calculate the horizontal forward direction (flattened to ignore Y component)
                        let horizontal_forward = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize();
            
                        // Compute the cosine of the pitch angle between camera_forward and horizontal_forward
                        let pitch_cosine = camera_forward.dot(horizontal_forward) 
                                        / (camera_forward.length() * horizontal_forward.length());
            
                        // Calculate the pitch angle (theta) in radians
                        let pitch_angle = pitch_cosine.acos();
            
                        // Normalize pitch angle to a range of 0 to 1 (0 when horizontal, 1 when vertical)
                        let pitch_ratio = pitch_angle / std::f32::consts::FRAC_PI_2;
            
                        // Smooth stretch: lerp between 1.0 (no stretch) and MAX_STRETCH based on pitch_ratio
                        //let stretch_y = 1.0 + pitch_ratio * (1.5 - 1.0);
                        let max_stretch = 3.5; // Define your max stretch factor
                        let stretch_y: f32 = (1.0 / pitch.cos()).clamp(1.0, max_stretch);
                        //let stretch_y = 1. + pitch;
                        // Apply the stretch to the billboard’s Y scale
                        let k = 1.0; // Adjust this value to control the intensity of the stretch
                        let stretch_y =  1.0 / pitch.cos();
                        entity_transform.scale = Vec3::new(1.0, stretch_y, 1.0);

                    }
                
                }
            }        
        }
      
      

        fn set_camera_facing(
            mut camera_query: Query< (&Transform, &PanOrbitCamera),  (With<Camera>, Changed<Transform>) >,
            mut camera_facing: ResMut<CameraFacing>
        ) {
        
            if let Ok((camera_transform, pan_cam)) = camera_query.single_mut() {
                if let Some(yaw) = pan_cam.yaw {
                
                    let mut rotation = ((8.0 * (yaw.to_degrees()) / 360.0).round() % 8.0) as i32;
                    
                    if rotation < 0 {
                        rotation += 8;
                    }

                    if rotation as u8 != camera_facing.0 {
                        camera_facing.0 = rotation as u8;    
                        println!("camera_facing {:?}", camera_facing.0);
                    }
                    
                }        
            }    
        }

        
        fn set_entities_facing(
            mut query: Query<(&mut Facing, &GameVelocity)>,
        ) {
            for (mut facing, velocity) in query.iter_mut() {  
                
                if velocity.0 == Vec3::ZERO {               
                    continue;
                }             
        
                let x = (velocity.0.x * 1000.0).round() / 1000.0;
                let z = (velocity.0.z * 1000.0).round() / 1000.0;        
                
                // Mirando hacia arriba
                if z > 0. && x == 0.0 {
                    *facing = Facing(0);
                
                }
                // Mirando hacia la arriba a la derecha
                else if z > 0. && x < 0.0 {
                    *facing = Facing(1);
                
                }
                // Mirando hacia la derecha
                else if z == 0. && x < 0.0 {
                    *facing = Facing(2);
                    
                }
                // Mirando hacia la abajo a la derecha
                else if z < 0. && x < 0.0 {
                    *facing = Facing(3);
                
                }
                // Mirando hacia abajo
                else if z < 0. && x == 0.0 {
                    *facing = Facing(4);
                
                }
                // Mirando hacia la abajo a la izquierda
                else if z < 0. && x > 0.0 {
                    *facing = Facing(5);
                
                }
                // Mirando hacia la izquierda
                else if z == 0. && x > 0.0 {
                    *facing = Facing(6);
                    
                }
                
                // Mirando hacia la arriba a la izquierda
                else if z > 0. && x > 0.0 {
                    *facing = Facing(7);
                
                }           
            }
        }

         fn on_death_animate(
            trigger: Trigger<DeathEvent>, 
            mut query: Query<(&Transform)>,
            mut commands: Commands,
        ) {
            // If a triggered event is targeting a specific entity you can access it with `.entity()`
            let death_event = trigger.event();
            let id: Entity = death_event.entity;            

            if let Ok((transform)) = query.get_mut(id) {
             
                info!("Se crea loot en:  {:?} ", transform.translation);
                // Si es jugador, mantenrlo muerto en el piso.
                // Si es monstruo, debe soltar ítems.
        
            }          
    
        }


        

            
        fn sprite_movement(
            time: Res<Time>,
            mut q_parent: Query<(&mut AnimationTimer, &mut Facing, &GameVelocity, &mut Animation)>,
            mut q_child: Query<(&ChildOf, &mut Sprite3d)>,
            camera_rotation: Res<CameraFacing>
        ) {    

        
            for (parent, mut sprite) in q_child.iter_mut() {

                
                if let Ok ((mut timer, facing, velocity, animation)) = q_parent.get_mut(parent.get()) {


                    //println!("Animation {:?}", animation);  
                
                    // Cuando se cambia la rotación, se debe ajustar el sprite.
                    if camera_rotation.is_changed() {

                        if let Some(atlas) = &mut sprite.texture_atlas {

                            let col_index = atlas.index  % 8;
                            println!("col_index {:?}", col_index);  

                            let row_index = camera_rotation.0+facing.0;
                            println!("row_index {:?}", row_index);  
                            atlas.index = col_index + (( row_index * 8) % 64) as usize;
                        }
                    
                    }

                    
                    if velocity.0 == Vec3::ZERO {               
                        continue;
                    }             
            
                

                    let x = (velocity.0.x * 1000.0).round() / 1000.0;
                    let z = (velocity.0.z * 1000.0).round() / 1000.0;
                
                    if z != 0. || x  != 0.0 { 

                        //let row_index = (8 * atlas.index / 64) % 8;

                        timer.tick(time.delta());
                        if timer.just_finished() {

                            let row_index = ((camera_rotation.0+facing.0) % 8) as usize;
                            //let col_index = atlas.index  % 8;
                            
                            //println!("row_index {:?}",row_index);    
                            let starting_row_animation = row_index*8;
                            //println!("starting_row_animation {:?}",starting_row_animation);  
                            let a = (starting_row_animation)..(starting_row_animation + 7);

                            //println!("range {:?}, atlas.index {:?}",a ,atlas.index );  
                            if let Some(atlas) = &mut sprite.texture_atlas {
                                atlas.index = if !a.contains(&atlas.index) || atlas.index == ((row_index*8)+7) {
                                    starting_row_animation
                                }
                                else {
                                    atlas.index + 1
                                };
                            }

                        
                        }

                    }
                    
                }
            }

       
        }
    }
 
}
