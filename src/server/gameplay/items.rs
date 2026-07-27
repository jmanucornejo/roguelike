use std::{f32::consts::TAU, time::Duration};

use bevy::prelude::*;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use bevy_renet::RenetServer;
use rand::Rng;

use crate::{
    server::{
        gameplay::{
            pathing::get_path_between_translations, spatial::NearestNeighbourComponent,
            spells::AuthoritativeCast,
        },
        network::replication::LineOfSight,
        persistence::{PersistenceClient, PersistenceRequest},
        state::CharacterPersistenceQueue,
    },
    shared::{
        gameplay::{
            components::{
                Aggro, Attacking, AttackingTimer, CharacterId, Health, Map, Monster, MonsterKind,
                Walking,
            },
            entities::Player,
            events::DeathEvent,
            items::{
                item_definition, ConsumableEffect, GroundItem, Inventory, ItemDefinitionId,
                GROUND_ITEM_VISUAL_HALF_HEIGHT, LUCKY_CLOVER, PIG_MEAT, RED_HERB,
            },
        },
        network::{channels::ServerChannel, messages::ServerMessages},
        states::ServerState,
    },
};

const CHANCE_SCALE: u16 = 10_000;
const GROUND_ITEM_LIFETIME: Duration = Duration::from_secs(60);
const DROP_MIN_RADIUS: f32 = 0.25;
const DROP_MAX_RADIUS: f32 = 1.25;
const PICKUP_RANGE: f32 = 2.5;
const TERRAIN_PROBE_HEIGHT: f32 = 64.0;

#[derive(Clone, Copy, Debug)]
struct LootEntry {
    item_id: ItemDefinitionId,
    chance_basis_points: u16,
}

const PIG_LOOT: [LootEntry; 3] = [
    LootEntry {
        item_id: PIG_MEAT,
        chance_basis_points: 10_000,
    },
    LootEntry {
        item_id: RED_HERB,
        chance_basis_points: 5_000,
    },
    LootEntry {
        item_id: LUCKY_CLOVER,
        chance_basis_points: 2_500,
    },
];

#[derive(Component, Debug, Default)]
struct PickupClaim(bool);

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct PendingItemPickup {
    pub(crate) ground_item: Entity,
}

impl PickupClaim {
    fn claim(&mut self) -> bool {
        if self.0 {
            return false;
        }
        self.0 = true;
        true
    }
}

#[derive(Component, Deref, DerefMut)]
struct GroundItemLifetime(Timer);

#[derive(Event, Debug)]
pub struct RequestItemPickup {
    pub player: Entity,
    pub ground_item: Entity,
}

#[derive(Event, Debug)]
pub struct RequestItemUse {
    pub player: Entity,
    pub item_id: ItemDefinitionId,
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_loot_on_death)
            .add_observer(begin_ground_item_pickup)
            .add_observer(use_inventory_item)
            .add_systems(
                Update,
                (complete_pending_item_pickups, expire_ground_items)
                    .run_if(in_state(ServerState::InGame)),
            );
    }
}

fn use_inventory_item(
    trigger: On<RequestItemUse>,
    mut server: ResMut<RenetServer>,
    mut players: Query<(Entity, &Player, &CharacterId, &mut Inventory, &mut Health)>,
    persistence: Option<Res<PersistenceClient>>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
) {
    let request = trigger.event();
    let Some(definition) = item_definition(request.item_id) else {
        return;
    };
    let Some(effect) = definition.consumable else {
        return;
    };
    let Ok((player_entity, player, character_id, mut inventory, mut health)) =
        players.get_mut(request.player)
    else {
        return;
    };

    // Removing the stack is the authoritative gate. The item is consumed even
    // when its effect cannot change the character, such as healing at full HP.
    if !inventory.remove(request.item_id, 1) {
        return;
    }

    match effect {
        ConsumableEffect::RestoreHealth(amount) => {
            let (new_health, restored) = restore_health(health.current, health.max, amount);
            if restored > 0 {
                health.current = new_health;
            }
            info!(
                "Character {} consumed {} and restored {} HP",
                character_id.0, definition.name, restored
            );
        }
    }

    let inventory_message = bincode::serialize(&ServerMessages::InventoryUpdated {
        entity: player_entity,
        inventory: inventory.clone(),
    })
    .expect("inventory update should serialize");
    server.send_message(player.id, ServerChannel::ServerMessages, inventory_message);

    if character_id.0 != 0 {
        if let Some(persistence) = persistence.as_deref() {
            let request_id = persistence_queue.next_request_id();
            if let Err(error) = persistence.send(PersistenceRequest::RemoveInventoryItem {
                request_id,
                character_id: *character_id,
                item_id: request.item_id,
                quantity: 1,
            }) {
                error!(
                    "Could not persist consumption of item {} for character {}: {}",
                    request.item_id.0, character_id.0, error
                );
            }
        }
    }
}

