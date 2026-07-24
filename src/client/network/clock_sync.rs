use std::time::Duration;

use bevy::prelude::*;
use bevy_renet::{RenetClient, RenetReceive};

use crate::{
    client::state::RenderTime,
    shared::{
        constants::INTERPOLATE_BUFFER,
        network::{
            channels::{ClientChannel, ServerChannel},
            messages::{ClientSyncMessages, ServerSyncMessages},
        },
        states::ClientState,
    },
};

#[derive(Resource, Debug)]
pub struct ClockSync {
    /// Signed server-clock minus client-clock difference in milliseconds.
    offset_ms: f64,
    round_trip_ms: f64,
    initialized: bool,
}

impl Default for ClockSync {
    fn default() -> Self {
        Self {
            offset_ms: 0.0,
            round_trip_ms: 0.0,
            initialized: false,
        }
    }
}

impl ClockSync {
    fn record_sample(&mut self, client_sent: u128, client_now: u128, server_time: u128) {
        let round_trip = client_now.saturating_sub(client_sent) as f64;
        let client_midpoint = client_sent as f64 + round_trip * 0.5;
        let sampled_offset = server_time as f64 - client_midpoint;

        if !self.initialized {
            self.offset_ms = sampled_offset;
            self.round_trip_ms = round_trip;
            self.initialized = true;
            return;
        }

        // Clock samples are noisy because transport latency is asymmetric. A small EMA keeps
        // render time monotonic-looking without assuming that the offset must be positive.
        const OFFSET_ALPHA: f64 = 0.1;
        const RTT_ALPHA: f64 = 0.2;
        self.offset_ms += (sampled_offset - self.offset_ms) * OFFSET_ALPHA;
        self.round_trip_ms += (round_trip - self.round_trip_ms) * RTT_ALPHA;
    }

    pub fn estimated_server_time(&self, client_time_ms: u128) -> Option<u128> {
        self.initialized
            .then(|| (client_time_ms as f64 + self.offset_ms).max(0.0).round() as u128)
    }

    pub fn round_trip_ms(&self) -> Option<f64> {
        self.initialized.then_some(self.round_trip_ms)
    }
}

#[derive(Resource)]
struct ClockSyncTimer(Timer);

pub struct ClientClockSyncPlugin;

impl Plugin for ClientClockSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClockSync>()
            .init_resource::<RenderTime>()
            .insert_resource(ClockSyncTimer(Timer::new(
                Duration::from_secs(1),
                TimerMode::Repeating,
            )))
            .add_systems(OnEnter(ClientState::InGame), send_clock_sync_request)
            .add_systems(
                PreUpdate,
                (receive_clock_sync, set_render_time)
                    .chain()
                    .after(RenetReceive)
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(
                Update,
                send_periodic_clock_sync.run_if(in_state(ClientState::InGame)),
            );
    }
}

fn send_clock_sync_request(time: Res<Time>, mut client: ResMut<RenetClient>) {
    let request = ClientSyncMessages::Ping {
        client_time: time.elapsed().as_millis(),
    };
    client.send_message(
        ClientChannel::SyncTimeRequest,
        bincode::serialize(&request).expect("clock request should serialize"),
    );
}

fn send_periodic_clock_sync(
    time: Res<Time>,
    mut timer: ResMut<ClockSyncTimer>,
    mut client: ResMut<RenetClient>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        let request = ClientSyncMessages::Ping {
            client_time: time.elapsed().as_millis(),
        };
        client.send_message(
            ClientChannel::SyncTimeRequest,
            bincode::serialize(&request).expect("clock request should serialize"),
        );
    }
}

fn receive_clock_sync(
    time: Res<Time>,
    mut client: ResMut<RenetClient>,
    mut clock: ResMut<ClockSync>,
) {
    let client_now = time.elapsed().as_millis();

    while let Some(message) = client.receive_message(ServerChannel::SyncTimeResponse) {
        let Ok(response) = bincode::deserialize::<ServerSyncMessages>(&message) else {
            warn!("Ignoring malformed clock synchronization response");
            continue;
        };

        match response {
            ServerSyncMessages::Pong {
                client_time,
                server_time,
            }
            | ServerSyncMessages::SyncTimeResponse {
                client_time,
                server_time,
            } => clock.record_sample(client_time, client_now, server_time),
            ServerSyncMessages::LatencyResponse { .. } => {}
        }
    }
}

pub fn set_render_time(
    time: Res<Time>,
    clock: Res<ClockSync>,
    mut render_time: ResMut<RenderTime>,
) {
    render_time.0 = clock
        .estimated_server_time(time.elapsed().as_millis())
        .map(|server_time| server_time.saturating_sub(INTERPOLATE_BUFFER))
        .unwrap_or_default();
}

pub fn get_server_time(time: &Time, clock: &ClockSync) -> u128 {
    clock
        .estimated_server_time(time.elapsed().as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_negative_server_offsets() {
        let mut clock = ClockSync::default();
        clock.record_sample(1_000, 1_100, 950);
        assert_eq!(clock.estimated_server_time(2_000), Some(1_900));
    }

    #[test]
    fn midpoint_compensates_for_round_trip_time() {
        let mut clock = ClockSync::default();
        clock.record_sample(1_000, 1_100, 1_100);
        assert_eq!(clock.estimated_server_time(2_000), Some(2_050));
    }
}
