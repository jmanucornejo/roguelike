use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::components::MonsterKind;

pub const MAX_BASE_LEVEL: u16 = 99;
const BASE_EXPERIENCE_PER_LEVEL: u64 = 100;
const NOVICE_JOB_EXPERIENCE_PER_LEVEL: u64 = 40;
const FIRST_JOB_EXPERIENCE_PER_LEVEL: u64 = 75;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Reflect, Serialize, Deserialize)]
#[repr(u16)]
pub enum CharacterClass {
    #[default]
    Novice = 0,
    Swordsman = 1,
    Mage = 2,
    Archer = 3,
    Acolyte = 4,
    Merchant = 5,
    Thief = 6,
    QollqaKamayuq = 7,
    ChacraKamayuq = 8,
    LlamaMichiq = 9,
    Mitmaq = 10,
    Yana = 11,
    Awqaq = 12,
    RunaSimiKamayuq = 13,
    Conquistador = 14,
    Encomendero = 15,
    Corregidor = 16,
    Virrey = 17,
    Oidor = 18,
    Escribano = 19,
    Alguacil = 20,
    Visitador = 21,
    Doctrinero = 22,
    Fraile = 23,
    Hacendado = 24,
    Estanciero = 25,
    Minero = 26,
    Azoguero = 27,
    Arriero = 28,
    Mercader = 29,
    Pulpero = 30,
    Artesano = 31,
    MaestroDeOficio = 32,
    SoldadoDePresidio = 33,
    Marinero = 34,
    Mayordomo = 35,
    Capataz = 36,
}

impl CharacterClass {
    pub const PLACEHOLDERS: [Self; 37] = [
        Self::Novice,
        Self::Swordsman,
        Self::Mage,
        Self::Archer,
        Self::Acolyte,
        Self::Merchant,
        Self::Thief,
        Self::QollqaKamayuq,
        Self::ChacraKamayuq,
        Self::LlamaMichiq,
        Self::Mitmaq,
        Self::Yana,
        Self::Awqaq,
        Self::RunaSimiKamayuq,
        Self::Conquistador,
        Self::Encomendero,
        Self::Corregidor,
        Self::Virrey,
        Self::Oidor,
        Self::Escribano,
        Self::Alguacil,
        Self::Visitador,
        Self::Doctrinero,
        Self::Fraile,
        Self::Hacendado,
        Self::Estanciero,
        Self::Minero,
        Self::Azoguero,
        Self::Arriero,
        Self::Mercader,
        Self::Pulpero,
        Self::Artesano,
        Self::MaestroDeOficio,
        Self::SoldadoDePresidio,
        Self::Marinero,
        Self::Mayordomo,
        Self::Capataz,
    ];

    pub fn id(self) -> u16 {
        self as u16
    }

    pub fn from_id(id: u16) -> Option<Self> {
        Self::PLACEHOLDERS
            .into_iter()
            .find(|class| class.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Novice => "Chasqui",
            Self::Swordsman => "Quipucamayoc",
            Self::Mage => "Amauta",
            Self::Archer => "Haravicu",
            Self::Acolyte => "Curaca",
            Self::Merchant => "Willac Umu",
            Self::Thief => "Aclla",
            Self::QollqaKamayuq => "Qollqa kamayuq",
            Self::ChacraKamayuq => "Chacra kamayuq",
            Self::LlamaMichiq => "Llama michiq",
            Self::Mitmaq => "Mitmaq",
            Self::Yana => "Yana",
            Self::Awqaq => "Awqaq",
            Self::RunaSimiKamayuq => "Runa simi kamayuq",
            Self::Conquistador => "Conquistador",
            Self::Encomendero => "Encomendero",
            Self::Corregidor => "Corregidor",
            Self::Virrey => "Virrey",
            Self::Oidor => "Oidor",
            Self::Escribano => "Escribano",
            Self::Alguacil => "Alguacil",
            Self::Visitador => "Visitador",
            Self::Doctrinero => "Doctrinero",
            Self::Fraile => "Fraile",
            Self::Hacendado => "Hacendado",
            Self::Estanciero => "Estanciero",
            Self::Minero => "Minero",
            Self::Azoguero => "Azoguero",
            Self::Arriero => "Arriero",
            Self::Mercader => "Mercader",
            Self::Pulpero => "Pulpero",
            Self::Artesano => "Artesano",
            Self::MaestroDeOficio => "Maestro de oficio",
            Self::SoldadoDePresidio => "Soldado de presidio",
            Self::Marinero => "Marinero",
            Self::Mayordomo => "Mayordomo",
            Self::Capataz => "Capataz",
        }
    }

    pub fn max_job_level(self) -> u16 {
        match self {
            Self::Novice => 10,
            _ => 50,
        }
    }

    pub fn next_placeholder(self) -> Self {
        let index = Self::PLACEHOLDERS
            .iter()
            .position(|class| *class == self)
            .unwrap_or_default();
        Self::PLACEHOLDERS[(index + 1) % Self::PLACEHOLDERS.len()]
    }
}

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

    /// Removes one percent of the EXP currently accumulated toward the next
    /// level. Any non-zero balance loses at least one point.
    pub fn apply_death_penalty(&mut self) -> u64 {
        let lost = self.experience.div_ceil(100);
        self.experience = self.experience.saturating_sub(lost);
        lost
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Reflect, Serialize, Deserialize)]
pub struct JobProgression {
    pub class: CharacterClass,
    pub level: u16,
    /// Experience accumulated toward the next job level for the current class.
    pub experience: u64,
}

