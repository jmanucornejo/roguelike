use bevy::prelude::*;
use crate::shared::channels::{ClientChannel, ServerChannel};
use crate::*;
use crate::shared::messages::*;


pub struct ServerClockSyncPlugin;

impl Plugin for ServerClockSyncPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app                      
            .add_systems(FixedUpdate, sync_client_time);
        
        fn sync_client_time(
            mut server: ResMut<RenetServer>,
            time: Res<Time>
        ) {
            //let reliable_channel_id = ReliableChannelConfig::default().channel_id;
            //println!("Time  {:?} ", time.elapsed().as_millis() );
            // Receive message from channel
            for client_id in server.clients_id() {
                // The enum DefaultChannel describe the channels used by the default configuration
                while let Some(message) = server.receive_message(client_id, ClientChannel::SyncTimeRequest) {
                    let client_message: ClientSyncMessages = bincode::deserialize(&message).unwrap();
                    match client_message {
                        ClientSyncMessages::Ping { client_time } => {
                            //info!("Got sync time request from {}!", client_id);
                            let sync_time_response = bincode::serialize(&ServerSyncMessages::Pong { client_time: client_time, server_time: time.elapsed().as_millis() }).unwrap();
                            server.send_message(client_id, ServerChannel::SyncTimeResponse, sync_time_response);
                        },
                        ClientSyncMessages::SyncTimeRequest { client_time } => {
                            //info!("Got sync time request from {}!", client_id);
                            let sync_time_response = bincode::serialize(&ServerSyncMessages::SyncTimeResponse { client_time: client_time, server_time: time.elapsed().as_millis() }).unwrap();
                            server.send_message(client_id, ServerChannel::SyncTimeResponse, sync_time_response);
                        },
                        ClientSyncMessages::LatencyRequest { client_time } => {
                            //info!("Got Latency request from {}!", time.elapsed().as_millis(), client_id);
                            let sync_time_response = bincode::serialize(&ServerSyncMessages::LatencyResponse { client_time: client_time }).unwrap();
                            server.send_message(client_id, ServerChannel::SyncTimeResponse, sync_time_response);
                        },
                    }
                }
            }
        }
    }    
}
