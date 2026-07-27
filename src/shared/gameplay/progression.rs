use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::components::MonsterKind;

pub const MAX_BASE_LEVEL: u16 = 99;
const BASE_EXPERIENCE_PER_LEVEL: u64 = 100;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BaseProgression {
    pub level: u16,
    /// Experience accumulated toward the next base level.
    pub experience: u64,
}

impl Default for BaseProgression {
    fn default() -> Self {
        Self {
            level: 1,
            experience: 0,
        }
    }
}

impl BaseProgression {
    pub fn experience_to_next_level(&self) -> Option<u64> {
        experience_to_next_base_level(self.level)
    }

    pub fn grant_experience(&mut self, amount: u64) -> ExperienceGain {
        if amount == 0 || self.level >= MAX_BASE_LEVEL {
            return ExperienceGain {
                amount,
                levels_gained: 0,
            };
        }

        self.experience = self.experience.saturating_add(amount);
        let starting_level = self.level;

        while let Some(required) = self.experience_to_next_level() {
            if self.experience < required {
                break;
            }

            self.experience -= required;
            self.level += 1;
        }

        if self.level >= MAX_BASE_LEVEL {
            self.experience = 0;
        }

        ExperienceGain {
            amount,
            levels_gained: self.level - starting_level,
        }
    }
}

pub fn experience_to_next_base_level(level: u16) -> Option<u64> {
    (level < MAX_BASE_LEVEL)
        .then(|| u64::from(level.max(1)).saturating_mul(BASE_EXPERIENCE_PER_LEVEL))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExperienceGain {
    pub amount: u64,
    pub levels_gained: u16,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExperienceReward(pub u64);

impl ExperienceReward {
    pub fn for_monster_kind(kind: &MonsterKind) -> Self {
        match kind {
            MonsterKind::Pig => Self(50),
            MonsterKind::Orc => Self(120),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experience_can_cross_multiple_level_thresholds() {
        let mut progression = BaseProgression {
            level: 1,
            experience: 90,
        };

        let gain = progression.grant_experience(250);

        assert_eq!(
            progression,
            BaseProgression {
                level: 3,
                experience: 40,
            }
        );
        assert_eq!(gain.amount, 250);
        assert_eq!(gain.levels_gained, 2);
    }

    #[test]
    fn max_level_does_not_accumulate_more_experience() {
        let mut progression = BaseProgression {
            level: MAX_BASE_LEVEL,
            experience: 0,
        };

        let gain = progression.grant_experience(1_000);

        assert_eq!(progression.experience, 0);
        assert_eq!(gain.levels_gained, 0);
        assert_eq!(progression.experience_to_next_level(), None);
    }

    #[test]
    fn monster_kinds_have_explicit_rewards() {
        assert_eq!(
            ExperienceReward::for_monster_kind(&MonsterKind::Pig),
            ExperienceReward(50)
        );
        assert_eq!(
            ExperienceReward::for_monster_kind(&MonsterKind::Orc),
            ExperienceReward(120)
        );
    }
}
