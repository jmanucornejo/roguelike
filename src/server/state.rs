use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use renet_visualizer::RenetServerVisualizer;

use crate::server::persistence::{CharacterId, CharacterSnapshot};

#[derive(Debug)]
pub(super) struct DeferredCharacterSave {
    pub(super) snapshot: CharacterSnapshot,
    pub(super) reason: &'static str,
}

#[derive(Debug, Default, Resource)]
pub(super) struct ServerLobby {
    pub(super) players: HashMap<ClientId, Entity>,
    /// Stable database character identity kept independently from the ECS
    /// entity. This survives partial component removal during disconnect.
    pub(super) characters: HashMap<ClientId, CharacterId>,
}

pub(super) enum PendingServerEvent {
    Connected(ClientId),
    Disconnected(ClientId, String),
}

#[derive(Default, Resource)]
pub(super) struct PendingServerEvents(pub(super) Vec<PendingServerEvent>);

#[derive(Resource)]
pub(super) struct CharacterPersistenceQueue {
    next_request_id: u64,
    pub(super) waiting_clients: HashSet<ClientId>,
    pub(super) load_requests: HashMap<u64, ClientId>,
    pub(super) save_requests: HashMap<u64, CharacterId>,
    pub(super) saves_in_flight: HashMap<CharacterId, CharacterSnapshot>,
    pub(super) deferred_saves: HashMap<CharacterId, DeferredCharacterSave>,
    pub(super) revisions: HashMap<CharacterId, u64>,
    pub(super) last_saved: HashMap<CharacterId, CharacterSnapshot>,
    pub(super) autosave_timer: Timer,
}

impl Default for CharacterPersistenceQueue {
    fn default() -> Self {
        Self {
            next_request_id: 0,
            waiting_clients: HashSet::new(),
            load_requests: HashMap::new(),
            save_requests: HashMap::new(),
            saves_in_flight: HashMap::new(),
            deferred_saves: HashMap::new(),
            revisions: HashMap::new(),
            last_saved: HashMap::new(),
            autosave_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        }
    }
}

impl CharacterPersistenceQueue {
    pub(super) fn next_request_id(&mut self) -> u64 {
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        self.next_request_id
    }

    pub(super) fn remove_client(&mut self, client_id: ClientId) {
        self.waiting_clients.remove(&client_id);
        self.load_requests
            .retain(|_, pending_client_id| *pending_client_id != client_id);
    }
}

#[derive(Resource)]
pub(super) struct SnapshotTimer(pub(super) Timer);

#[derive(Resource, Deref, DerefMut)]
pub(super) struct ServerVisualizer(pub(super) RenetServerVisualizer<200>);