impl Default for JobProgression {
    fn default() -> Self {
        Self {
            class: CharacterClass::Novice,
            level: 1,
            experience: 0,
        }
    }
}

impl JobProgression {
    pub fn from_persisted(class_id: u16, level: u16, experience: u64) -> Self {
        let class = CharacterClass::from_id(class_id).unwrap_or_default();
        let level = level.clamp(1, class.max_job_level());
        Self {
            class,
            level,
            experience: if level == class.max_job_level() {
                0
            } else {
                experience
            },
        }
    }

    pub fn experience_to_next_level(&self) -> Option<u64> {
        experience_to_next_job_level(self.class, self.level)
    }

    pub fn grant_experience(&mut self, amount: u64) -> ExperienceGain {
        if amount == 0 || self.level >= self.class.max_job_level() {
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
        if self.level >= self.class.max_job_level() {
            self.experience = 0;
        }

        ExperienceGain {
            amount,
            levels_gained: self.level - starting_level,
        }
    }

    pub fn change_class(&mut self, class: CharacterClass) -> bool {
        if self.class == class {
            return false;
        }
        self.class = class;
        self.level = 1;
        self.experience = 0;
        true
    }
}

pub fn experience_to_next_base_level(level: u16) -> Option<u64> {
    (level < MAX_BASE_LEVEL)
        .then(|| u64::from(level.max(1)).saturating_mul(BASE_EXPERIENCE_PER_LEVEL))
}

pub fn experience_to_next_job_level(class: CharacterClass, level: u16) -> Option<u64> {
    if level >= class.max_job_level() {
        return None;
    }
    let experience_per_level = match class {
        CharacterClass::Novice => NOVICE_JOB_EXPERIENCE_PER_LEVEL,
        _ => FIRST_JOB_EXPERIENCE_PER_LEVEL,
    };
    Some(u64::from(level.max(1)).saturating_mul(experience_per_level))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExperienceGain {
    pub amount: u64,
    pub levels_gained: u16,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExperienceReward {
    pub base: u64,
    pub job: u64,
}

impl ExperienceReward {
    pub fn for_monster_kind(kind: &MonsterKind) -> Self {
        match kind {
            MonsterKind::Pig => Self { base: 50, job: 30 },
            MonsterKind::Orc => Self { base: 120, job: 80 },
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
    fn death_penalty_removes_one_percent_without_reducing_the_level() {
        let mut progression = BaseProgression {
            level: 12,
            experience: 1_050,
        };

        assert_eq!(progression.apply_death_penalty(), 11);
        assert_eq!(progression.level, 12);
        assert_eq!(progression.experience, 1_039);
    }

    #[test]
    fn death_penalty_is_zero_only_when_no_experience_is_available() {
        let mut one_point = BaseProgression {
            level: 2,
            experience: 1,
        };
        let mut empty = BaseProgression::default();

        assert_eq!(one_point.apply_death_penalty(), 1);
        assert_eq!(one_point.experience, 0);
        assert_eq!(empty.apply_death_penalty(), 0);
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
            ExperienceReward { base: 50, job: 30 }
        );
        assert_eq!(
            ExperienceReward::for_monster_kind(&MonsterKind::Orc),
            ExperienceReward { base: 120, job: 80 }
        );
    }

    #[test]
    fn base_and_job_progress_independently() {
        let mut base = BaseProgression::default();
        let mut job = JobProgression::default();

        base.grant_experience(50);
        job.grant_experience(50);

        assert_eq!(base.level, 1);
        assert_eq!(base.experience, 50);
        assert_eq!(job.level, 2);
        assert_eq!(job.experience, 10);
    }

    #[test]
    fn changing_class_resets_only_job_progress() {
        let mut job = JobProgression {
            class: CharacterClass::Novice,
            level: 7,
            experience: 12,
        };

        assert!(job.change_class(CharacterClass::Mage));
        assert_eq!(
            job,
            JobProgression {
                class: CharacterClass::Mage,
                level: 1,
                experience: 0,
            }
        );
    }

    #[test]
    fn persisted_unknown_classes_safely_fall_back_to_novice() {
        assert_eq!(
            JobProgression::from_persisted(999, 4, 20).class,
            CharacterClass::Novice
        );
    }

    #[test]
    fn placeholder_roster_uses_every_provided_inca_and_spanish_name() {
        let names = CharacterClass::PLACEHOLDERS.map(CharacterClass::name);

        assert_eq!(names.len(), 37);
        assert_eq!(
            &names[..7],
            [
                "Chasqui",
                "Quipucamayoc",
                "Amauta",
                "Haravicu",
                "Curaca",
                "Willac Umu",
                "Aclla",
            ]
        );
        assert!(names.contains(&"Runa simi kamayuq"));
        assert!(names.contains(&"Conquistador"));
        assert!(names.contains(&"Maestro de oficio"));
        assert_eq!(names.last(), Some(&"Capataz"));
    }

    #[test]
    fn existing_placeholder_ids_remain_stable() {
        assert_eq!(CharacterClass::Novice.id(), 0);
        assert_eq!(CharacterClass::Thief.id(), 6);
        assert_eq!(CharacterClass::QollqaKamayuq.id(), 7);
        assert_eq!(CharacterClass::Capataz.id(), 36);
    }
}
