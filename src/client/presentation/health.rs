use crate::client::presentation::health_bars::{
    BarHeight, BarSettings, ColorScheme, ForegroundColor, HealthBarPlugin, Percentage,
};
use crate::shared::gameplay::components::*;
use crate::shared::states::ClientState;
use bevy::color::palettes::basic::*;
use bevy::prelude::*;

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

fn monster_health_should_be_visible(health: &Health) -> bool {
    health.current < health.max
}

pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.register_type::<Mana>()
            .register_type::<Health>()
            .add_plugins((
                HealthBarPlugin::<Health>::default(),
                HealthBarPlugin::<Mana>::default(),
            ))
            // set a different color for the Mana bar
            .insert_resource(
                ColorScheme::<Mana>::new().foreground_color(ForegroundColor::Static(BLUE.into())),
            )
            .add_systems(
                Update,
                show_monster_health.run_if(in_state(ClientState::InGame)),
            );

        fn show_monster_health(
            mut commands: Commands,
            query: Query<
                (Entity, &Health),
                (With<Monster>, Without<BarSettings<Health>>, Changed<Health>),
            >,
        ) {
            for (entity, health) in &query {
                if !monster_health_should_be_visible(health) {
                    continue;
                }

                commands.entity(entity).insert(BarSettings::<Health> {
                    offset: -1.15,
                    width: 1.0,
                    height: BarHeight::Static(0.06),
                    foreground_color: Some(Color::srgb(0.9, 0.08, 0.08)),
                    ..default()
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_health_monsters_do_not_show_a_bar() {
        assert!(!monster_health_should_be_visible(&Health {
            max: 100,
            current: 100,
        }));
    }

    #[test]
    fn a_monster_damaged_by_any_source_shows_a_bar() {
        assert!(monster_health_should_be_visible(&Health {
            max: 100,
            current: 99,
        }));
    }
}
