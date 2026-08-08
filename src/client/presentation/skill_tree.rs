use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};

use crate::{
    client::{
        presentation::action_bar::{ActionBarBindings, ActionBarState},
        presentation::ui_drag::DraggableUi,
        state::ControlledPlayer,
    },
    shared::{
        gameplay::{
            action_bar::ActionBarBinding,
            progression::JobProgression,
            skills::{skill_definition, SkillDefinition, SkillId, SkillTree, SKILL_DEFINITIONS},
        },
        network::messages::PlayerCommand,
        states::ClientState,
    },
};

const PANEL_LEFT: f32 = 130.0;
const PANEL_TOP: f32 = 72.0;
const PANEL_WIDTH: f32 = 460.0;
const PANEL_HEIGHT: f32 = 376.0;

#[derive(Resource, Debug, Default)]
pub(crate) struct SkillTreeUiState {
    open: bool,
    dragging: Option<SkillId>,
    drag_ghost: Option<Entity>,
}

#[derive(Component)]
struct SkillTreeRoot;

#[derive(Component)]
struct SkillTreeHeader;

#[derive(Component)]
struct AvailableSkillPointsText;

#[derive(Component)]
struct SkillTreeRow(SkillId);

#[derive(Component)]
struct SkillRankText(SkillId);

#[derive(Component)]
struct SkillRequirementText(SkillId);

#[derive(Component)]
struct SkillIncreaseButton(SkillId);

#[derive(Component)]
struct SkillDragHandle(SkillId);

#[derive(Component)]
struct SkillDragGhost;

pub(crate) struct SkillTreePlugin;

impl Plugin for SkillTreePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillTreeUiState>()
            .add_systems(OnEnter(ClientState::InGame), spawn_skill_tree)
            .add_systems(
                Update,
                (
                    toggle_skill_tree,
                    begin_skill_drag,
                    update_skill_drag.after(begin_skill_drag),
                    spend_skill_point,
                    update_skill_tree,
                )
                    .run_if(in_state(ClientState::InGame)),
            )
            .add_systems(OnExit(ClientState::InGame), despawn_skill_tree);
    }
}

fn spawn_skill_tree(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PANEL_LEFT),
                top: Val::Px(PANEL_TOP),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(PANEL_HEIGHT),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.042, 0.065, 0.98)),
            BorderColor::all(Color::srgb(0.48, 0.62, 0.82)),
            GlobalZIndex(700),
            Visibility::Hidden,
            DraggableUi::header(36.0),
            SkillTreeRoot,
            Name::new("Skill Tree Panel"),
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|header| {
                    header.spawn((
                        Text::new("Novice Skill Tree"),
                        TextFont {
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.86, 0.52)),
                        Pickable::IGNORE,
                        SkillTreeHeader,
                    ));
                    header.spawn((
                        Text::new("Skill Points: 0"),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.42, 0.84, 1.0)),
                        Node {
                            margin: UiRect::left(Val::Auto),
                            ..default()
                        },
                        Pickable::IGNORE,
                        AvailableSkillPointsText,
                    ));
                });

            panel.spawn((
                Text::new(
                    "Spend one point per rank. Prerequisites are checked by the server. [K] Close",
                ),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.66, 0.70, 0.78)),
                Pickable::IGNORE,
            ));

            for definition in SKILL_DEFINITIONS {
                spawn_skill_row(panel, definition);
            }
        });
}

fn spawn_skill_row(panel: &mut ChildSpawnerCommands, definition: SkillDefinition) {
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(92.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.075, 0.09, 0.13, 0.98)),
            BorderColor::all(Color::srgb(0.25, 0.34, 0.46)),
            Visibility::Hidden,
            SkillTreeRow(definition.id),
            Name::new(format!("{} skill row", definition.name)),
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|title| {
                title
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(42.0),
                            height: Val::Px(22.0),
                            margin: UiRect::right(Val::Px(7.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.19, 0.22)),
                        BorderColor::all(Color::srgb(0.42, 0.45, 0.52)),
                        SkillDragHandle(definition.id),
                    ))
                    .with_children(|handle| {
                        handle.spawn((
                            Text::new("DRAG"),
                            TextFont {
                                font_size: FontSize::Px(8.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        ));
                    });
                title.spawn((
                    Text::new(format!(
                        "{}  Lv. 0/{}",
                        definition.name, definition.max_rank
                    )),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.93, 0.94, 0.98)),
                    Pickable::IGNORE,
                    SkillRankText(definition.id),
                ));
                title
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(30.0),
                            height: Val::Px(25.0),
                            margin: UiRect::left(Val::Auto),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.42, 0.25)),
                        BorderColor::all(Color::srgb(0.58, 0.82, 0.63)),
                        SkillIncreaseButton(definition.id),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("+"),
                            TextFont {
                                font_size: FontSize::Px(18.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        ));
                    });
            });
            row.spawn((
                Text::new(definition.description),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.81, 0.88)),
                Pickable::IGNORE,
            ));
            row.spawn((
                Text::new("No prerequisite"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.57, 0.78, 0.61)),
                Pickable::IGNORE,
                SkillRequirementText(definition.id),
            ));
        });
}

