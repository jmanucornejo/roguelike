use std::ops::Mul;
use bevy::prelude::*;
use crate::*;
use client_plugins::shared::*;
use shared::messages::*;
use shared::constants::*;
use shared::channels::ClientChannel;
use shared::states::ClientState;

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

#[derive(Resource)]
pub struct ClockSync {
    pub offset: i128,
    smoothing_factor: f64,
    is_initialized: bool,     
}


impl Default for ClockSync {
    fn default() -> Self {
        Self {
            offset: 0,
            smoothing_factor: 0.1,
            is_initialized: false,
        }
    }
}
pub struct ClientClockSyncPlugin;

impl Plugin for ClientClockSyncPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app        
            .insert_resource(PingTimer(Timer::new(Duration::from_secs(2), TimerMode::Repeating)))  
            .insert_resource(SyncTimer(Timer::new(Duration::from_secs(5), TimerMode::Repeating)))  
            .insert_resource(ClockSync::default())
            .insert_resource(RenderTime::default())
            .insert_resource(ClockOffset::default())
            .add_systems(Update, (send_ping.run_if(in_state(ClientState::InGame)), update_server_time_and_latency.run_if(in_state(ClientState::InGame))))
            .insert_resource(Latency::default())
            .insert_resource(SyncData  {
                timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                samples: Vec::new(),
                total_rtt: 0,
                total_offset: 0,
                sync_attempts: 0,
                max_attempts: 10, // Number of sync requests to send
            })       
            .add_systems(OnEnter(ClientState::InGame), ((setup_server_time_and_latency, send_initial_ping)))
            .add_systems(FixedUpdate, (
                set_render_time.run_if(in_state(ClientState::InGame)),  
                //clean_absolute_buffer.run_if(in_state(ClientState::InGame))
            ))
            .add_systems(
                Update, (
                    client_sync_time_system.run_if(in_state(ClientState::InGame))
                )
            );
    }    
}


fn setup_server_time_and_latency(
    time: Res<Time>,
    mut client: ResMut<RenetClient>,
) {

    let sync_request_message = bincode::serialize(&ClientSyncMessages::SyncTimeRequest { client_time: time.elapsed().as_millis() }).unwrap();

    client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
    //client.send_message(reliable_channel_id, ping_message);
    info!("Sent sync time request!");
}

fn update_server_time_and_latency(
    mut timer: ResMut<SyncTimer>,
    time: Res<Time>,
    mut client: ResMut<RenetClient>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        let sync_request_message = bincode::serialize(&ClientSyncMessages::SyncTimeRequest { client_time: time.elapsed().as_millis() }).unwrap();

        client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
        //client.send_message(reliable_channel_id, ping_message);
        info!("Sent sync time request!");
    }
}

pub fn set_render_time( time: Res<Time>,  mut render_time: ResMut<RenderTime>, clock_offset: Res<ClockOffset>,) 
{
    let estimated_server_time = time.elapsed().as_millis() + clock_offset.0;  

    if estimated_server_time >= INTERPOLATE_BUFFER {
        render_time.0 = estimated_server_time - INTERPOLATE_BUFFER; 
    }

}

