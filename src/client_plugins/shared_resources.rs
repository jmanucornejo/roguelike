use bevy::prelude::Resource;


#[derive(Default, Resource)]
pub struct ServerTime(pub u128);

#[derive(Default, Resource)]
pub struct ClockOffset(pub u128);

#[derive(Default, Resource)]
pub struct PrevClock(pub u128);



