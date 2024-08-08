use bevy::prelude::*;
use crate::{Money, Player};
use rand::prelude::*;
pub struct PigPlugin;

impl Plugin for PigPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, spawn_pig_parent)
            .add_systems(Update, (spawn_pig, pig_lifetime, pig_movement))
            .register_type::<Pig>();
    }
}

#[derive(Component)]
pub struct PigParent; 

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Pig {
    pub lifetime: Timer,
    pub speed: f32,
    pub direction: i32
}

fn spawn_pig_parent(mut commands: Commands) {
    commands.spawn((SpatialBundle::default(), PigParent, Name::new("Pig Parent")));
}

fn spawn_pig(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    input: Res<ButtonInput<KeyCode>>,
    mut money: ResMut<Money>,
    player: Query<&Transform, With<Player>>,
    parent: Query<Entity, With<PigParent>>
) {
    if !input.just_pressed(KeyCode::Space) {
        return;
    }

    let player_transform = player.single();
    let parent = parent.single();

    if money.0 >= 10.0 {
        money.0 -= 10.0;
        info!("Spent $10 on a pig, remaining money: ${:?}", money.0);

        let texture = asset_server.load("pig.png");

        commands.entity(parent).with_children(|commands| {
            commands.spawn((
                SpriteBundle {
                    texture,
                    transform: *player_transform,
                    ..default()
                },
                Pig {
                    lifetime: Timer::from_seconds(2.0, TimerMode::Once),
                    speed: 100.0,
                    direction: rand::thread_rng().gen_range(1..5)
                },
                Name::new("Pig")
            ));
        });
       
    }
}

fn pig_lifetime(
    mut commands: Commands,
    time: Res<Time>,
    mut pigs: Query<(Entity, &mut Pig)>,
    parent: Query<Entity, With<PigParent>>,
    mut money: ResMut<Money>,
) {

    let parent = parent.single();

    for (pig_entity, mut pig) in &mut pigs {
        pig.lifetime.tick(time.delta());

        if pig.lifetime.finished() {
            money.0 += 15.0;

            commands.entity(parent).remove_children(&[pig_entity]);
            commands.entity(pig_entity).despawn();

            info!("Pig sold for $15! Current Money: ${:?}", money.0);
        }
    }
}

fn pig_movement(
    mut pigs: Query<(&mut Transform, &mut Pig)>,
    time: Res<Time>,
) {
    for(mut transform, mut pig) in &mut pigs {

        let mut direction = pig.direction;

        if(pig.lifetime.fraction() >= 0.25 && pig.lifetime.fraction() < 0.26) {
            direction = rand::thread_rng().gen_range(1..5);
            pig.direction = direction;
               
            info!("Current direction: ${:?}", direction);
        }
      
        info!("Pig lifetime: {:?}", pig.lifetime.fraction());
        
        //info!("Current direction: ${:?}", direction);

        let movement_amount = pig.speed * time.delta_seconds();        
       

        match direction {
            1 => {
                transform.translation.y += movement_amount; 
            },
            2 =>  {
                transform.translation.y -= movement_amount;    
            },
            3 => {
                transform.translation.x += movement_amount; 
            },
            4 => {
                transform.translation.x -= movement_amount;
            },
            _ => {

            }
        }       
       

     
        

    }
}