fn client_sync_time_system(
    time: Res<Time>,
    mut sync_data: ResMut<SyncData>,
    mut client: ResMut<RenetClient>, 
    mut latency: ResMut<Latency>,
    mut clock_offset: ResMut<ClockOffset>,
    mut clock_sync: ResMut<ClockSync>,
) {

    sync_data.timer.tick(time.delta());

    if sync_data.timer.finished() {
    // let sync_request_message = bincode::serialize(&ClientSyncMessages::LatencyRequest { client_time: time.elapsed().as_millis() }).unwrap();
        //client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);
    }
    let now = time.elapsed().as_millis();

    while let Some(message) = client.receive_message(ClientChannel::SyncTimeRequest) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerSyncMessages::Pong { client_time,  server_time } => {    

                let rtt = now - client_time;
                let one_way_latency = rtt / 2;
                let estimated_server_time = server_time + one_way_latency;
                /*info!(
                    "Estimated_server_time = {}",
                    estimated_server_time
                );*/                    
                let new_offset = estimated_server_time as i128 - now as i128;

                if !clock_sync.is_initialized {
                    // First sync — set directly
                    clock_sync.offset = new_offset;
                    clock_sync.is_initialized = true;
                } else {
                    // Smoothly adjust offset
                    let alpha = clock_sync.smoothing_factor;
                    let smoothed = (1.0 - alpha) * clock_sync.offset as f64 + alpha * new_offset as f64;
                    clock_sync.offset = smoothed.round() as i128;
                }

                /*info!(
                    "Clock sync: new offset = {}, smoothed = {}",
                    new_offset, clock_sync.offset
                ); */                   
            },
            ServerSyncMessages::SyncTimeResponse { client_time, server_time } => {                      

                let rtt = now - client_time;
                let one_way_latency = rtt / 2;
                /*server_time_res.0 = server_time + one_way_latency;
                clock_offset.0 = server_time_res.0 - now;
                latency.0 = one_way_latency as u16;*/
                // info!("server_time_res {:?}, latency  {:?}", server_time_res.0, one_way_latency);

                sync_data.total_rtt += rtt;
                sync_data.total_offset +=  server_time + one_way_latency - now;
                sync_data.sync_attempts += 1;

                if sync_data.sync_attempts >= sync_data.max_attempts {
                    let avg_rtt = sync_data.total_rtt / sync_data.sync_attempts as u128;
                    let one_way_latency = avg_rtt / 2;
                    //latency.0 = one_way_latency;

                    let new_clock_offset = sync_data.total_offset / sync_data.sync_attempts as u128; ;
                    info!("new_clock_offset {:?}",new_clock_offset);
                    info!("old_clocl_offset {:?}",  clock_offset.0);
                    if (new_clock_offset as i128 -  clock_offset.0 as i128).abs() > 50 {                                      
                        // la hora se ha alejado más de 100ms, ajustar hora.                    
                        latency.0 = one_way_latency as u16;
                        clock_offset.0 = new_clock_offset;       

                        info!("one_way_latency {:?}",one_way_latency);
                        info!("offset {:?}",clock_offset.0);
                        info!("estimated server_time {:?}", now + clock_offset.0);                      
                
                    }
                 
                   

                  
                    // Reset data for next sync cycle.
                    sync_data.total_rtt = 0;
                    sync_data.total_offset = 0;
                    sync_data.sync_attempts = 0;
                    // Adjust client clock
                    //client_time.0 = estimated_server_time;

                    // Reset sync data for next sync cycle
                    // sync_data.pending_requests = 0;
                    // sync_data.total_rtt = 0;
                    // sync_data.sync_attempts = 0;
                } 
                else {
                    let sync_request_message = bincode::serialize(&ClientSyncMessages::SyncTimeRequest { client_time: now }).unwrap();
                    client.send_message(ClientChannel::SyncTimeRequest, sync_request_message);

                }
            
            },
            ServerSyncMessages::LatencyResponse { client_time } => {           

                //info!("client_time{:?}",client_time);
                let rtt = (now - client_time) as u16;

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

#[derive(Resource)]
struct PingTimer(Timer);

#[derive(Resource)]
struct SyncTimer(Timer);

fn send_initial_ping(    
    time: Res<Time>,
    mut client: ResMut<RenetClient>,) 
{
    let ping = ClientSyncMessages::Ping {
        client_time: time.elapsed().as_millis(),
    };

    let message = bincode::serialize(&ping).unwrap();
    client.send_message(ClientChannel::SyncTimeRequest, message);
}

fn send_ping(
    mut timer: ResMut<PingTimer>,
    time: Res<Time>,
    mut client: ResMut<RenetClient>,
) {
  
    if timer.0.tick(time.delta()).just_finished() {
        let ping = ClientSyncMessages::Ping {
            client_time: time.elapsed().as_millis(),
        };

        let message = bincode::serialize(&ping).unwrap();
        client.send_message(ClientChannel::SyncTimeRequest, message);
    }
}

/// Get the estimated current server time
pub fn get_server_time(time: &Time, clock: &ClockSync) -> u128 {
   
    if clock.is_initialized == false {
        return 0
    }       

    (time.elapsed().as_millis() as i128 + clock.offset).max(0) as u128
}
    