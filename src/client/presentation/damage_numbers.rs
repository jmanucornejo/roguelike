use bevy::prelude::*;
use bevy::ui::UiSystems;

use crate::shared::gameplay::components::Monster;
use crate::shared::states::ClientState;

const DAMAGE_NUMBER_LIFETIME: f32 = 0.9;
const DAMAGE_NUMBER_WORLD_HEIGHT: f32 = 1.35;
const DAMAGE_NUMBER_RISE_PIXELS: f32 = 58.0;
const DAMAGE_NUMBER_WIDTH: f32 = 96.0;
const DAMAGE_NUMBER_HEIGHT: f32 = 44.0;
const DAMAGE_NUMBER_FONT_SIZE: f32 = 30.0;
const DAMAGE_NUMBER_SPREAD: [f32; 5] = [0.0, -14.0, 13.0, -7.0, 8.0];

#[derive(Event)]
pub(crate) struct DamageNumberEvent {
    pub(crate) entity: Entity,
    pub(crate) amount: i32,
}

#[derive(Component)]
struct FloatingDamageNumber {
    owner: Entity,
    world_anchor: Vec3,
    elapsed: f32,
    horizontal_offset: f32,
}

#[derive(Resource, Default)]
struct DamageNumberSequence(usize);

pub(crate) struct DamageNumbersPlugin;

impl Plugin for DamageNumbersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DamageNumberSequence>()
            .add_observer(spawn_damage_number)
            .add_systems(
                PostUpdate,
                update_damage_numbers
                    .run_if(in_state(ClientState::InGame))
                    .before(UiSystems::Prepare),
            );
    }
}

fn spawn_damage_number(
    trigger: On<DamageNumberEvent>,
    mut commands: Commands,
    monsters: Query<&GlobalTransform, With<Monster>>,
    mut sequence: ResMut<DamageNumberSequence>,
) {
    let event = trigger.event();
    let Ok(monster_transform) = monsters.get(event.entity) else {
        return;
    };
    if event.amount <= 0 {
        return;
    }

    let horizontal_offset = DAMAGE_NUMBER_SPREAD[sequence.0 % DAMAGE_NUMBER_SPREAD.len()];
    sequence.0 = sequence.0.wrapping_add(1);

    commands.spawn((
        Text::new(event.amount.to_string()),
        TextFont {
            font_size: FontSize::Px(DAMAGE_NUMBER_FONT_SIZE),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.88, 0.32)),
        TextShadow {
            offset: Vec2::new(2.0, 2.0),
            color: Color::srgba(0.08, 0.01, 0.01, 0.95),
        },
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(DAMAGE_NUMBER_WIDTH),
            height: Val::Px(DAMAGE_NUMBER_HEIGHT),
            ..default()
        },
        GlobalZIndex(200),
        Pickable::IGNORE,
        FloatingDamageNumber {
            owner: event.entity,
            world_anchor: monster_transform.translation() + Vec3::Y * DAMAGE_NUMBER_WORLD_HEIGHT,
            elapsed: 0.0,
            horizontal_offset,
        },
    ));
}

fn damage_number_animation(progress: f32) -> (f32, f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    let rise = DAMAGE_NUMBER_RISE_PIXELS * (1.0 - (1.0 - progress).powi(2));

    let scale = if progress < 0.12 {
        0.65 + (progress / 0.12) * 0.55
    } else if progress < 0.25 {
        1.2 - ((progress - 0.12) / 0.13) * 0.2
    } else {
        1.0
    };

    let alpha = if progress < 0.65 {
        1.0
    } else {
        1.0 - (progress - 0.65) / 0.35
    };

    (rise, scale, alpha.clamp(0.0, 1.0))
}

fn update_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    owners: Query<&GlobalTransform, Without<FloatingDamageNumber>>,
    mut numbers: Query<(
        Entity,
        &mut FloatingDamageNumber,
        &mut Node,
        &mut TextFont,
        &mut TextColor,
        &mut TextShadow,
    )>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let viewport_size = camera.logical_viewport_size();

    for (entity, mut number, mut node, mut font, mut color, mut shadow) in &mut numbers {
        number.elapsed += time.delta_secs();
        let progress = number.elapsed / DAMAGE_NUMBER_LIFETIME;

        if progress >= 1.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        if let Ok(owner_transform) = owners.get(number.owner) {
            number.world_anchor =
                owner_transform.translation() + Vec3::Y * DAMAGE_NUMBER_WORLD_HEIGHT;
        }

        let Ok(viewport_position) = camera.world_to_viewport(camera_transform, number.world_anchor)
        else {
            node.display = Display::None;
            continue;
        };

        let (rise, scale, alpha) = damage_number_animation(progress);
        let on_screen = viewport_size.is_some_and(|viewport_size| {
            viewport_position.x >= -DAMAGE_NUMBER_WIDTH
                && viewport_position.x <= viewport_size.x + DAMAGE_NUMBER_WIDTH
                && viewport_position.y >= -DAMAGE_NUMBER_HEIGHT
                && viewport_position.y <= viewport_size.y + DAMAGE_NUMBER_HEIGHT + rise
        });

        node.display = if on_screen {
            Display::Flex
        } else {
            Display::None
        };
        node.left =
            Val::Px(viewport_position.x - DAMAGE_NUMBER_WIDTH * 0.5 + number.horizontal_offset);
        node.top = Val::Px(viewport_position.y - DAMAGE_NUMBER_HEIGHT * 0.5 - rise);

        font.font_size = FontSize::Px(DAMAGE_NUMBER_FONT_SIZE * scale);
        color.0.set_alpha(alpha);
        shadow.color.set_alpha(alpha * 0.95);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_number_pops_then_fades() {
        let (_, initial_scale, initial_alpha) = damage_number_animation(0.0);
        let (middle_rise, middle_scale, middle_alpha) = damage_number_animation(0.5);
        let (final_rise, _, final_alpha) = damage_number_animation(1.0);

        assert!(initial_scale < 1.0);
        assert!((middle_scale - 1.0).abs() < f32::EPSILON);
        assert_eq!(initial_alpha, 1.0);
        assert_eq!(middle_alpha, 1.0);
        assert_eq!(final_alpha, 0.0);
        assert!(final_rise > middle_rise);
    }
}
