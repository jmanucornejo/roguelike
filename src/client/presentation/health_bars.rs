//! Lightweight Bevy 0.19 screen-space health bars.
//!
//! This replaces `bevy_health_bar3d`, whose latest release still targets Bevy
//! 0.18. Bars are projected from their owner's world position into UI space so
//! they remain readable and are not hidden by 3D geometry.

use std::marker::PhantomData;

use bevy::prelude::*;
use bevy::ui::UiSystems;

const SCREEN_PIXELS_PER_WORLD_UNIT: f32 = 60.0;

pub trait Percentage: Component {
    fn value(&self) -> f32;
}

#[derive(Clone, Copy, Debug)]
pub enum BarHeight {
    Static(f32),
}

#[derive(Component)]
pub struct BarSettings<T: Component> {
    pub offset: f32,
    pub width: f32,
    pub height: BarHeight,
    pub foreground_color: Option<Color>,
    pub screen_anchor_offset: Option<f32>,
    pub screen_offset: Vec2,
    #[doc(hidden)]
    pub marker: PhantomData<fn() -> T>,
}

impl<T: Component> Default for BarSettings<T> {
    fn default() -> Self {
        Self {
            offset: 0.0,
            width: 1.0,
            height: BarHeight::Static(0.1),
            foreground_color: None,
            screen_anchor_offset: None,
            screen_offset: Vec2::ZERO,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ForegroundColor {
    Static(Color),
}

#[derive(Resource)]
pub struct ColorScheme<T: Component> {
    foreground: ForegroundColor,
    marker: PhantomData<fn() -> T>,
}

impl<T: Component> Default for ColorScheme<T> {
    fn default() -> Self {
        Self {
            foreground: ForegroundColor::Static(Color::srgb(0.85, 0.05, 0.05)),
            marker: PhantomData,
        }
    }
}

impl<T: Component> ColorScheme<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn foreground_color(mut self, color: ForegroundColor) -> Self {
        self.foreground = color;
        self
    }
}

#[derive(Component)]
struct ScreenBarRoot<T: Component> {
    owner: Entity,
    marker: PhantomData<fn() -> T>,
}

#[derive(Component)]
struct ScreenBarFill<T: Component> {
    owner: Entity,
    marker: PhantomData<fn() -> T>,
}

pub struct HealthBarPlugin<T: Percentage>(PhantomData<fn() -> T>);

impl<T: Percentage> Default for HealthBarPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Percentage> Plugin for HealthBarPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_resource::<ColorScheme<T>>();

        app.add_systems(
            Update,
            (
                spawn_screen_bars::<T>,
                update_screen_bar_fills::<T>.after(spawn_screen_bars::<T>),
            ),
        )
        .add_systems(
            PostUpdate,
            position_screen_bars::<T>.before(UiSystems::Prepare),
        );
    }
}

fn spawn_screen_bars<T: Percentage>(
    mut commands: Commands,
    colors: Res<ColorScheme<T>>,
    bars: Query<(Entity, &T, &BarSettings<T>), Added<BarSettings<T>>>,
) {
    for (owner, value, settings) in &bars {
        let BarHeight::Static(height) = settings.height;
        let percentage = value.value().clamp(0.0, 1.0);
        let width = settings.width * SCREEN_PIXELS_PER_WORLD_UNIT;
        let height = (height * SCREEN_PIXELS_PER_WORLD_UNIT).max(4.0);
        let ForegroundColor::Static(default_foreground) = &colors.foreground;
        let foreground = settings
            .foreground_color
            .clone()
            .unwrap_or_else(|| default_foreground.clone());

        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(width),
                    height: Val::Px(height),
                    border: UiRect::all(Val::Px(1.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.035, 0.035, 0.045)),
                BorderColor::all(Color::srgb(0.015, 0.015, 0.02)),
                GlobalZIndex(100),
                Pickable::IGNORE,
                ScreenBarRoot::<T> {
                    owner,
                    marker: PhantomData,
                },
            ))
            .with_child((
                Node {
                    width: Val::Percent(percentage * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(foreground),
                Pickable::IGNORE,
                ScreenBarFill::<T> {
                    owner,
                    marker: PhantomData,
                },
            ));
    }
}

fn update_screen_bar_fills<T: Percentage>(
    owners: Query<&T, Changed<T>>,
    mut fills: Query<(&ScreenBarFill<T>, &mut Node)>,
) {
    for (fill, mut node) in &mut fills {
        let Ok(value) = owners.get(fill.owner) else {
            continue;
        };
        node.width = Val::Percent(value.value().clamp(0.0, 1.0) * 100.0);
    }
}

fn position_screen_bars<T: Percentage>(
    mut commands: Commands,
    camera: Query<(&Camera, &Transform), With<Camera3d>>,
    owners: Query<(&Transform, &BarSettings<T>)>,
    mut bars: Query<(Entity, &ScreenBarRoot<T>, &mut Node)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let camera_global = GlobalTransform::from(*camera_transform);
    let viewport_size = camera.logical_viewport_size();

    for (bar_entity, bar, mut node) in &mut bars {
        let Ok((owner_transform, settings)) = owners.get(bar.owner) else {
            commands.entity(bar_entity).try_despawn();
            continue;
        };

        let anchor_offset = settings.screen_anchor_offset.unwrap_or(settings.offset);
        let world_anchor = owner_transform.translation + Vec3::Y * anchor_offset;
        let Ok(mut viewport_position) = camera.world_to_viewport(&camera_global, world_anchor)
        else {
            node.display = Display::None;
            continue;
        };
        viewport_position += settings.screen_offset;

        let BarHeight::Static(height) = settings.height;
        let width = settings.width * SCREEN_PIXELS_PER_WORLD_UNIT;
        let height = (height * SCREEN_PIXELS_PER_WORLD_UNIT).max(4.0);
        let on_screen = viewport_size.is_some_and(|viewport_size| {
            viewport_position.x >= -width
                && viewport_position.x <= viewport_size.x + width
                && viewport_position.y >= -height
                && viewport_position.y <= viewport_size.y + height
        });

        node.display = if on_screen {
            Display::Flex
        } else {
            Display::None
        };
        node.left = Val::Px(viewport_position.x - width * 0.5);
        node.top = Val::Px(viewport_position.y - height * 0.5);
    }
}
