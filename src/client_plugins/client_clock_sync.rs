use std::ops::Mul;
use bevy::prelude::*;
use crate::*;
use client_plugins::shared::*;

#[derive(Default, Resource)]
struct SyncData {
    timer: Timer,
    samples: Vec<u16>,
    total_rtt: u128,
    total_offset: u128,
    sync_attempts: usize,
    max_attempts: usize,
}

#[derive(Default, Resource)]
struct Latency(u16);

pub struct ClientClockSyncPlugin;

impl Plugin for ClientClockSyncPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app          
            .insert_resource(Latency::default())
            .insert_resource(SyncData  {
                timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                samples: Vec::new(),
                total_rtt: 0,
                total_offset: 0,
                sync_attempts: 0,
                max_attempts: 10, // Number of sync requests to send
            })
            .add_systems(OnEnter(AppState::InGame), ((setup_server_time_and_latency)))
            .add_systems(
                FixedUpdate, (
                    client_sync_time_system
                )
            );
        
      
      
        fn setup_server_time_and_latency(
            time: Res<Time>,
            mut client: ResMut<RenetClient>,
        ) {

            let sync_request_message = bincode::serialize(&ClientMessage::SyncTimeRequest { client_time: time.elapsed().as_millis() }).unwrap();

            client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
            //client.send_message(reliable_channel_id, ping_message);
            info!("Sent sync time request!");
        }

        fn client_sync_time_system(
            time: Res<Time>,
            mut sync_data: ResMut<SyncData>,
            mut client: ResMut<RenetClient>,
            mut server_time_res: ResMut<ServerTime>,
            mut latency: ResMut<Latency>,
            mut clock_offset: ResMut<ClockOffset>,
        ) {

            sync_data.timer.tick(time.delta());

            if sync_data.timer.finished() {
            // let sync_request_message = bincode::serialize(&ClientMessage::LatencyRequest { client_time: time.elapsed().as_millis() }).unwrap();
                //client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
            }

            while let Some(message) = client.receive_message(ClientChannel::SyncTimeRequest) {
                let server_message = bincode::deserialize(&message).unwrap();
                match server_message {
                    ServerMessage::SyncTimeResponse { client_time, server_time } => {                      

                        let rtt = time.elapsed().as_millis() - client_time;
                        let one_way_latency = rtt / 2;
                        server_time_res.0 = server_time + one_way_latency;
                        clock_offset.0 = server_time_res.0 - time.elapsed().as_millis();
                        latency.0 = one_way_latency as u16;
                        info!("server_time_res {:?}, latency  {:?}", server_time_res.0, one_way_latency);

                        sync_data.total_rtt += rtt;
                        sync_data.total_offset +=  clock_offset.0;
                        sync_data.sync_attempts += 1;

                        if sync_data.sync_attempts >= sync_data.max_attempts {
                            let avg_rtt = sync_data.total_rtt / sync_data.sync_attempts as u128;
                            let one_way_latency = avg_rtt / 2;
                            //latency.0 = one_way_latency;

                            server_time_res.0 = server_time + one_way_latency;
                            latency.0 = one_way_latency as u16;
                            clock_offset.0 = sync_data.total_offset / sync_data.sync_attempts as u128;
                            info!("one_way_latency {:?}",one_way_latency);
                            info!("offset {:?}",clock_offset.0);
                        
                        
                            // Adjust client clock
                            //client_time.0 = estimated_server_time;

                            // Reset sync data for next sync cycle
                            // sync_data.pending_requests = 0;
                            // sync_data.total_rtt = 0;
                            // sync_data.sync_attempts = 0;
                        } 
                        else {
                            let sync_request_message = bincode::serialize(&ClientMessage::SyncTimeRequest { client_time: time.elapsed().as_millis() }).unwrap();
                            client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);

                        }
                    
                    },
                    ServerMessage::LatencyResponse { client_time } => {           

                        //info!("client_time{:?}",client_time);
                        let rtt = (time.elapsed().as_millis() - client_time) as u16;

                        sync_data.samples.push(rtt);

                        if(sync_data.samples.len() == 9) {

                            sync_data.samples.sort();

                            //let mid_point = sync_data.samples.get(4);

                            let median = sync_data.samples[4];
                            info!("median {:?}",median);
            
                            sync_data.samples.retain(|sample|  if *sample > median.mul(2) && *sample > 20 {  
                                false
                            }
                            else {
                                true
                            });
                            info!("median {:?}",sync_data.samples);
            
                            latency.0 = sync_data.samples.iter().sum::<u16 >() / sync_data.samples.len() as u16 ;
                            info!("average_latency {:?}",latency.0);

                            sync_data.samples.clear();

                        }
                    
                        /* 
                        sync_data.total_rtt += rtt;
                        sync_data.sync_attempts += 1;

                        if sync_data.sync_attempts >= sync_data.max_attempts {
                            let avg_rtt = sync_data.total_rtt / sync_data.sync_attempts as u128;
                            let one_way_latency = avg_rtt / 2;
                            //latency.0 = one_way_latency;

                        
                            info!("one_way_latency {:?}",one_way_latency);
                        
                            // Adjust client clock
                            //client_time.0 = estimated_server_time;

                            // Reset sync data for next sync cycle
                            // sync_data.pending_requests = 0;
                            // sync_data.total_rtt = 0;
                            // sync_data.sync_attempts = 0;
                        } */
                    }
                }
            }
        }

     

    }

    
}