//! Bevy 0.19 compatibility layer for the subset of `bevy_spatial` used by the server.
//!
//! The upstream crate currently targets Bevy 0.16. This module deliberately keeps
//! the same automatic, periodically rebuilt KD-tree design and the small API surface
//! used by this project, backed by the same `kd-tree` crate.

use std::{marker::PhantomData, time::Duration};

use bevy::prelude::*;
use kd_tree::{KdPoint, KdTree as BaseKdTree, KdTreeN};
use typenum::U3;

#[derive(Component)]
pub struct NearestNeighbourComponent;

pub type NNTree = KDTree3<NearestNeighbourComponent>;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point3 {
    position: Vec3,
    entity: Option<Entity>,
}

impl From<Vec3> for Point3 {
    fn from(position: Vec3) -> Self {
        Self {
            position,
            entity: None,
        }
    }
}

impl From<(Entity, Vec3)> for Point3 {
    fn from((entity, position): (Entity, Vec3)) -> Self {
        Self {
            position,
            entity: Some(entity),
        }
    }
}

impl KdPoint for Point3 {
    type Scalar = f32;
    type Dim = U3;

    fn at(&self, index: usize) -> Self::Scalar {
        self.position[index]
    }
}

#[derive(Resource)]
pub struct KDTree3<C: Component> {
    tree: BaseKdTree<Point3>,
    marker: PhantomData<fn() -> C>,
}

impl<C: Component> Default for KDTree3<C> {
    fn default() -> Self {
        Self {
            tree: BaseKdTree::default(),
            marker: PhantomData,
        }
    }
}

pub trait SpatialAccess {
    fn within_distance(&self, location: Vec3, distance: f32) -> Vec<(Vec3, Option<Entity>)>;
}

impl<C: Component> SpatialAccess for KDTree3<C> {
    fn within_distance(&self, location: Vec3, distance: f32) -> Vec<(Vec3, Option<Entity>)> {
        if self.tree.is_empty() {
            return Vec::new();
        }

        self.tree
            .within_radius(&Point3::from(location), distance)
            .iter()
            .map(|point| (point.position, point.entity))
            .collect()
    }
}

#[derive(Resource)]
struct SpatialUpdateTimer<C: Component> {
    timer: Timer,
    marker: PhantomData<fn() -> C>,
}

pub struct AutomaticUpdate<C: Component> {
    frequency: Duration,
    marker: PhantomData<fn() -> C>,
}

impl<C: Component> AutomaticUpdate<C> {
    pub fn new() -> Self {
        Self {
            frequency: Duration::from_millis(50),
            marker: PhantomData,
        }
    }
}

impl<C: Component> Plugin for AutomaticUpdate<C> {
    fn build(&self, app: &mut App) {
        app.init_resource::<KDTree3<C>>()
            .insert_resource(SpatialUpdateTimer::<C> {
                timer: Timer::new(self.frequency, TimerMode::Repeating),
                marker: PhantomData,
            })
            .add_systems(Update, rebuild_tree::<C>);
    }
}

fn rebuild_tree<C: Component>(
    time: Res<Time>,
    mut timer: ResMut<SpatialUpdateTimer<C>>,
    mut tree: ResMut<KDTree3<C>>,
    tracked: Query<(Entity, &Transform), With<C>>,
) {
    timer.timer.tick(time.delta());
    if !timer.timer.just_finished() {
        return;
    }

    tree.tree = KdTreeN::par_build_by_ordered_float(
        tracked
            .iter()
            .map(|(entity, transform)| Point3::from((entity, transform.translation)))
            .collect::<Vec<_>>(),
    );
}
