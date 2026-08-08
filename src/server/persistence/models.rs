use sqlx::FromRow;

use crate::shared::gameplay::components::{
    CharacterId, CharacterStats, Equipment, Facing, Gold, Health, Mana, SavePoint,
};
use crate::shared::gameplay::items::equipment_derived_stats;
use crate::shared::gameplay::maps::map_to_local_position;
use crate::shared::gameplay::progression::{BaseProgression, JobProgression};
use bevy::prelude::{Component, Transform};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AccountId(pub u64);

#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct CharacterSummary {
    pub id: u64,
    pub slot: u8,
    pub name: String,
    pub class_id: u16,
    pub base_level: u16,
    pub job_level: u16,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct CharacterRecord {
    pub id: u64,
    pub account_id: u64,
    pub slot: u8,
    pub name: String,
    pub class_id: u16,

    pub base_level: u16,
    pub base_experience: u64,
    pub job_level: u16,
    pub job_experience: u64,

    pub might: u16,
    pub finesse: u16,
    pub agility: u16,
    pub vitality: u16,
    pub intellect: u16,
    pub spirit: u16,
    pub attribute_points: u32,
    pub skill_points: u32,

    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub gold: u64,

    pub map_name: String,
    pub save_map_name: Option<String>,
    pub save_position_x: Option<f32>,
    pub save_position_y: Option<f32>,
    pub save_position_z: Option<f32>,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub facing: u8,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewCharacter {
    pub account_id: AccountId,
    pub slot: u8,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterSnapshot {
    pub character_id: CharacterId,
    pub class_id: u16,
    pub base_level: u16,
    pub base_experience: u64,
    pub job_level: u16,
    pub job_experience: u64,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub gold: u64,
    pub stats: CharacterStats,
    pub map_name: String,
    pub save_point: Option<SavePoint>,
    pub position: [f32; 3],
    pub facing: u8,
    pub expected_revision: u64,
}

impl CharacterSnapshot {
    pub fn from_record(record: &CharacterRecord) -> Self {
        Self {
            character_id: CharacterId(record.id),
            class_id: record.class_id,
            base_level: record.base_level,
            base_experience: record.base_experience,
            job_level: record.job_level,
            job_experience: record.job_experience,
            hp: record.hp,
            max_hp: record.max_hp,
            sp: record.sp,
            max_sp: record.max_sp,
            gold: record.gold,
            stats: CharacterStats {
                might: record.might,
                finesse: record.finesse,
                agility: record.agility,
                vitality: record.vitality,
                intellect: record.intellect,
                spirit: record.spirit,
                available_points: record.attribute_points,
            },
            map_name: record.map_name.clone(),
            save_point: persisted_save_point(record),
            position: [record.position_x, record.position_y, record.position_z],
            facing: record.facing,
            expected_revision: record.revision,
        }
    }

    pub fn without_revision(mut self) -> Self {
        self.expected_revision = 0;
        self
    }
}

/// Database-backed values that are not otherwise represented by gameplay
/// components yet. Gold, health, mana, facing, and position remain authoritative
/// ECS components and are folded into a snapshot when the character is saved.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PersistentCharacter {
    pub account_id: AccountId,
    pub revision: u64,
    pub map_name: String,
}

impl PersistentCharacter {
    pub fn from_record(record: &CharacterRecord) -> Self {
        Self {
            account_id: AccountId(record.account_id),
            revision: record.revision,
            map_name: record.map_name.clone(),
        }
    }

    pub fn snapshot(
        &self,
        character_id: CharacterId,
        transform: &Transform,
        facing: &Facing,
        health: &Health,
        mana: &Mana,
        gold: &Gold,
        stats: &CharacterStats,
        equipment: &Equipment,
        save_point: Option<&SavePoint>,
        progression: &BaseProgression,
        job_progression: &JobProgression,
    ) -> CharacterSnapshot {
        let derived = equipment_derived_stats(stats, progression.level, equipment);
        CharacterSnapshot {
            character_id,
            class_id: job_progression.class.id(),
            base_level: progression.level,
            base_experience: progression.experience,
            job_level: job_progression.level,
            job_experience: job_progression.experience,
            hp: health.current.min(derived.max_health),
            max_hp: derived.max_health,
            sp: mana.current.min(derived.max_mana),
            max_sp: derived.max_mana,
            gold: gold.0,
            stats: *stats,
            map_name: self.map_name.clone(),
            save_point: save_point.cloned(),
            position: map_to_local_position(&self.map_name, transform.translation).into(),
            facing: facing.0,
            expected_revision: self.revision,
        }
    }
}

pub fn persisted_save_point(record: &CharacterRecord) -> Option<SavePoint> {
    match (
        record.save_map_name.clone(),
        record.save_position_x,
        record.save_position_y,
        record.save_position_z,
    ) {
        (Some(map_name), Some(x), Some(y), Some(z)) => Some(SavePoint {
            map_name,
            position: [x, y, z],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec3;

    #[test]
    fn persistent_character_builds_snapshot_from_authoritative_components() {
        let persistent = PersistentCharacter {
            account_id: AccountId(7),
            revision: 3,
            map_name: "prontera".into(),
        };
        let transform = Transform::from_translation(Vec3::new(1.5, 2.0, -3.5));

        let snapshot = persistent.snapshot(
            CharacterId(42),
            &transform,
            &Facing(6),
            &Health {
                current: 31,
                max: 40,
            },
            &Mana {
                current: 7,
                max: 10,
            },
            &Gold(1_500),
            &CharacterStats {
                might: 10,
                finesse: 6,
                agility: 9,
                vitality: 8,
                intellect: 7,
                spirit: 5,
                available_points: 4,
            },
            &Equipment::default(),
            Some(&SavePoint {
                map_name: "prontera".into(),
                position: [4.0, 1.0, 5.0],
            }),
            &BaseProgression {
                level: 12,
                experience: 345,
            },
            &JobProgression {
                class: crate::shared::gameplay::progression::CharacterClass::Mage,
                level: 8,
                experience: 89,
            },
        );

        assert_eq!(snapshot.character_id, CharacterId(42));
        assert_eq!(snapshot.position, [1.5, 2.0, -3.5]);
        assert_eq!(snapshot.facing, 6);
        assert_eq!(snapshot.hp, 31);
        assert_eq!(snapshot.sp, 7);
        assert_eq!(snapshot.gold, 1_500);
        assert_eq!(snapshot.stats.might, 10);
        assert_eq!(snapshot.stats.available_points, 4);
        assert_eq!(
            snapshot.save_point.as_ref().map(|point| point.position),
            Some([4.0, 1.0, 5.0])
        );
        assert_eq!(snapshot.base_level, 12);
        assert_eq!(snapshot.base_experience, 345);
        assert_eq!(snapshot.class_id, 2);
        assert_eq!(snapshot.job_level, 8);
        assert_eq!(snapshot.job_experience, 89);
        assert_eq!(snapshot.expected_revision, 3);
    }
}
