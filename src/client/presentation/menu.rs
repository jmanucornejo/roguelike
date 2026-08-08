use crate::client::app::queue_client_connection;
use crate::client::state::CurrentClientId;
use crate::shared::gameplay::progression::CharacterClass;
use crate::shared::network::{
    channels::{ClientChannel, ServerChannel},
    messages::{
        AccountClientMessage, AccountServerMessage, CharacterSelectionSummary, CHARACTER_SLOT_COUNT,
    },
};
use crate::shared::states::ClientState;
use bevy::{
    app::AppExit,
    ecs::{component::Mutable, resource::IsResource},
    prelude::*,
};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use bevy_renet::{netcode::NetcodeClientTransport, RenetClient};

const TEXT_COLOR: Color = Color::srgb(0.95, 0.92, 0.77);
const MUTED_TEXT_COLOR: Color = Color::srgb(0.62, 0.67, 0.76);
const SCREEN_BACKGROUND: Color = Color::srgb(0.015, 0.02, 0.035);
const PANEL_BACKGROUND: Color = Color::srgba(0.035, 0.042, 0.065, 0.985);
const PANEL_BORDER: Color = Color::srgb(0.55, 0.68, 0.86);

// State used for the current menu screen
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum MenuState {
    Main,
    Settings,
    SettingsDisplay,
    SettingsSound,
    Account,
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

#[derive(Component)]
struct JoinStatusText;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum AccountView {
    #[default]
    Login,
    Characters,
}

#[derive(Resource, Debug, Default)]
struct AccountMenu {
    view: AccountView,
    username: String,
    password: String,
    new_character_name: String,
    characters: Vec<CharacterSelectionSummary>,
    status: String,
    pending: Option<AccountClientMessage>,
    request_sent: bool,
}

const NORMAL_BUTTON: Color = Color::srgb(0.08, 0.09, 0.12);
const HOVERED_BUTTON: Color = Color::srgb(0.14, 0.18, 0.26);
const HOVERED_PRESSED_BUTTON: Color = Color::srgb(0.18, 0.48, 0.28);
const PRESSED_BUTTON: Color = Color::srgb(0.14, 0.38, 0.23);

// Tag component used to mark which setting is currently selected
#[derive(Component)]
struct SelectedOption;

// All actions that can be triggered from a button click
#[derive(Component)]
enum MenuButtonAction {
    Join,
    Settings,
    SettingsDisplay,
    SettingsSound,
    BackToMainMenu,
    BackToSettings,
    Quit,
}

// One of the two settings that can be set through the menu. It will be a resource in the app
#[derive(Resource, Debug, PartialEq, Eq, Clone, Copy)]
enum DisplayQuality {
    Low,
    Medium,
    High,
}

// One of the two settings that can be set through the menu. It will be a resource in the app
#[derive(Resource, Debug, PartialEq, Eq, Clone, Copy)]
struct Volume(u32);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.init_state::<MenuState>()
            .init_resource::<AccountMenu>()
            .add_systems(
                OnEnter(ClientState::InMenu),
                (menu_setup, spawn_menu_camera),
            )
            // Systems to handle the main menu screen
            .add_systems(OnEnter(MenuState::Main), main_menu_setup)
            .add_systems(OnExit(MenuState::Main), despawn_screen::<OnMainMenuScreen>)
            .add_systems(OnEnter(MenuState::Account), account_menu_setup)
            .add_systems(
                EguiPrimaryContextPass,
                account_menu_ui.run_if(in_state(MenuState::Account)),
            )
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
                (menu_action, button_system, account_network_system)
                    .run_if(in_state(ClientState::InMenu)),
            )
            .add_systems(OnExit(ClientState::InMenu), despawn_menu_camera);

        fn menu_setup(mut menu_state: ResMut<NextState<MenuState>>) {
            menu_state.set(MenuState::Main);
        }

        fn account_menu_setup(mut account: ResMut<AccountMenu>) {
            account.status.clear();
            account.pending = None;
            account.request_sent = false;
        }

        fn account_menu_ui(
            mut contexts: EguiContexts,
            mut account: ResMut<AccountMenu>,
            mut commands: Commands,
            mut transport: Option<ResMut<NetcodeClientTransport>>,
            mut menu_state: ResMut<NextState<MenuState>>,
        ) -> Result {
            let context = contexts.ctx_mut()?;
            context.style_mut(|style| {
                style.visuals.panel_fill = egui::Color32::from_rgb(4, 6, 11);
                style.visuals.window_fill = egui::Color32::from_rgb(9, 11, 17);
                style.visuals.extreme_bg_color = egui::Color32::from_rgb(15, 18, 27);
                style.visuals.selection.bg_fill = egui::Color32::from_rgb(36, 97, 59);
                style.spacing.item_spacing = egui::vec2(8.0, 10.0);
                style.spacing.button_padding = egui::vec2(16.0, 8.0);
            });

            egui::CentralPanel::default().show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(52.0);
                    ui.heading(
                        egui::RichText::new("TRIBUTE")
                            .size(35.0)
                            .color(egui::Color32::from_rgb(242, 234, 196)),
                    );
                    ui.label(
                        egui::RichText::new(match account.view {
                            AccountView::Login => "Account Login",
                            AccountView::Characters => "Choose Your Character",
                        })
                        .size(18.0)
                        .color(egui::Color32::from_rgb(140, 174, 219)),
                    );
                    ui.add_space(18.0);

                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(9, 11, 17))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(92, 122, 168),
                        ))
                        .inner_margin(egui::Margin::same(24))
                        .show(ui, |ui| {
                            ui.set_width(460.0);
                            match account.view {
                                AccountView::Login => {
                                    ui.label("Username");
                                    ui.add_sized(
                                        [460.0, 34.0],
                                        egui::TextEdit::singleline(&mut account.username)
                                            .hint_text("3-32 letters, numbers, or underscores"),
                                    );
                                    ui.label("Password");
                                    let password_response = ui.add_sized(
                                        [460.0, 34.0],
                                        egui::TextEdit::singleline(&mut account.password)
                                            .password(true)
                                            .hint_text("8 or more characters"),
                                    );

                                    let waiting = account.pending.is_some();
                                    ui.horizontal(|ui| {
                                        let login_clicked = ui
                                            .add_enabled(!waiting, egui::Button::new("Log In"))
                                            .clicked();
                                        let create_clicked = ui
                                            .add_enabled(
                                                !waiting,
                                                egui::Button::new("Create Account"),
                                            )
                                            .clicked();
                                        if login_clicked
                                            || (password_response.lost_focus()
                                                && ui.input(|input| {
                                                    input.key_pressed(egui::Key::Enter)
                                                }))
                                        {
                                            account.pending = Some(AccountClientMessage::Login {
                                                username: account.username.clone(),
                                                password: account.password.clone(),
                                            });
                                            account.request_sent = false;
                                            account.status =
                                                "Connecting to the account server...".into();
                                        } else if create_clicked {
                                            account.pending =
                                                Some(AccountClientMessage::CreateAccount {
                                                    username: account.username.clone(),
                                                    password: account.password.clone(),
                                                });
                                            account.request_sent = false;
                                            account.status = "Creating account...".into();
                                        }
                                    });
                                    ui.separator();
                                    if ui.button("Back").clicked() {
                                        if let Some(transport) = transport.as_deref_mut() {
                                            transport.disconnect();
                                        }
                                        commands.remove_resource::<RenetClient>();
                                        commands.remove_resource::<NetcodeClientTransport>();
                                        commands.remove_resource::<CurrentClientId>();
                                        menu_state.set(MenuState::Main);
                                    }
                                }
                                AccountView::Characters => {
                                    ui.label(format!("Account: {}", account.username));
                                    ui.separator();
                                    let characters = account.characters.clone();
                                    for slot in 0..CHARACTER_SLOT_COUNT {
                                        if let Some(character) = characters
                                            .iter()
                                            .find(|character| character.slot == slot)
                                        {
                                            let class_name =
                                                CharacterClass::from_id(character.class_id)
                                                    .map(CharacterClass::name)
                                                    .unwrap_or("Unknown class");
                                            ui.horizontal(|ui| {
                                                ui.label(format!(
                                                    "Slot {}  {}  -  {}  Base {} / Job {}",
                                                    slot + 1,
                                                    character.name,
                                                    class_name,
                                                    character.base_level,
                                                    character.job_level
                                                ));
                                                if ui
                                                    .add_enabled(
                                                        account.pending.is_none(),
                                                        egui::Button::new("Enter World"),
                                                    )
                                                    .clicked()
                                                {
                                                    account.pending = Some(
                                                        AccountClientMessage::SelectCharacter {
                                                            character_id: character.id,
                                                        },
                                                    );
                                                    account.request_sent = false;
                                                    account.status =
                                                        format!("Loading {}...", character.name);
                                                }
                                            });
                                        }
                                    }

                                    if let Some(empty_slot) =
                                        (0..CHARACTER_SLOT_COUNT).find(|slot| {
                                            !characters
                                                .iter()
                                                .any(|character| character.slot == *slot)
                                        })
                                    {
                                        ui.separator();
                                        ui.label(format!(
                                            "Create a character in slot {}",
                                            empty_slot + 1
                                        ));
                                        ui.add_sized(
                                            [460.0, 34.0],
                                            egui::TextEdit::singleline(
                                                &mut account.new_character_name,
                                            )
                                            .hint_text("Character name"),
                                        );
                                        if ui
                                            .add_enabled(
                                                account.pending.is_none(),
                                                egui::Button::new("Create Character"),
                                            )
                                            .clicked()
                                        {
                                            account.pending =
                                                Some(AccountClientMessage::CreateCharacter {
                                                    slot: empty_slot,
                                                    name: account.new_character_name.clone(),
                                                });
                                            account.request_sent = false;
                                            account.status = "Creating character...".into();
                                        }
                                    } else {
                                        ui.label("All character slots are occupied.");
                                    }

                                    ui.separator();
                                    if ui.button("Log Out").clicked() {
                                        if let Some(transport) = transport.as_deref_mut() {
                                            transport.disconnect();
                                        }
                                        commands.remove_resource::<RenetClient>();
                                        commands.remove_resource::<NetcodeClientTransport>();
                                        commands.remove_resource::<CurrentClientId>();
                                        *account = AccountMenu::default();
                                        menu_state.set(MenuState::Main);
                                    }
                                }
                            }

                            if !account.status.is_empty() {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(&account.status)
                                        .color(egui::Color32::from_rgb(184, 194, 214)),
                                );
                            }
                        });
                });
            });
            Ok(())
        }

        fn account_network_system(
            mut commands: Commands,
            mut account: ResMut<AccountMenu>,
            mut client: Option<ResMut<RenetClient>>,
            mut game_state: ResMut<NextState<ClientState>>,
            mut menu_state: ResMut<NextState<MenuState>>,
        ) {
            if account.pending.is_some() && client.is_none() {
                match queue_client_connection(&mut commands) {
                    Ok(_) => account.status = "Connecting to the server...".into(),
                    Err(error) => {
                        account.pending = None;
                        account.status = format!("Connection could not start: {error}");
                    }
                }
                return;
            }

            let Some(client) = client.as_deref_mut() else {
                return;
            };
            if client.is_connected() && !account.request_sent {
                if let Some(request) = account.pending.as_ref() {
                    match bincode::serialize(request) {
                        Ok(message) => {
                            client.send_message(ClientChannel::Account, message);
                            account.request_sent = true;
                            account.status = "Waiting for the server...".into();
                        }
                        Err(error) => {
                            account.pending = None;
                            account.status = format!("Could not prepare request: {error}");
                        }
                    }
                }
            }

            while let Some(message) = client.receive_message(ServerChannel::Account) {
                match bincode::deserialize::<AccountServerMessage>(&message) {
                    Ok(AccountServerMessage::CharacterList {
                        username,
                        mut characters,
                    }) => {
                        characters.sort_by_key(|character| character.slot);
                        account.username = username;
                        account.password.clear();
                        account.new_character_name.clear();
                        account.characters = characters;
                        account.view = AccountView::Characters;
                        account.pending = None;
                        account.request_sent = false;
                        account.status.clear();
                    }
                    Ok(AccountServerMessage::EnteringWorld) => {
                        account.password.clear();
                        account.pending = None;
                        account.status = "Entering the world...".into();
                        game_state.set(ClientState::InGame);
                        menu_state.set(MenuState::Disabled);
                    }
                    Ok(AccountServerMessage::Error { message }) => {
                        account.pending = None;
                        account.request_sent = false;
                        account.status = message;
                    }
                    Err(error) => {
                        account.pending = None;
                        account.request_sent = false;
                        account.status = format!("Invalid account response: {error}");
                    }
                }
            }

            if client.is_disconnected() {
                commands.remove_resource::<RenetClient>();
                commands.remove_resource::<NetcodeClientTransport>();
                commands.remove_resource::<CurrentClientId>();
                account.pending = None;
                account.request_sent = false;
                account.view = AccountView::Login;
                account.characters.clear();
                account.status = "Connection failed or was closed. Please try again.".into();
            }
        }

        fn spawn_menu_camera(mut commands: Commands) {
            commands.spawn(Camera2d);
        }

        fn despawn_menu_camera(mut commands: Commands, camera_query: Query<(Entity, &Camera2d)>) {
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
                    (Interaction::Pressed, _) | (Interaction::None, Some(_)) => {
                        PRESSED_BUTTON.into()
                    }
                    (Interaction::Hovered, Some(_)) => HOVERED_PRESSED_BUTTON.into(),
                    (Interaction::Hovered, None) => HOVERED_BUTTON.into(),
                    (Interaction::None, None) => NORMAL_BUTTON.into(),
                }
            }
        }

        // This system updates the settings when a new value for a setting is selected, and marks
        // the button as the one currently selected
        fn setting_button<T: Resource<Mutability = Mutable> + PartialEq + Copy>(
            interaction_query: Query<
                (&Interaction, &T, Entity),
                (Changed<Interaction>, With<Button>, Without<IsResource>),
            >,
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
            let button_node = Node {
                width: Val::Px(330.0),
                height: Val::Px(54.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            };
            let button_icon_node = Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                ..default()
            };
            let button_text_font = TextFont {
                font_size: FontSize::Px(21.0),
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
                    BackgroundColor(SCREEN_BACKGROUND),
                    OnMainMenuScreen,
                ))
                .with_children(|parent| {
                    parent
                        .spawn((
                            Node {
                                width: Val::Px(440.0),
                                min_height: Val::Px(500.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                padding: UiRect::all(Val::Px(28.0)),
                                row_gap: Val::Px(10.0),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL_BACKGROUND),
                            BorderColor::all(PANEL_BORDER),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new("ONLINE WORLD · PRE-ALPHA"),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(MUTED_TEXT_COLOR),
                                Node {
                                    margin: UiRect::bottom(Val::Px(2.0)),
                                    ..default()
                                },
                            ));
                            parent.spawn((
                                Text::new("TRIBUTE"),
                                TextFont {
                                    font_size: FontSize::Px(54.0),
                                    ..default()
                                },
                                TextColor(TEXT_COLOR),
                            ));
                            parent.spawn((
                                Text::new("Enter a world shaped by the Andes"),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(MUTED_TEXT_COLOR),
                                Node {
                                    margin: UiRect::bottom(Val::Px(16.0)),
                                    ..default()
                                },
                            ));
                            parent.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(1.0),
                                    margin: UiRect::bottom(Val::Px(12.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.25, 0.34, 0.49)),
                            ));

                            parent
                                .spawn((
                                    Button,
                                    button_node.clone(),
                                    BackgroundColor(NORMAL_BUTTON),
                                    BorderColor::all(Color::srgb(0.36, 0.48, 0.66)),
                                    MenuButtonAction::Join,
                                ))
                                .with_children(|parent| {
                                    let icon = asset_server.load("textures/Game Icons/right.png");
                                    parent.spawn((ImageNode::new(icon), button_icon_node.clone()));
                                    parent.spawn((
                                        Text::new("Account Login"),
                                        button_text_font.clone(),
                                        TextColor(TEXT_COLOR),
                                    ));
                                });
                            parent
                                .spawn((
                                    Button,
                                    button_node.clone(),
                                    BackgroundColor(NORMAL_BUTTON),
                                    BorderColor::all(Color::srgb(0.24, 0.27, 0.34)),
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
                                    BorderColor::all(Color::srgb(0.24, 0.27, 0.34)),
                                    MenuButtonAction::Quit,
                                ))
                                .with_children(|parent| {
                                    let icon =
                                        asset_server.load("textures/Game Icons/exitRight.png");
                                    parent.spawn((ImageNode::new(icon), button_icon_node));
                                    parent.spawn((
                                        Text::new("Quit"),
                                        button_text_font,
                                        TextColor(TEXT_COLOR),
                                    ));
                                });
                            parent.spawn((
                                Text::new("Log in, create a character, then enter the world."),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(MUTED_TEXT_COLOR),
                                Node {
                                    margin: UiRect::top(Val::Px(14.0)),
                                    ..default()
                                },
                                JoinStatusText,
                            ));
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
                    font_size: FontSize::Px(33.0),
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
                    BackgroundColor(SCREEN_BACKGROUND),
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
                            BackgroundColor(PANEL_BACKGROUND),
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

        fn display_settings_menu_setup(
            mut commands: Commands,
            display_quality: Res<DisplayQuality>,
        ) {
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
                    font_size: FontSize::Px(33.0),
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
                    BackgroundColor(SCREEN_BACKGROUND),
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
                            BackgroundColor(PANEL_BACKGROUND),
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
                                    BackgroundColor(PANEL_BACKGROUND),
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
                    font_size: FontSize::Px(33.0),
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
                    BackgroundColor(SCREEN_BACKGROUND),
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
                            BackgroundColor(PANEL_BACKGROUND),
                        ))
                        .with_children(|parent| {
                            parent
                                .spawn((
                                    Node {
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(PANEL_BACKGROUND),
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
            mut app_exit_events: MessageWriter<AppExit>,
            mut menu_state: ResMut<NextState<MenuState>>,
        ) {
            for (interaction, menu_button_action) in &interaction_query {
                if *interaction == Interaction::Pressed {
                    match menu_button_action {
                        MenuButtonAction::Quit => {
                            app_exit_events.write(AppExit::Success);
                        }
                        MenuButtonAction::Join => menu_state.set(MenuState::Account),
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
        fn despawn_screen<T: Component>(
            to_despawn: Query<Entity, With<T>>,
            mut commands: Commands,
        ) {
            for entity in &to_despawn {
                commands.entity(entity).despawn();
            }
        }
    }
}
