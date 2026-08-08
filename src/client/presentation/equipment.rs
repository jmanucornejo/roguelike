use bevy::prelude::*;

use crate::{
    client::{presentation::ui_drag::DraggableUi, state::ControlledPlayer},
    shared::{
        gameplay::{
            components::{Equipment, EquipmentSlot},
            items::{equipment_bonus_summary, item_definition},
        },
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

const PANEL_WIDTH: f32 = 360.0;
const PANEL_HEIGHT: f32 = 452.0;
const PANEL_RIGHT: f32 = 300.0;
const PANEL_TOP: f32 = 12.0;
const DOUBLE_CLICK_SECONDS: f64 = 0.35;

#[derive(Resource, Debug, Default)]
pub(crate) struct EquipmentUiState {
    open: bool,
    last_click: Option<(EquipmentSlot, f64)>,
}

#[derive(Component)]
struct EquipmentPanelRoot;

#[derive(Component)]
struct EquipmentItemText(EquipmentSlot);

#[derive(Component)]
struct EquipmentRow(EquipmentSlot);

pub(crate) struct EquipmentUiPlugin;

impl Plugin for EquipmentUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EquipmentUiState>()
            .add_systems(OnEnter(ClientState::InGame), spawn_equipment_panel)
            .add_systems(
                Update,
                (
                    toggle_equipment_panel,
                    equipment_slot_interactions,
                    update_equipment_panel,
                )
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_equipment_panel);
    }
}

fn spawn_equipment_panel(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(PANEL_RIGHT),
                top: Val::Px(PANEL_TOP),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(PANEL_HEIGHT),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.045, 0.052, 0.07, 0.97)),
            BorderColor::all(Color::srgba(0.60, 0.64, 0.72, 0.96)),
            GlobalZIndex(310),
            Visibility::Hidden,
            Pickable::IGNORE,
            DraggableUi::header(32.0),
            EquipmentPanelRoot,
            Name::new("Equipment Panel"),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Equipment [E]"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.92, 0.77)),
                Pickable::IGNORE,
            ));
            panel.spawn((
                Text::new("Double-click an occupied slot to unequip"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.62, 0.66, 0.74)),
                Pickable::IGNORE,
            ));

            for slot in EquipmentSlot::ALL {
                panel
                    .spawn((
                        Node {
                            height: Val::Px(32.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.92)),
                        BorderColor::all(Color::srgb(0.24, 0.27, 0.34)),
                        Button,
                        EquipmentRow(slot),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new(slot.name()),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.72, 0.76, 0.84)),
                            Node {
                                width: Val::Px(96.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                        row.spawn((
                            Text::new("Empty"),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.92, 0.95)),
                            Pickable::IGNORE,
                            EquipmentItemText(slot),
                        ));
                    });
            }
        });
}

fn toggle_equipment_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EquipmentUiState>,
    mut panels: Query<&mut Visibility, With<EquipmentPanelRoot>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }
    state.open = !state.open;
    if !state.open {
        state.last_click = None;
    }
    for mut visibility in &mut panels {
        *visibility = if state.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn equipment_slot_interactions(
    time: Res<Time>,
    mut interactions: Query<
        (&Interaction, &EquipmentRow, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    equipment: Query<&Equipment, With<ControlledPlayer>>,
    mut state: ResMut<EquipmentUiState>,
    mut player_commands: MessageWriter<PlayerCommand>,
) {
    let Ok(equipment) = equipment.single() else {
        return;
    };

    for (interaction, row, mut background) in &mut interactions {
        background.0 = match *interaction {
            Interaction::Pressed => Color::srgba(0.18, 0.20, 0.27, 0.98),
            Interaction::Hovered => Color::srgba(0.12, 0.14, 0.19, 0.96),
            Interaction::None => Color::srgba(0.08, 0.09, 0.12, 0.92),
        };
        if *interaction != Interaction::Pressed || equipment.item(row.0).is_none() {
            continue;
        }

        let now = time.elapsed_secs_f64();
        if state.last_click.is_some_and(|(slot, clicked_at)| {
            slot == row.0 && now - clicked_at <= DOUBLE_CLICK_SECONDS
        }) {
            player_commands.write(PlayerCommand::UnequipItem { slot: row.0 });
            state.last_click = None;
        } else {
            state.last_click = Some((row.0, now));
        }
    }
}

fn update_equipment_panel(
    equipment: Query<&Equipment, (With<ControlledPlayer>, Changed<Equipment>)>,
    mut item_texts: Query<(&EquipmentItemText, &mut Text, &mut TextColor)>,
) {
    let Ok(equipment) = equipment.single() else {
        return;
    };

    for (slot_text, mut text, mut color) in &mut item_texts {
        if let Some(item_id) = equipment.item(slot_text.0) {
            text.0 = item_definition(item_id)
                .map(|definition| {
                    let bonuses = equipment_bonus_summary(definition.bonuses);
                    if bonuses.is_empty() {
                        definition.name.to_owned()
                    } else {
                        format!("{}  ({bonuses})", definition.name)
                    }
                })
                .unwrap_or_else(|| format!("Unknown item #{}", item_id.0));
            color.0 = Color::srgb(0.98, 0.84, 0.38);
        } else {
            text.0 = "Empty".into();
            color.0 = Color::srgb(0.52, 0.55, 0.62);
        }
    }
}

fn despawn_equipment_panel(
    panels: Query<Entity, With<EquipmentPanelRoot>>,
    mut state: ResMut<EquipmentUiState>,
    mut commands: Commands,
) {
    for panel in &panels {
        commands.entity(panel).try_despawn();
    }
    state.open = false;
    state.last_click = None;
}
