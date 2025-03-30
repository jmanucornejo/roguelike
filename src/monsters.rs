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

#[derive(Debug, PartialEq, Component, Clone)]
pub struct MonsterMovement {
    pub move_timer: Timer,
    pub speed: f32,
}



#[derive(Event)]
struct SpawnMonster {
    monster: Monster,
    pos: Pos,
    monster_movement: MonsterMovement
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
            /* DESCOMENTAR pAR  Q SE MUEVAN LOS MONSTRUOS 
            .add_systems(
                FixedUpdate, (
                    monster_movement_timer_reset.run_if(in_state(AppState::InGame)),
                )
            )*/
            
            .add_observer(
                |trigger: Trigger<SpawnMonster>,
                parent: Query<Entity, With<MonsterParent>>,               
                assets            : Res<TestAssets>,
                mut sprite_params : Sprite3dParams,
                mut commands: Commands| {
                    // You can access the trigger data via the `Observer`
                    let monster_spawner = trigger.event();
             
                    let parent = parent.single();   

                    //let texture = asset_server.load("pig.png");

                    let transform = Transform::from_xyz(monster_spawner.pos.0 as f32, 0.0, monster_spawner.pos.1 as f32);  

                    let texture_atlas = TextureAtlas {
                        layout: assets.layout.clone(),
                        index: 3,
                    };                    

                    commands.entity(parent).with_children(|commands| {
                        commands.spawn((

                            transform,
                            Sprite3dBuilder {
                                image: assets.sprite.clone(),
                                pixels_per_metre: 32.,
                                alpha_mode: AlphaMode::Blend,
                                unlit: true,
                                // pivot: Some(Vec2::new(0.5, 0.5)),
                
                                ..default()
                            }.bundle_with_atlas(&mut sprite_params,texture_atlas.clone()),    
                            monster_spawner.monster.clone(),
                            monster_spawner.monster_movement.clone(),
                            Name::new("Pig"),
                            KinematicCharacterController {
                                offset: CharacterLength::Absolute(0.3),
                                filter_flags: QueryFilterFlags::EXCLUDE_KINEMATIC,  
                                //snap_to_ground: Some(CharacterLength::Absolute(1.)),
                                ..KinematicCharacterController::default()
                            },
                            Collider::capsule_y(0.5, 0.5),
                            /*CollisionGroups::new(
                                Group::GROUP_1,
                                Group::GROUP_2,
                            ),*/
                            RigidBody::KinematicPositionBased,
                            //Collider::capsule(0.4, 1.0),
                            )
                        )
                        .insert(GameVelocity::default())
                        .insert(Facing(0))
                        .insert(SpriteId(1))
                        .insert(PrevState { translation: transform.translation, rotation: Facing(0)})
                        .insert(NearestNeighbourComponent)
                        //.insert(SeenBy::default())
                        .insert(Health { max: 100, current: 100})
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
                    commands.trigger(SpawnMonster { 
                        monster: Monster {   
                            hp: 100,
                            kind: MonsterKind::Pig, 
                        }, 
                        monster_movement: MonsterMovement {                           
                            move_timer: Timer::from_seconds(fastrand::i32(5..10) as f32, TimerMode::Once),
                            speed: 5.0    
                        },
                        pos: pos 
                    });       
                }                
            }        
    
        }     

        fn monster_movement_timer_reset(
            mut query: Query<(Entity, &mut MonsterMovement, &Transform), With<Monster>>,
            time: Res<Time>,
            mut commands: Commands,
            map: Res<Map>
        ) {
            for (mut monster, mut movement, transform) in &mut query {
                //let (position_old, position, mut transform) = query.single_mut();
                
                movement.move_timer.tick(time.delta());

                if movement.move_timer.finished() {
                 
                    let move_destination =  Vec3 { 
                        x: transform.translation.x.round() + fastrand::i32(-10..10) as f32, 
                        y: 2.0, 
                        z: transform.translation.z.round() + fastrand::i32(-10..10) as f32
                    };                
                    

                    //info!("Se acabó timer. Se mueve monstruo a {:?}", monster.move_destination);

                    movement.move_timer = Timer::from_seconds(fastrand::i32(5..10) as f32, TimerMode::Once);

                    commands.entity(monster).insert(Walking {
                        target_translation: move_destination,
                        path: get_path_between_translations(transform.translation, move_destination, &map),                               
                    }); 
                   
                }            
           
            }
        }

    }   
}