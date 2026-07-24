use super::pathing::*;
use super::spatial::NearestNeighbourComponent;
use crate::server::network::replication::PrevState;
use crate::shared::gameplay::components::*;
use crate::shared::gameplay::entities::AttackSpeed;
use crate::shared::states::ServerState;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy_sprite3d::*;
use pathfinding::prelude::{astar, bfs};
use rand::prelude::*;
use std::ops::Div;

pub struct MonstersPlugin;

#[derive(Component)]
pub struct MonsterParent;

#[derive(AssetCollection, Resource, Debug)]
struct TestAssets {
    #[asset(texture_atlas_layout(
        tile_size_x = 24,
        tile_size_y = 24,
        columns = 7,
        rows = 1,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
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
    monster_movement: MonsterMovement,
}

impl Plugin for MonstersPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.add_systems(OnEnter(ServerState::Initializing), spawn_monster_parent)
            .add_plugins(Sprite3dPlugin)
            .add_loading_state(
                LoadingState::new(ServerState::Initializing)
                    .load_collection::<TestAssets>()
                    .continue_to_state(ServerState::InGame),
            )
            .add_systems(OnExit(ServerState::Initializing), (setup_map))
            // DESCOMENTAR pAR  Q SE MUEVAN LOS MONSTRUOS
            /*.add_systems(
                FixedUpdate, (
                    monster_movement_timer_reset.run_if(in_state(ServerState::InGame)),
                )
            )*/
            .add_observer(
                |trigger: On<SpawnMonster>,
                 parent: Query<Entity, With<MonsterParent>>,
                 assets: Res<TestAssets>,
                 mut commands: Commands| {
                    // You can access the trigger data via the `Observer`
                    let monster_spawner = trigger.event();

                    //let texture = asset_server.load("pig.png");

                    let transform = Transform::from_xyz(
                        monster_spawner.pos.0 as f32,
                        0.0,
                        monster_spawner.pos.1 as f32,
                    );

                    let texture_atlas = TextureAtlas {
                        layout: assets.layout.clone(),
                        index: 3,
                    };

                    let mut monster_commands = commands.spawn((
                        transform,
                        Sprite3d {
                            pixels_per_metre: 32.,
                            alpha_mode: AlphaMode::Blend,
                            unlit: true,
                            ..default()
                        },
                        Sprite {
                            image: assets.sprite.clone(),
                            texture_atlas: Some(texture_atlas.clone()),
                            ..default()
                        },
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
                        GameVelocity::default(),
                        Facing(0),
                        SpriteId(1),
                        PrevState {
                            translation: transform.translation,
                            rotation: Facing(0),
                        },
                        NearestNeighbourComponent,
                        TargetPos {
                            position: transform.translation.into(),
                        },
                    ));
                    monster_commands.insert((
                        Health {
                            max: 100,
                            current: 100,
                        },
                        AttackSpeed(0.5),
                    ));

                    if let Ok(parent) = parent.single() {
                        monster_commands.insert(ChildOf(parent));
                    }

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
            commands.spawn((
                Transform::default(),
                Visibility::default(),
                MonsterParent,
                Name::new("Pig Parent"),
            ));
        }

        fn setup_map(mut commands: Commands, map: ResMut<Map>) {
            println!("Spawning monsters");
            for _i in 1..40 {
                let pos = Pos(fastrand::i32(-20..20), fastrand::i32(-20..20));

                if !map.blocked_paths.contains(&pos) {
                    commands.trigger(SpawnMonster {
                        monster: Monster {
                            hp: 100,
                            kind: MonsterKind::Pig,
                        },
                        monster_movement: MonsterMovement {
                            move_timer: Timer::from_seconds(
                                fastrand::i32(5..10) as f32,
                                TimerMode::Once,
                            ),
                            speed: 5.0,
                        },
                        pos: pos,
                    });
                }
            }
        }

        fn monster_movement_timer_reset(
            mut query: Query<(Entity, &mut MonsterMovement, &Transform), With<Monster>>,
            time: Res<Time>,
            mut commands: Commands,
            map: Res<Map>,
        ) {
            for (mut monster, mut movement, transform) in &mut query {
                //let (position_old, position, mut transform) = query.single_mut();

                movement.move_timer.tick(time.delta());

                if movement.move_timer.is_finished() {
                    let move_destination = Vec3 {
                        x: transform.translation.x.round() + fastrand::i32(-10..10) as f32,
                        y: 2.0,
                        z: transform.translation.z.round() + fastrand::i32(-10..10) as f32,
                    };

                    //info!("Se acabó timer. Se mueve monstruo a {:?}", monster.move_destination);

                    movement.move_timer =
                        Timer::from_seconds(fastrand::i32(5..10) as f32, TimerMode::Once);

                    commands.entity(monster).insert(Walking {
                        target_translation: move_destination,
                        path: get_path_between_translations(
                            transform.translation,
                            move_destination,
                            &map,
                        ),
                    });
                }
            }
        }
    }
}
