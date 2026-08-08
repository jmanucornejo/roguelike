use bevy::{
    prelude::*,
    window::{PrimaryWindow, Window},
};

use crate::shared::states::ClientState;

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct DraggableUi {
    handle_height: f32,
}

impl DraggableUi {
    pub(crate) const fn header(handle_height: f32) -> Self {
        Self { handle_height }
    }

    pub(crate) const fn entire_panel() -> Self {
        Self {
            handle_height: f32::INFINITY,
        }
    }
}

#[derive(Resource, Debug, Default)]
struct UiDragState {
    entity: Option<Entity>,
    pointer_offset: Vec2,
}

pub(crate) struct UiDragPlugin;

impl Plugin for UiDragPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDragState>()
            .add_systems(Update, drag_ui_panels.run_if(in_state(ClientState::InGame)));
    }
}

fn drag_ui_panels(
    window: Query<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<UiDragState>,
    mut panels: Query<(
        Entity,
        &DraggableUi,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&GlobalZIndex>,
        &InheritedVisibility,
        &mut Node,
    )>,
) {
    let Ok(window) = window.single() else {
        state.entity = None;
        return;
    };

    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        state.entity = None;
    }

    let Some(pointer) = window.cursor_position() else {
        return;
    };

    if let Some(entity) = state.entity {
        let Ok((_, _, computed, _, _, visibility, mut node)) = panels.get_mut(entity) else {
            state.entity = None;
            return;
        };
        if !visibility.get() {
            state.entity = None;
            return;
        }

        let position = clamp_panel_position(
            pointer - state.pointer_offset,
            computed.size(),
            Vec2::new(window.width(), window.height()),
        );
        node.position_type = PositionType::Absolute;
        node.left = Val::Px(position.x);
        node.top = Val::Px(position.y);
        node.right = Val::Auto;
        node.bottom = Val::Auto;
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let mut candidate: Option<(Entity, i32, Vec2)> = None;
    for (entity, draggable, computed, transform, z_index, visibility, _) in &mut panels {
        if !visibility.get() || !computed.contains_point(*transform, pointer) {
            continue;
        }

        let (_, _, center) = transform.to_scale_angle_translation();
        let top_left = center - computed.size() * 0.5;
        let local_pointer = pointer - top_left;
        if local_pointer.y < 0.0 || local_pointer.y > draggable.handle_height {
            continue;
        }

        let z_index = z_index.map_or(0, |z_index| z_index.0);
        if candidate.is_none_or(|(_, candidate_z, _)| z_index >= candidate_z) {
            candidate = Some((entity, z_index, local_pointer));
        }
    }

    if let Some((entity, _, pointer_offset)) = candidate {
        state.entity = Some(entity);
        state.pointer_offset = pointer_offset;
    }
}

fn clamp_panel_position(position: Vec2, panel_size: Vec2, window_size: Vec2) -> Vec2 {
    let max = (window_size - panel_size).max(Vec2::ZERO);
    position.clamp(Vec2::ZERO, max)
}

pub(crate) fn pointer_over_draggable_ui(
    pointer: Vec2,
    panels: &Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<DraggableUi>>,
) -> bool {
    panels.iter().any(|(node, transform, visibility)| {
        visibility.get() && node.contains_point(*transform, pointer)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_positions_are_clamped_inside_the_window() {
        let window = Vec2::new(800.0, 600.0);
        let panel = Vec2::new(300.0, 200.0);

        assert_eq!(
            clamp_panel_position(Vec2::new(-20.0, 700.0), panel, window),
            Vec2::new(0.0, 400.0)
        );
    }
}