fn toggle_skill_tree(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SkillTreeUiState>,
    mut roots: Query<&mut Visibility, With<SkillTreeRoot>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyK) {
        return;
    }
    state.open = !state.open;
    for mut visibility in &mut roots {
        *visibility = if state.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn begin_skill_drag(
    interactions: Query<(&Interaction, &SkillDragHandle), Changed<Interaction>>,
    player: Query<&SkillTree, With<ControlledPlayer>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    mut state: ResMut<SkillTreeUiState>,
) {
    let Ok(skill_tree) = player.single() else {
        return;
    };
    let Ok(window) = primary_window.single() else {
        return;
    };
    for (interaction, handle) in &interactions {
        if *interaction != Interaction::Pressed || skill_tree.rank(handle.0) == 0 {
            continue;
        }
        if let Some(old_ghost) = state.drag_ghost.take() {
            commands.entity(old_ghost).try_despawn();
        }
        let pointer = window.cursor_position().unwrap_or_default();
        let ghost = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(pointer.x - 12.0),
                    top: Val::Px(pointer.y - 12.0),
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.28, 0.52, 0.88)),
                BorderColor::all(Color::WHITE),
                GlobalZIndex(1003),
                Pickable::IGNORE,
                SkillDragGhost,
                Name::new("Dragged learned skill"),
            ))
            .id();
        state.dragging = Some(handle.0);
        state.drag_ghost = Some(ghost);
    }
}

fn update_skill_drag(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    action_bar: Res<ActionBarState>,
    mut bindings: ResMut<ActionBarBindings>,
    mut state: ResMut<SkillTreeUiState>,
    mut ghosts: Query<&mut Node, With<SkillDragGhost>>,
    mut player_commands: MessageWriter<PlayerCommand>,
    mut commands: Commands,
) {
    let Some(skill_id) = state.dragging else {
        return;
    };
    let Ok(window) = primary_window.single() else {
        return;
    };
    let pointer = window.cursor_position();
    if let (Some(pointer), Some(ghost)) = (pointer, state.drag_ghost) {
        if let Ok(mut node) = ghosts.get_mut(ghost) {
            node.left = Val::Px(pointer.x - 12.0);
            node.top = Val::Px(pointer.y - 12.0);
        }
    }
    if !mouse_buttons.just_released(MouseButton::Left) {
        return;
    }

    if let Some(slot_index) = pointer.and_then(|pointer| action_bar.slot_at(pointer)) {
        if bindings.bind_skill(slot_index, skill_id) {
            player_commands.write(PlayerCommand::SetActionBarSlot {
                slot_index: slot_index as u8,
                binding: Some(ActionBarBinding::Skill(skill_id)),
            });
        }
    }
    if let Some(ghost) = state.drag_ghost.take() {
        commands.entity(ghost).try_despawn();
    }
    state.dragging = None;
}

fn spend_skill_point(
    interactions: Query<(&Interaction, &SkillIncreaseButton), (Changed<Interaction>, With<Button>)>,
    player: Query<(&JobProgression, &SkillTree), With<ControlledPlayer>>,
    mut commands: MessageWriter<PlayerCommand>,
) {
    let Ok((job_progression, skill_tree)) = player.single() else {
        return;
    };
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed
            && skill_tree
                .can_spend_point(job_progression.class, button.0)
                .is_ok()
        {
            commands.write(PlayerCommand::SpendSkillPoint { skill_id: button.0 });
        }
    }
}

