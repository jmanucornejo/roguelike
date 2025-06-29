use bevy::prelude::*;
use crate::*;
use bevy::{app::AppExit, color::palettes::css::CRIMSON, prelude::*};
use client_plugins::shared::*;
use shared::states::ClientState;

const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);


// State used for the current menu screen
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum MenuState {
    Main,
    Settings,
    SettingsDisplay,
    SettingsSound,
    #[default]
    Disabled,
}


// Tag component used to tag entities added on the main menu screen
#[derive(Component)]
struct OnMainMenuScreen;

// Tag component used to tag entities added on the settings menu screen
#[derive(Component)]
struct OnSettingsMenuScreen;

// Tag component used to tag entities added on the display settings menu screen
#[derive(Component)]
struct OnDisplaySettingsMenuScreen;

// Tag component used to tag entities added on the sound settings menu screen
#[derive(Component)]
struct OnSoundSettingsMenuScreen;

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const HOVERED_PRESSED_BUTTON: Color = Color::srgb(0.25, 0.65, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);


// Tag component used to mark which setting is currently selected
#[derive(Component)]
struct SelectedOption;

// All actions that can be triggered from a button click
#[derive(Component)]
enum MenuButtonAction {
    Play,
    Settings,
    SettingsDisplay,
    SettingsSound,
    BackToMainMenu,
    BackToSettings,
    Quit,
}


// One of the two settings that can be set through the menu. It will be a resource in the app
#[derive(Resource, Debug, Component, PartialEq, Eq, Clone, Copy)]
enum DisplayQuality {
    Low,
    Medium,
    High,
}


