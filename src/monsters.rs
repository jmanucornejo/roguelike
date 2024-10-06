use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use pathing::*;
use crate::*;
use rand::prelude::*;
use bevy_sprite3d::*;
use pathfinding::prelude::{astar, bfs};
use std::ops::Div;
pub struct MonstersPlugin;

#[derive(Component)]
pub struct MonsterParent; 



#[derive(AssetCollection, Resource, Debug)]
struct TestAssets {
    #[asset(texture_atlas_layout(tile_size_x = 24, tile_size_y = 24, columns = 7, rows = 1, padding_x = 0, padding_y = 0, offset_x = 0, offset_y = 0))]
    layout: Handle<TextureAtlasLayout>,
    #[asset(path = "gabe-idle-run.png")]
    sprite: Handle<Image>,

    
}




#[derive(Event)]
struct SpawnMonster {
    monster: Monster,
    pos: Pos,
}



impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app
            .add_systems(
            Startup, (
                    spawn_monster_parent,
                              
                )            
            )
            .add_plugins(Sprite3dPlugin)
            .add_loading_state(
                LoadingState::new(AppState::Setup)                  
                    .continue_to_state(AppState::InGame)
                    .load_collection::<TestAssets>()
            )
            .add_systems(OnEnter(AppState::InGame), (( setup_map )))
            .add_systems(
                FixedUpdate, (
                    monster_movement_timer_reset.run_if(in_state(AppState::InGame)),
                    monster_movement.run_if(in_state(AppState::InGame)),
                )
            )
            
            .observe(
                |trigger: Trigger<SpawnMonster>,
                parent: Query<Entity, With<MonsterParent>>,               
                assets            : Res<TestAssets>,
                mut sprite_params : Sprite3dParams,
                mut commands: Commands| {
                    // You can access the trigger data via the `Observer`
                    let monster_spawner = trigger.event();
             
                    let parent = parent.single();   

                    //let texture = asset_server.load("pig.png");

                    let transform = Transform::from_xyz(monster_spawner.pos.0 as f32, 2.0, monster_spawner.pos.1 as f32);  

                    let texture_atlas = TextureAtlas {
                        layout: assets.layout.clone(),
                        index: 3,
                    };                    

                    commands.entity(parent).with_children(|commands| {
                        commands.spawn((
                            Sprite3d {
                                image: assets.sprite.clone(),
                                pixels_per_metre: 32.,
                                alpha_mode: AlphaMode::Blend,
                                unlit: true,
                                transform: transform,
                                // pivot: Some(Vec2::new(0.5, 0.5)),
                
                                ..default()
                            }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()),    
                            monster_spawner.monster.clone(),
                            Name::new("Pig")
                            )
                        )
                        .insert(Velocity::default())
                        .insert(Facing(0))
                        .insert(PrevState { translation: transform.translation, rotation: Facing(0)})
                        .insert(NearestNeighbourComponent)
                        .insert(TargetPos { position: transform.translation.into() });       
                    });

                    /*
                    let message = ServerMessages::SpawnProjectile {
                        entity: monster_entity.id(),
                        translation: transform.translation.into(),
                    };
                    let message = bincode::serialize(&message).unwrap();
                    server.broadcast_message(ServerChannel::ServerMessages, message);*/
     
                },
            );


        fn spawn_monster_parent(mut commands: Commands) {
            commands.spawn((SpatialBundle::default(), MonsterParent, Name::new("Pig Parent")));
        }

        fn setup_map(
            mut commands: Commands,
            map: ResMut<Map>
        ) {          


            for _i in 1..40 {
             
                let pos = Pos(fastrand::i32(-20..20),fastrand::i32(-20..20));
                
                if !map.blocked_paths.contains(&pos) {
                    /*commands.trigger(SpawnMonster { monster: Monster {   
                        hp: 100,
                        kind: MonsterKind::Pig,
                        move_destination: Vec3 { x: pos.0 as f32, y: 2.0, z: pos.1 as f32 },
                        speed: 5.0,
                        move_timer: Timer::from_seconds(fastrand::i32(5..10) as f32, TimerMode::Once)
                    }, pos: pos });       */ 
                }                
            }        
    
        }     

        fn monster_movement_timer_reset(
            mut query: Query<(&mut Monster, &Transform)>,
            time: Res<Time>
        ) {
            for (mut monster, transform) in &mut query {
                //let (position_old, position, mut transform) = query.single_mut();
                
                monster.move_timer.tick(time.delta());

                if monster.move_timer.finished() {
                 
                    monster.move_destination =  Vec3 { 
                        x: transform.translation.x.round() + fastrand::i32(-10..10) as f32, 
                        y: 2.0, 
                        z: transform.translation.z.round() + fastrand::i32(-10..10) as f32
                    };                

                    //info!("Se acabó timer. Se mueve monstruo a {:?}", monster.move_destination);

                    monster.move_timer = Timer::from_seconds(fastrand::i32(5..10) as f32, TimerMode::Once);
                   
                }            
           
            }
        }


        fn monster_movement(
            mut query: Query<(&mut TargetPos, &mut Monster, &mut Transform)>,
            map: Res<Map>
        ) {
            for (mut target_pos, mut monster, mut transform) in &mut query {
                //let (position_old, position, mut transform) = query.single_mut();
                            
                //info!("Pig sold for $15! Current Money: ${:?}", target_pos);
            
                let goal: Pos = Pos(
                    monster.move_destination.x as i32, 
                    monster.move_destination.z as i32
                );    
   

                let target = get_next_step(transform.translation.into(), goal, &map); 

                if let Some(final_pos) = target {    
                    target_pos.position =  final_pos;        
                }
               

                /*if((monster.move_destination.x != transform.translation.x || monster.move_destination .z != transform.translation.z) && !map.blocked_paths.contains(&goal)) {                     
    
                    info!("Start   {:?}!  Goal  {:?}!", start,goal);

                    //let succesors = get_succesors(&start, &map);                        
                    let astar_result = astar(
                        &start,
                        |p|  get_astar_successors(p, &map),
                        |p| ((p.0 - goal.0).abs() + (p.1 - goal.1).abs()) as u32,
                        |p| *p==goal);


                    info!("*Star Result {:?}! ",astar_result);    

                
                    if let Some(result) = astar_result{
                        let steps_vec = result.0;
                        let steps_left =  result.1;
                        let mut index = 1;
                        if(steps_left == 0) {
                            index = 0;
                        }
                    
    
                        if let Some(final_pos) = steps_vec.get(index) {
                        
                            let &Pos(x, z) = final_pos;

                            target_pos.position =  Vec3 { x: x as f32, y: 2.0, z: z as f32};
     
                        }
                        
                    }        
                    
                }*/

            }
        }


    }   
}