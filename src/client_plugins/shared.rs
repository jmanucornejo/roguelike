use bevy::prelude::Entity;
use bevy::prelude::Resource;
use bevy::prelude::Component;
use std::{
    collections::{HashMap}
};

#[derive(Default, Resource)]
pub struct ServerTime(pub u128);

#[derive(Default, Resource)]
pub struct ClockOffset(pub u128);

#[derive(Default, Resource)]
pub struct PrevClock(pub u128);

#[derive(Default, Resource, )]
pub struct NetworkMapping(pub HashMap<Entity, Entity>);

#[derive(Component)]
pub struct ControlledPlayer;
