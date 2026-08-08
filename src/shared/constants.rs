pub const CLOCK_SYNC_CHANNEL_ID: u8 = 10;
pub const PROTOCOL_ID: u64 = 1000;
pub const PLAYER_MOVE_SPEED: f32 = 10.0;
pub const ATTACK_ANIMATION_FRAME_COUNT: usize = 8;
pub const ATTACK_HIT_FRAME_INDEX: usize = 4;
pub const ATTACK_HIT_FRACTION: f32 =
    ATTACK_HIT_FRAME_INDEX as f32 / ATTACK_ANIMATION_FRAME_COUNT as f32;
pub const CHARACTER_GRAVITY: f32 = -9.81;
/// Downward speed requested while grounded so Rapier keeps snap-to-ground
/// active when a character walks onto a descending surface.
pub const CHARACTER_GROUND_STICK_SPEED: f32 = -2.0;
pub const CHARACTER_CONTROLLER_OFFSET: f32 = 0.3;
pub const CHARACTER_GROUND_SNAP_DISTANCE: f32 = 0.5;
/// Safe starting-map position on connected terrain and outside the origin wall.
pub const DEFAULT_CHARACTER_SPAWN: [f32; 3] = [-10.0, 1.0, 0.0];
pub const WATER_LEVEL: f32 = -2.0;
pub const WATER_RENDER_LEVEL: f32 = WATER_LEVEL - 2.02;
pub const LINE_OF_SIGHT: f32 = 14.0;
pub const TRANSLATION_PRECISION: f32 = 0.001;
pub const INTERPOLATE_BUFFER: u128 = 200;
pub const NETWORK_SNAPSHOT_HZ: f32 = 30.0;
pub const MAX_POSITION_SNAPSHOTS: usize = 64;
pub const PREDICTION_SOFT_CORRECTION_RATE: f32 = 8.0;
pub const PREDICTION_HARD_SNAP_DISTANCE: f32 = 2.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifth_attack_frame_is_the_halfway_hit_point() {
        assert_eq!(ATTACK_HIT_FRAME_INDEX + 1, 5);
        assert!((ATTACK_HIT_FRACTION - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn water_renders_just_below_its_gameplay_height() {
        assert!(WATER_RENDER_LEVEL < WATER_LEVEL);
        assert!((WATER_LEVEL - WATER_RENDER_LEVEL - 0.02).abs() < f32::EPSILON);
    }
}
