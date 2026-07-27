use sqlx::FromRow;

use crate::shared::gameplay::components::{CharacterId, Facing, Health, Mana};
use crate::shared::gameplay::progression::BaseProgression;
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

    pub strength: u16,
    pub agility: u16,
    pub vitality: u16,
    pub intelligence: u16,
    pub dexterity: u16,
    pub luck: u16,
    pub status_points: u32,
    pub skill_points: u32,

    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub zeny: u64,

    pub map_name: String,
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
    pub base_level: u16,
    pub base_experience: u64,
    pub job_level: u16,
    pub job_experience: u64,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub zeny: u64,
    pub map_name: String,
    pub position: [f32; 3],
    pub facing: u8,
    pub expected_revision: u64,
}

impl CharacterSnapshot {
    pub fn from_record(record: &CharacterRecord) -> Self {
        Self {
            character_id: CharacterId(record.id),
            base_level: record.base_level,
            base_experience: record.base_experience,
            job_level: record.job_level,
            job_experience: record.job_experience,
            hp: record.hp,
            max_hp: record.max_hp,
            sp: record.sp,
            max_sp: record.max_sp,
            zeny: record.zeny,
            map_name: record.map_name.clone(),
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
/// components yet. Health, mana, facing, and position remain authoritative ECS
/// components and are folded into a snapshot when the character is saved.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PersistentCharacter {
    pub account_id: AccountId,
    pub revision: u64,
    pub job_level: u16,
    pub job_experience: u64,
    pub zeny: u64,
    pub map_name: String,
}

impl PersistentCharacter {
    pub fn from_record(record: &CharacterRecord) -> Self {
        Self {
            account_id: AccountId(record.account_id),
            revision: record.revision,
            job_level: record.job_level,
            job_experience: record.job_experience,
            zeny: record.zeny,
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
        progression: &BaseProgression,
    ) -> CharacterSnapshot {
        CharacterSnapshot {
            character_id,
            base_level: progression.level,
            base_experience: progression.experience,
            job_level: self.job_level,
            job_experience: self.job_experience,
            hp: health.current,
            max_hp: health.max,
            sp: mana.current,
            max_sp: mana.max,
            zeny: self.zeny,
            map_name: self.map_name.clone(),
            position: transform.translation.into(),
            facing: facing.0,
            expected_revision: self.revision,
        }
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
            job_level: 8,
            job_experience: 89,
            zeny: 1_500,
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
            &BaseProgression {
                level: 12,
                experience: 345,
            },
        );

        assert_eq!(snapshot.character_id, CharacterId(42));
        assert_eq!(snapshot.position, [1.5, 2.0, -3.5]);
        assert_eq!(snapshot.facing, 6);
        assert_eq!(snapshot.hp, 31);
        assert_eq!(snapshot.sp, 7);
        assert_eq!(snapshot.base_level, 12);
        assert_eq!(snapshot.base_experience, 345);
        assert_eq!(snapshot.expected_revision, 3);
    }
}