fn update_skill_tree(
    player: Query<
        (&JobProgression, &SkillTree),
        (
            With<ControlledPlayer>,
            Or<(Changed<JobProgression>, Changed<SkillTree>)>,
        ),
    >,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<SkillTreeHeader>>,
        Query<&mut Text, With<AvailableSkillPointsText>>,
        Query<(&SkillRankText, &mut Text)>,
        Query<(&SkillRequirementText, &mut Text, &mut TextColor)>,
    )>,
    mut rows: Query<(&SkillTreeRow, &mut Visibility)>,
    mut buttons: Query<(&SkillIncreaseButton, &mut BackgroundColor)>,
    mut drag_handles: Query<(&SkillDragHandle, &mut BackgroundColor), Without<SkillIncreaseButton>>,
) {
    let Ok((job_progression, skill_tree)) = player.single() else {
        return;
    };

    {
        let mut headers = text_queries.p0();
        for mut header in &mut headers {
            header.0 = format!("{} Skill Tree", job_progression.class.name());
        }
    }
    {
        let mut points_text = text_queries.p1();
        for mut text in &mut points_text {
            text.0 = format!("Skill Points: {}", skill_tree.available_points());
        }
    }
    for (row, mut visibility) in &mut rows {
        let visible = skill_definition(row.0)
            .is_some_and(|definition| definition.class == job_progression.class);
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    {
        let mut ranks = text_queries.p2();
        for (rank_text, mut text) in &mut ranks {
            let Some(definition) = skill_definition(rank_text.0) else {
                continue;
            };
            text.0 = format!(
                "{}  Lv. {}/{}",
                definition.name,
                skill_tree.rank(definition.id),
                definition.max_rank
            );
        }
    }
    {
        let mut requirements = text_queries.p3();
        for (requirement_text, mut text, mut color) in &mut requirements {
            let Some(definition) = skill_definition(requirement_text.0) else {
                continue;
            };
            let current_rank = skill_tree.rank(definition.id);
            if current_rank >= definition.max_rank {
                text.0 = "Maximum rank reached".into();
                color.0 = Color::srgb(0.96, 0.76, 0.30);
            } else if let Some(requirement) = definition.prerequisite {
                let prerequisite_name = skill_definition(requirement.skill_id)
                    .map_or("Unknown skill", |prerequisite| prerequisite.name);
                let prerequisite_rank = skill_tree.rank(requirement.skill_id);
                text.0 = format!(
                    "Requires {} Lv. {}  (current {})",
                    prerequisite_name, requirement.rank, prerequisite_rank
                );
                color.0 = if prerequisite_rank >= requirement.rank {
                    Color::srgb(0.48, 0.88, 0.55)
                } else {
                    Color::srgb(0.96, 0.38, 0.34)
                };
            } else {
                text.0 = "No prerequisite".into();
                color.0 = Color::srgb(0.57, 0.78, 0.61);
            }
        }
    }
    for (button, mut background) in &mut buttons {
        background.0 = if skill_tree
            .can_spend_point(job_progression.class, button.0)
            .is_ok()
        {
            Color::srgb(0.18, 0.48, 0.27)
        } else {
            Color::srgb(0.18, 0.19, 0.22)
        };
    }
    for (handle, mut background) in &mut drag_handles {
        background.0 = if skill_tree.rank(handle.0) > 0 {
            Color::srgb(0.22, 0.42, 0.72)
        } else {
            Color::srgb(0.18, 0.19, 0.22)
        };
    }
}

fn despawn_skill_tree(
    mut commands: Commands,
    roots: Query<Entity, With<SkillTreeRoot>>,
    ghosts: Query<Entity, With<SkillDragGhost>>,
    mut state: ResMut<SkillTreeUiState>,
) {
    for entity in roots.iter().chain(ghosts.iter()) {
        commands.entity(entity).try_despawn();
    }
    state.open = false;
    state.dragging = None;
    state.drag_ghost = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_tree_system_initializes_and_updates_all_ui_groups() {
        let mut app = App::new();
        app.add_systems(Update, update_skill_tree);
        app.world_mut().spawn((
            ControlledPlayer,
            JobProgression {
                class: crate::shared::gameplay::progression::CharacterClass::Novice,
                level: 2,
                experience: 0,
            },
            SkillTree::from_persisted(
                crate::shared::gameplay::progression::CharacterClass::Novice,
                2,
                std::iter::empty(),
            ),
        ));

        let header = app.world_mut().spawn((Text::new(""), SkillTreeHeader)).id();
        let points = app
            .world_mut()
            .spawn((Text::new(""), AvailableSkillPointsText))
            .id();
        let row = app
            .world_mut()
            .spawn((Visibility::Hidden, SkillTreeRow(SkillId(100))))
            .id();
        let rank = app
            .world_mut()
            .spawn((Text::new(""), SkillRankText(SkillId(100))))
            .id();
        let requirement = app
            .world_mut()
            .spawn((
                Text::new(""),
                TextColor(Color::WHITE),
                SkillRequirementText(SkillId(100)),
            ))
            .id();
        app.world_mut().spawn((
            BackgroundColor(Color::BLACK),
            SkillIncreaseButton(SkillId(100)),
        ));
        app.world_mut()
            .spawn((BackgroundColor(Color::BLACK), SkillDragHandle(SkillId(100))));

        app.update();

        let class_name = crate::shared::gameplay::progression::CharacterClass::Novice.name();
        assert_eq!(
            app.world().get::<Text>(header).unwrap().0,
            format!("{class_name} Skill Tree")
        );
        assert_eq!(
            app.world().get::<Text>(points).unwrap().0,
            "Skill Points: 1"
        );
        assert_eq!(
            *app.world().get::<Visibility>(row).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            app.world().get::<Text>(rank).unwrap().0,
            "First Aid  Lv. 0/5"
        );
        assert_eq!(
            app.world().get::<Text>(requirement).unwrap().0,
            "No prerequisite"
        );
    }
}