fn restore_health(current: u32, max: u32, amount: u32) -> (u32, u32) {
    let restored = max.saturating_sub(current).min(amount);
    (current.saturating_add(restored).min(max), restored)
}

fn loot_table(kind: &MonsterKind) -> &'static [LootEntry] {
    match kind {
        MonsterKind::Pig => &PIG_LOOT,
        MonsterKind::Orc => &[],
    }
}

fn roll_succeeds(chance_basis_points: u16, roll: u16) -> bool {
    roll < chance_basis_points.min(CHANCE_SCALE)
}

fn random_drop_offset(rng: &mut impl Rng) -> Vec3 {
    let angle = rng.gen_range(0.0..TAU);
    let radius = rng.gen_range(DROP_MIN_RADIUS..=DROP_MAX_RADIUS);
    Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
}

fn spawn_loot_on_death(
    trigger: On<DeathEvent>,
    monsters: Query<(&Monster, &Transform)>,
    read_rapier_context: ReadRapierContext,
    mut commands: Commands,
) {
    let Ok((monster, transform)) = monsters.get(trigger.event().entity) else {
        return;
    };

    let mut rng = rand::thread_rng();
    let rapier_context = read_rapier_context.single().ok();
    for entry in loot_table(&monster.kind) {
        if !roll_succeeds(entry.chance_basis_points, rng.gen_range(0..CHANCE_SCALE)) {
            continue;
        }

        let mut translation = transform.translation + random_drop_offset(&mut rng);
        let probe_origin = translation + Vec3::Y * (TERRAIN_PROBE_HEIGHT * 0.5);
        let floor_y = rapier_context
            .as_ref()
            .and_then(|context| {
                context
                    .cast_ray(
                        probe_origin,
                        Vec3::NEG_Y,
                        TERRAIN_PROBE_HEIGHT,
                        true,
                        QueryFilter::only_fixed().exclude_sensors(),
                    )
                    .map(|(_, time_of_impact)| probe_origin.y - time_of_impact)
            })
            .unwrap_or(transform.translation.y);
        translation.y = floor_y + GROUND_ITEM_VISUAL_HALF_HEIGHT;
        commands.spawn((
            Transform::from_translation(translation),
            GroundItem {
                item_id: entry.item_id,
                quantity: 1,
            },
            PickupClaim::default(),
            GroundItemLifetime(Timer::new(GROUND_ITEM_LIFETIME, TimerMode::Once)),
            NearestNeighbourComponent,
            Name::new("Ground item"),
        ));
    }
}

fn begin_ground_item_pickup(
    trigger: On<RequestItemPickup>,
    mut commands: Commands,
    map: Res<Map>,
    drops: Query<&Transform, With<GroundItem>>,
    players: Query<(&Transform, Option<&AuthoritativeCast>), With<Player>>,
) {
    let request = trigger.event();
    let Ok((player_transform, active_cast)) = players.get(request.player) else {
        return;
    };
    if active_cast.is_some() {
        return;
    }
    let Ok(item_transform) = drops.get(request.ground_item) else {
        return;
    };

    let mut player_commands = commands.entity(request.player);
    player_commands
        .remove::<Aggro>()
        .remove::<Attacking>()
        .remove::<AttackingTimer>()
        .insert(PendingItemPickup {
            ground_item: request.ground_item,
        });

    if !is_within_pickup_range(player_transform.translation, item_transform.translation) {
        let Some(path) = get_path_between_translations(
            player_transform.translation,
            item_transform.translation,
            &map,
        ) else {
            player_commands.remove::<PendingItemPickup>();
            return;
        };
        player_commands.insert(Walking {
            target_translation: item_transform.translation,
            path: Some(path),
        });
    }
}

fn is_within_pickup_range(player: Vec3, item: Vec3) -> bool {
    player.distance_squared(item) <= PICKUP_RANGE * PICKUP_RANGE
}

