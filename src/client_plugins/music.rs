
use bevy::prelude::*;
use crate::*;


#[derive(Component)]
struct MyMusic;


pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app          
            .add_systems(OnEnter(AppState::InGame), (setup_music));


        fn setup_music(asset_server: Res<AssetServer>, mut commands: Commands) {

            commands.spawn((
                AudioBundle {
                    source: asset_server.load("audio/music/tribute.ogg"),
                    settings: PlaybackSettings::LOOP,
                    //..default()
                },
                MyMusic,
            ));            
            
        }
        
        /*
        fn pause(keyboard_input: Res<Input<KeyCode>>, music_controller: Query<&AudioSink, With<MyMusic>>) {
            if keyboard_input.just_pressed(KeyCode::Space) {
                if let Ok(sink) = music_controller.get_single() {
                    sink.toggle();
                }
            }
        }
        
        fn volume(keyboard_input: Res<Input<KeyCode>>, music_controller: Query<&AudioSink, With<MyMusic>>) {
            if let Ok(sink) = music_controller.get_single() {
                if keyboard_input.just_pressed(KeyCode::Plus) {
                    sink.set_volume(sink.volume() + 0.1);
                } else if keyboard_input.just_pressed(KeyCode::Minus) {
                    sink.set_volume(sink.volume() - 0.1);
                }
            }
        }*/
    }

  
}