// One of the two settings that can be set through the menu. It will be a resource in the app
#[derive(Resource, Debug, Component, PartialEq, Eq, Clone, Copy)]
struct Volume(u32);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app          
            .init_state::<MenuState>()            
            .add_systems(OnEnter(ClientState::InMenu), (menu_setup, spawn_menu_camera))
            // Systems to handle the main menu screen
            .add_systems(OnEnter(MenuState::Main), main_menu_setup)
            .add_systems(OnExit(MenuState::Main), despawn_screen::<OnMainMenuScreen>)
            // Systems to handle the settings menu screen
            .add_systems(OnEnter(MenuState::Settings), settings_menu_setup)
            .add_systems(
                OnExit(MenuState::Settings),
                despawn_screen::<OnSettingsMenuScreen>,
            )
            // Systems to handle the display settings screen
            .add_systems(
                OnEnter(MenuState::SettingsDisplay),
                display_settings_menu_setup,
            )
            .add_systems(
                Update,
                (setting_button::<DisplayQuality>.run_if(in_state(MenuState::SettingsDisplay)),),
            )
            .add_systems(
                OnExit(MenuState::SettingsDisplay),
                despawn_screen::<OnDisplaySettingsMenuScreen>,
            )
            // Systems to handle the sound settings screen
            .add_systems(OnEnter(MenuState::SettingsSound), sound_settings_menu_setup)
            .add_systems(
                Update,
                setting_button::<Volume>.run_if(in_state(MenuState::SettingsSound)),
            )
            .add_systems(
                OnExit(MenuState::SettingsSound),
                despawn_screen::<OnSoundSettingsMenuScreen>,
            )
            // Common systems to all screens that handles buttons behavior
            .add_systems(
                Update,
                (menu_action, button_system).run_if(in_state(ClientState::InMenu)),
            )
            .add_systems(OnExit(ClientState::InMenu), 
                despawn_menu_camera
            );


    
        fn menu_setup(mut menu_state: ResMut<NextState<MenuState>>) {     
            menu_state.set(MenuState::Main);
           
        }

        fn spawn_menu_camera(mut commands: Commands) {
            commands.spawn(Camera2d);
        }
    
        fn despawn_menu_camera(
            mut commands: Commands, 
            camera_query: Query<(Entity, &Camera2d)>,
        ) {

            if let Ok((entity, camera)) = camera_query.single() {
                commands.entity(entity).despawn();
            }           
           
         }
    
    
        // This system handles changing all buttons color based on mouse interaction
        fn button_system(
            mut interaction_query: Query<
                (&Interaction, &mut BackgroundColor, Option<&SelectedOption>),
                (Changed<Interaction>, With<Button>),
            >,
        ) {
            for (interaction, mut background_color, selected) in &mut interaction_query {
                *background_color = match (*interaction, selected) {
                    (Interaction::Pressed, _) | (Interaction::None, Some(_)) => PRESSED_BUTTON.into(),
                    (Interaction::Hovered, Some(_)) => HOVERED_PRESSED_BUTTON.into(),
                    (Interaction::Hovered, None) => HOVERED_BUTTON.into(),
                    (Interaction::None, None) => NORMAL_BUTTON.into(),
                }
            }
        }
  
  
        // This system updates the settings when a new value for a setting is selected, and marks
        // the button as the one currently selected
        fn setting_button<T: Resource + Component + PartialEq + Copy>(
            interaction_query: Query<(&Interaction, &T, Entity), (Changed<Interaction>, With<Button>)>,
            selected_query: Single<(Entity, &mut BackgroundColor), With<SelectedOption>>,
            mut commands: Commands,
            mut setting: ResMut<T>,
        ) {
            let (previous_button, mut previous_button_color) = selected_query.into_inner();
            for (interaction, button_setting, entity) in &interaction_query {
                if *interaction == Interaction::Pressed && *setting != *button_setting {
                    *previous_button_color = NORMAL_BUTTON.into();
                    commands.entity(previous_button).remove::<SelectedOption>();
                    commands.entity(entity).insert(SelectedOption);
                    *setting = *button_setting;
                }
            }
        }


        fn main_menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
            // Common style for all buttons on the screen
            let button_node = Node {
                width: Val::Px(300.0),
                height: Val::Px(65.0),
                margin: UiRect::all(Val::Px(20.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            };
            let button_icon_node = Node {
                width: Val::Px(30.0),
                // This takes the icons out of the flexbox flow, to be positioned exactly
                position_type: PositionType::Absolute,
                // The icon will be close to the left border of the button
                left: Val::Px(10.0),
                ..default()
            };
            let button_text_font = TextFont {
                font_size: 33.0,
                ..default()
            };
    
            commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    OnMainMenuScreen,
                ))
                .with_children(|parent| {
                    parent
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(CRIMSON.into()),
                        ))
                        .with_children(|parent| {
                            // Display the game name
                            parent.spawn((
                                Text::new("Bevy Game Menu UI"),
                                TextFont {
                                    font_size: 67.0,
                                    ..default()
                                },
                                TextColor(TEXT_COLOR),
                                Node {
                                    margin: UiRect::all(Val::Px(50.0)),
                                    ..default()
                                },
                            ));
    
                            // Display three buttons for each action available from the main menu:
                            // - new game
                            // - settings
                            // - quit
                            parent
                                .spawn((
                                    Button,
                                    button_node.clone(),
                                    BackgroundColor(NORMAL_BUTTON),
                                    MenuButtonAction::Play,
                                ))
                                .with_children(|parent| {
                                    let icon = asset_server.load("textures/Game Icons/right.png");
                                    parent.spawn((ImageNode::new(icon), button_icon_node.clone()));
                                    parent.spawn((
                                        Text::new("New Game"),
                                        button_text_font.clone(),
                                        TextColor(TEXT_COLOR),
                                    ));
                                });
                            parent
                                .spawn((
                                    Button,
                                    button_node.clone(),
                                    BackgroundColor(NORMAL_BUTTON),
                                    MenuButtonAction::Settings,
                                ))
                                .with_children(|parent| {
                                    let icon = asset_server.load("textures/Game Icons/wrench.png");
                                    parent.spawn((ImageNode::new(icon), button_icon_node.clone()));
                                    parent.spawn((
                                        Text::new("Settings"),
                                        button_text_font.clone(),
                                        TextColor(TEXT_COLOR),
                                    ));
                                });
                            parent
                                .spawn((
                                    Button,
                                    button_node,
                                    BackgroundColor(NORMAL_BUTTON),
                                    MenuButtonAction::Quit,
                                ))
                                .with_children(|parent| {
                                    let icon = asset_server.load("textures/Game Icons/exitRight.png");
                                    parent.spawn((ImageNode::new(icon), button_icon_node));
                                    parent.spawn((
                                        Text::new("Quit"),
                                        button_text_font,
                                        TextColor(TEXT_COLOR),
                                    ));
                                });
                        });
                });
        }

        fn settings_menu_setup(mut commands: Commands) {
            let button_node = Node {
                width: Val::Px(200.0),
                height: Val::Px(65.0),
                margin: UiRect::all(Val::Px(20.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            };
    
            let button_text_style = (
                TextFont {
                    font_size: 33.0,
                    ..default()
                },
                TextColor(TEXT_COLOR),
            );
    
            commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    OnSettingsMenuScreen,
                ))
                .with_children(|parent| {
                    parent
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(CRIMSON.into()),
                        ))
                        .with_children(|parent| {
                            for (action, text) in [
                                (MenuButtonAction::SettingsDisplay, "Display"),
                                (MenuButtonAction::SettingsSound, "Sound"),
                                (MenuButtonAction::BackToMainMenu, "Back"),
                            ] {
                                parent
                                    .spawn((
                                        Button,
                                        button_node.clone(),
                                        BackgroundColor(NORMAL_BUTTON),
                                        action,
                                    ))
                                    .with_children(|parent| {
                                        parent.spawn((Text::new(text), button_text_style.clone()));
                                    });
                            }
                        });
                });
        }
    
        fn display_settings_menu_setup(mut commands: Commands, display_quality: Res<DisplayQuality>) {
            let button_node = Node {
                width: Val::Px(200.0),
                height: Val::Px(65.0),
                margin: UiRect::all(Val::Px(20.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            };
            let button_text_style = (
                TextFont {
                    font_size: 33.0,
                    ..default()
                },
                TextColor(TEXT_COLOR),
            );
    
            commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    OnDisplaySettingsMenuScreen,
                ))
                .with_children(|parent| {
                    parent
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(CRIMSON.into()),
                        ))
                        .with_children(|parent| {
                            // Create a new `Node`, this time not setting its `flex_direction`. It will
                            // use the default value, `FlexDirection::Row`, from left to right.
                            parent
                                .spawn((
                                    Node {
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(CRIMSON.into()),
                                ))
                                .with_children(|parent| {
                                    // Display a label for the current setting
                                    parent.spawn((
                                        Text::new("Display Quality"),
                                        button_text_style.clone(),
                                    ));
                                    // Display a button for each possible value
                                    for quality_setting in [
                                        DisplayQuality::Low,
                                        DisplayQuality::Medium,
                                        DisplayQuality::High,
                                    ] {
                                        let mut entity = parent.spawn((
                                            Button,
                                            Node {
                                                width: Val::Px(150.0),
                                                height: Val::Px(65.0),
                                                ..button_node.clone()
                                            },
                                            BackgroundColor(NORMAL_BUTTON),
                                            quality_setting,
                                        ));
                                        entity.with_children(|parent| {
                                            parent.spawn((
                                                Text::new(format!("{quality_setting:?}")),
                                                button_text_style.clone(),
                                            ));
                                        });
                                        if *display_quality == quality_setting {
                                            entity.insert(SelectedOption);
                                        }
                                    }
                                });
                            // Display the back button to return to the settings screen
                            parent
                                .spawn((
                                    Button,
                                    button_node,
                                    BackgroundColor(NORMAL_BUTTON),
                                    MenuButtonAction::BackToSettings,
                                ))
                                .with_children(|parent| {
                                    parent.spawn((Text::new("Back"), button_text_style));
                                });
                        });
                });
        }
    
        fn sound_settings_menu_setup(mut commands: Commands, volume: Res<Volume>) {
            let button_node = Node {
                width: Val::Px(200.0),
                height: Val::Px(65.0),
                margin: UiRect::all(Val::Px(20.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            };
            let button_text_style = (
                TextFont {
                    font_size: 33.0,
                    ..default()
                },
                TextColor(TEXT_COLOR),
            );
    
            commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    OnSoundSettingsMenuScreen,
                ))
                .with_children(|parent| {
                    parent
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(CRIMSON.into()),
                        ))
                        .with_children(|parent| {
                            parent
                                .spawn((
                                    Node {
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(CRIMSON.into()),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((Text::new("Volume"), button_text_style.clone()));
                                    for volume_setting in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
                                        let mut entity = parent.spawn((
                                            Button,
                                            Node {
                                                width: Val::Px(30.0),
                                                height: Val::Px(65.0),
                                                ..button_node.clone()
                                            },
                                            BackgroundColor(NORMAL_BUTTON),
                                            Volume(volume_setting),
                                        ));
                                        if *volume == Volume(volume_setting) {
                                            entity.insert(SelectedOption);
                                        }
                                    }
                                });
                            parent
                                .spawn((
                                    Button,
                                    button_node,
                                    BackgroundColor(NORMAL_BUTTON),
                                    MenuButtonAction::BackToSettings,
                                ))
                                .with_child((Text::new("Back"), button_text_style));
                        });
                });
        }
    
        fn menu_action(
            interaction_query: Query<
                (&Interaction, &MenuButtonAction),
                (Changed<Interaction>, With<Button>),
            >,
            mut app_exit_events: EventWriter<AppExit>,
            mut menu_state: ResMut<NextState<MenuState>>,
            mut game_state: ResMut<NextState<ClientState>>,
        ) {
            for (interaction, menu_button_action) in &interaction_query {
                if *interaction == Interaction::Pressed {
                    match menu_button_action {
                        MenuButtonAction::Quit => {
                            app_exit_events.send(AppExit::Success);
                        }
                        MenuButtonAction::Play => {
                            game_state.set(ClientState::InGame);
                            menu_state.set(MenuState::Disabled);
                        }
                        MenuButtonAction::Settings => menu_state.set(MenuState::Settings),
                        MenuButtonAction::SettingsDisplay => {
                            menu_state.set(MenuState::SettingsDisplay);
                        }
                        MenuButtonAction::SettingsSound => {
                            menu_state.set(MenuState::SettingsSound);
                        }
                        MenuButtonAction::BackToMainMenu => menu_state.set(MenuState::Main),
                        MenuButtonAction::BackToSettings => {
                            menu_state.set(MenuState::Settings);
                        }
                    }
                }
            }
        }

                
        // Generic system that takes a component as a parameter, and will despawn all entities with that component
        fn despawn_screen<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
            for entity in &to_despawn {
                commands.entity(entity).despawn_recursive();
            }
        }
    }

} 