fn complete_pending_item_pickups(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    mut drops: Query<(&GroundItem, &Transform, &mut PickupClaim)>,
    mut players: Query<(
        Entity,
        &Player,
        &Transform,
        &CharacterId,
        &mut Inventory,
        &PendingItemPickup,
    )>,
    viewers: Query<(Entity, &Player, &LineOfSight)>,
    persistence: Option<Res<PersistenceClient>>,
    mut persistence_queue: ResMut<CharacterPersistenceQueue>,
) {
    for (player_entity, player, player_transform, character_id, mut inventory, pending) in
        &mut players
    {
        let Ok((item, item_transform, mut claim)) = drops.get_mut(pending.ground_item) else {
            commands
                .entity(player_entity)
                .remove::<PendingItemPickup>()
                .remove::<Walking>();
            continue;
        };

        if !is_within_pickup_range(player_transform.translation, item_transform.translation) {
            continue;
        }
        if !claim.claim() {
            commands
                .entity(player_entity)
                .remove::<PendingItemPickup>()
                .remove::<Walking>();
            continue;
        }

        inventory.add(item.item_id, item.quantity);
        let inventory_message = bincode::serialize(&ServerMessages::InventoryUpdated {
            entity: player_entity,
            inventory: inventory.clone(),
        })
        .expect("inventory update should serialize");
        server.send_message(player.id, ServerChannel::ServerMessages, inventory_message);

        if character_id.0 != 0 {
            if let Some(persistence) = persistence.as_deref() {
                let request_id = persistence_queue.next_request_id();
                if let Err(error) = persistence.send(PersistenceRequest::AddInventoryItem {
                    request_id,
                    character_id: *character_id,
                    item_id: item.item_id,
                    quantity: item.quantity,
                }) {
                    error!(
                        "Could not persist item {} for character {}: {}",
                        item.item_id.0, character_id.0, error
                    );
                }
            }
        }

        announce_despawn(
            &mut server,
            &viewers,
            pending.ground_item,
            Some(player_entity),
        );
        commands
            .entity(player_entity)
            .remove::<PendingItemPickup>()
            .remove::<Walking>();
        commands.entity(pending.ground_item).try_despawn();
    }
}

fn expire_ground_items(
    mut commands: Commands,
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
    mut drops: Query<(Entity, &mut GroundItemLifetime)>,
    viewers: Query<(Entity, &Player, &LineOfSight)>,
) {
    for (entity, mut lifetime) in &mut drops {
        lifetime.tick(time.delta());
        if !lifetime.is_finished() {
            continue;
        }

        announce_despawn(&mut server, &viewers, entity, None);
        commands.entity(entity).try_despawn();
    }
}

fn announce_despawn(
    server: &mut RenetServer,
    viewers: &Query<(Entity, &Player, &LineOfSight)>,
    entity: Entity,
    always_notify: Option<Entity>,
) {
    let message = bincode::serialize(&ServerMessages::DespawnEntity { entity })
        .expect("ground item despawn should serialize");
    for (viewer_entity, player, line_of_sight) in viewers {
        if Some(viewer_entity) == always_notify || line_of_sight.0.contains(&entity) {
            server.send_message(player.id, ServerChannel::ServerMessages, message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn loot_chances_use_exact_basis_point_boundaries() {
        assert!(roll_succeeds(10_000, 9_999));
        assert!(roll_succeeds(5_000, 4_999));
        assert!(!roll_succeeds(5_000, 5_000));
        assert!(roll_succeeds(2_500, 2_499));
        assert!(!roll_succeeds(2_500, 2_500));
    }

    #[test]
    fn random_drop_offsets_stay_inside_the_configured_ring() {
        let mut rng = StdRng::seed_from_u64(123);
        for _ in 0..100 {
            let distance = random_drop_offset(&mut rng).length();
            assert!((DROP_MIN_RADIUS..=DROP_MAX_RADIUS).contains(&distance));
        }
    }

    #[test]
    fn a_ground_item_can_only_be_claimed_once() {
        let mut claim = PickupClaim::default();
        assert!(claim.claim());
        assert!(!claim.claim());
    }

    #[test]
    fn pickup_range_includes_the_boundary_but_not_positions_beyond_it() {
        assert!(is_within_pickup_range(Vec3::ZERO, Vec3::X * PICKUP_RANGE));
        assert!(!is_within_pickup_range(
            Vec3::ZERO,
            Vec3::X * (PICKUP_RANGE + 0.01)
        ));
    }

    #[test]
    fn healing_is_capped_but_full_health_still_has_a_valid_zero_effect() {
        assert_eq!(restore_health(20, 40, 10), (30, 10));
        assert_eq!(restore_health(35, 40, 10), (40, 5));
        assert_eq!(restore_health(40, 40, 10), (40, 0));
    }
}
