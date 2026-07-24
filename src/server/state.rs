use std::collections::HashMap;

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use renet_visualizer::RenetServerVisualizer;

#[derive(Debug, Default, Resource)]
pub(super) struct ServerLobby {
    pub(super) players: HashMap<ClientId, Entity>,
}

pub(super) enum PendingServerEvent {
    Connected(ClientId),
    Disconnected(ClientId, String),
}

#[derive(Default, Resource)]
pub(super) struct PendingServerEvents(pub(super) Vec<PendingServerEvent>);

#[derive(Resource)]
pub(super) struct SnapshotTimer(pub(super) Timer);

#[derive(Resource, Deref, DerefMut)]
pub(super) struct ServerVisualizer(pub(super) RenetServerVisualizer<200>);
