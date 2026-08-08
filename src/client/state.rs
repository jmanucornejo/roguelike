use bevy::prelude::{Component, Deref, DerefMut, Entity, Resource};
use bevy_renet::renet::ClientId;
use std::collections::HashMap;

use crate::shared::gameplay::components::PlayerInput;

#[derive(Debug)]
pub(super) struct PlayerInfo {
    pub(super) client_entity: Entity,
    pub(super) server_entity: Entity,
}

#[derive(Debug, Default, Resource)]
pub(super) struct ClientLobby {
    pub(super) players: HashMap<ClientId, PlayerInfo>,
}

#[derive(Debug, Resource)]
pub(super) struct CurrentClientId(pub(super) u64);

#[derive(Debug, Default, Resource, Deref, DerefMut)]
pub(super) struct LocalPlayerInput(pub(super) PlayerInput);

#[derive(Default, Resource)]
pub struct NetworkMapping(pub HashMap<Entity, Entity>);

#[derive(Component)]
pub struct ControlledPlayer;

#[derive(Default, Resource)]
pub struct RenderTime(pub u128);

#[derive(Debug, Default, Resource)]
pub struct CameraFacing(pub u8);

#[derive(Debug, Default, Resource)]
pub(super) struct CurrentClientMap(pub(super) Option<String>);
