
use bevy::state::state::States;

#[derive(States, Default, Hash, Debug, PartialEq, Clone, Eq)]
pub enum ClientState {
    // Make this the default instead of `InMenu`.
    #[default]
    Setup,
    InMenu,
    InGame,
}


#[derive(States, Default, Hash, Debug, PartialEq, Clone, Eq)]
pub enum ServerState {
    // Make this the default instead of `InMenu`.
    #[default]
    Initializing,
    InGame,
    Maintenance
}
