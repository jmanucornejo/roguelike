use bevy::prelude::*;

use crate::shared::gameplay::components::Facing;

#[derive(Component, Debug)]
pub struct PrevState {
    pub translation: Vec3,
    pub rotation: Facing,
}

#[derive(Debug, Default, Component)]
pub struct LineOfSight(pub Vec<Entity>);

#[derive(Debug, Default, Component)]
pub struct SeenBy(pub Vec<Entity>);
