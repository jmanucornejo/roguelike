
use bevy::prelude::*;
use bevy_health_bar3d::prelude::BarHeight;
use bevy_health_bar3d::prelude::BarSettings;
use crate::*;
use bevy_health_bar3d::prelude::{ColorScheme, ForegroundColor, HealthBarPlugin, Percentage};
use bevy::color::palettes::basic::*;
use bevy::color::palettes::css::*;


impl Percentage for Health {
    fn value(&self) -> f32 {
        self.current as f32 / self.max as f32
    }
}

impl Percentage for Mana {
    fn value(&self) -> f32 {
        self.current as f32 / self.max as f32
    }
}

pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app  
            .register_type::<Mana>()
            .register_type::<Health>()
            .add_plugins((
                HealthBarPlugin::<Health>::default(), 
                HealthBarPlugin::<Mana>::default())
            )
            // set a different color for the Mana bar
            .insert_resource(ColorScheme::<Mana>::new().foreground_color(ForegroundColor::Static(BLUE.into())))
            .insert_resource(ColorScheme::<Mana>::new().foreground_color(ForegroundColor::Static(BLUE.into())))
            .add_systems(
                FixedUpdate, (
                    show_monster_health.run_if(in_state(AppState::InGame)),
                )
            );

     
           
        fn show_monster_health(  
            mut query: Query<(Entity, &mut Health, &mut BarSettings<Health>), Changed<Health>>
        ) {

            for (entity, mut health, mut bar_settings) in query.iter_mut() {
                //println!("Se detectó cambio de  HP {:?}, {:?} ", health, entity);

                if(health.max == health.current) {
                    continue;
                }      
                bar_settings.offset = -1.55;
                bar_settings.width = 1.2;
                bar_settings.height = BarHeight::Static(0.10);
               
            }
        }
    }

  
}
