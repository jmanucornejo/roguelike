use bevy::prelude::*;

use crate::shared::gameplay::components::Facing;

#[derive(Component, Debug)]
pub struct PrevState {
    pub translation: Vec3,
    pub rotation: Facing,
}

#[derive(Debug, Default, Component)]
pub struct LineOfSight(pub Vec<Entity>);

#[derive(Debug, Default, Component)]
pub struct SeenBy(pub Vec<Entity>);

/// Returns whether a viewer should receive a transient action performed by a
/// player. Looking at both interest sets keeps actions visible during the one
/// simulation tick in which two nearby players' sets are being reconciled.
pub fn should_receive_player_action(
    viewer_entity: Entity,
    actor_entity: Entity,
    viewer_line_of_sight: &LineOfSight,
    actor_line_of_sight: &LineOfSight,
) -> bool {
    viewer_entity == actor_entity
        || viewer_line_of_sight.0.contains(&actor_entity)
        || actor_line_of_sight.0.contains(&viewer_entity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_actions_are_visible_from_either_interest_set() {
        let mut world = World::new();
        let actor = world.spawn_empty().id();
        let viewer = world.spawn_empty().id();
        let empty = LineOfSight::default();

        assert!(should_receive_player_action(actor, actor, &empty, &empty));
        assert!(should_receive_player_action(
            viewer,
            actor,
            &LineOfSight(vec![actor]),
            &empty,
        ));
        assert!(should_receive_player_action(
            viewer,
            actor,
            &empty,
            &LineOfSight(vec![viewer]),
        ));
        assert!(!should_receive_player_action(viewer, actor, &empty, &empty,));
    }
}
