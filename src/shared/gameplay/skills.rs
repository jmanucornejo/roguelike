use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

use super::progression::CharacterClass;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SkillId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkillRequirement {
    pub skill_id: SkillId,
    pub rank: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkillDefinition {
    pub id: SkillId,
    pub class: CharacterClass,
    pub name: &'static str,
    pub description: &'static str,
    pub max_rank: u8,
    pub prerequisite: Option<SkillRequirement>,
}

const fn skill(
    id: u16,
    class: CharacterClass,
    name: &'static str,
    description: &'static str,
    max_rank: u8,
    prerequisite: Option<SkillRequirement>,
) -> SkillDefinition {
    SkillDefinition {
        id: SkillId(id),
        class,
        name,
        description,
        max_rank,
        prerequisite,
    }
}

const fn requires(skill_id: u16, rank: u8) -> Option<SkillRequirement> {
    Some(SkillRequirement {
        skill_id: SkillId(skill_id),
        rank,
    })
}

pub const SKILL_DEFINITIONS: [SkillDefinition; 21] = [
    skill(
        100,
        CharacterClass::Novice,
        "First Aid",
        "Placeholder: improve emergency self-healing.",
        5,
        None,
    ),
    skill(
        101,
        CharacterClass::Novice,
        "Battle Lessons",
        "Placeholder: increase basic weapon confidence.",
        5,
        requires(100, 2),
    ),
    skill(
        102,
        CharacterClass::Novice,
        "Adventurer Spirit",
        "Placeholder: unlock a short all-purpose combat boost.",
        3,
        requires(101, 3),
    ),
    skill(
        200,
        CharacterClass::Swordsman,
        "Sword Mastery",
        "Placeholder: increase damage dealt with swords.",
        5,
        None,
    ),
    skill(
        201,
        CharacterClass::Swordsman,
        "Bash",
        "Placeholder: deliver a powerful single-target strike.",
        5,
        requires(200, 2),
    ),
    skill(
        202,
        CharacterClass::Swordsman,
        "Provoke",
        "Placeholder: lower an enemy's defense and draw aggro.",
        3,
        requires(201, 3),
    ),
    skill(
        300,
        CharacterClass::Mage,
        "Bolt Studies",
        "Placeholder: improve elemental bolt spell damage.",
        5,
        None,
    ),
    skill(
        301,
        CharacterClass::Mage,
        "Fire Ball",
        "Placeholder: launch an explosive ground-area spell.",
        5,
        requires(300, 2),
    ),
    skill(
        302,
        CharacterClass::Mage,
        "Mana Focus",
        "Placeholder: briefly reduce cast time and SP costs.",
        3,
        requires(301, 3),
    ),
    skill(
        400,
        CharacterClass::Archer,
        "Owl's Eye",
        "Placeholder: improve accuracy and ranged damage.",
        5,
        None,
    ),
    skill(
        401,
        CharacterClass::Archer,
        "Double Strafe",
        "Placeholder: fire two arrows at one target.",
        5,
        requires(400, 2),
    ),
    skill(
        402,
        CharacterClass::Archer,
        "Arrow Shower",
        "Placeholder: rain arrows over a target area.",
        3,
        requires(401, 3),
    ),
    skill(
        500,
        CharacterClass::Acolyte,
        "Divine Protection",
        "Placeholder: reduce damage from hostile creatures.",
        5,
        None,
    ),
    skill(
        501,
        CharacterClass::Acolyte,
        "Heal",
        "Placeholder: restore an ally's health.",
        5,
        requires(500, 2),
    ),
    skill(
        502,
        CharacterClass::Acolyte,
        "Blessing",
        "Placeholder: temporarily improve an ally's attributes.",
        3,
        requires(501, 3),
    ),
    skill(
        600,
        CharacterClass::Merchant,
        "Enlarge Weight",
        "Placeholder: increase inventory carrying capacity.",
        5,
        None,
    ),
    skill(
        601,
        CharacterClass::Merchant,
        "Discount",
        "Placeholder: reduce prices charged by NPC shops.",
        5,
        requires(600, 2),
    ),
    skill(
        602,
        CharacterClass::Merchant,
        "Mammonite",
        "Placeholder: spend Gold to deliver a heavy strike.",
        3,
        requires(601, 3),
    ),
    skill(
        700,
        CharacterClass::Thief,
        "Double Attack",
        "Placeholder: gain a chance to strike twice.",
        5,
        None,
    ),
    skill(
        701,
        CharacterClass::Thief,
        "Improve Dodge",
        "Placeholder: increase movement and evasion.",
        5,
        requires(700, 2),
    ),
    skill(
        702,
        CharacterClass::Thief,
        "Envenom",
        "Placeholder: attack with a chance to inflict poison.",
        3,
        requires(701, 3),
    ),
];

pub fn skill_definition(skill_id: SkillId) -> Option<&'static SkillDefinition> {
    SKILL_DEFINITIONS
        .iter()
        .find(|definition| definition.id == skill_id)
}

pub fn skills_for_class(class: CharacterClass) -> impl Iterator<Item = &'static SkillDefinition> {
    SKILL_DEFINITIONS
        .iter()
        .filter(move |definition| definition.class == class)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LearnedSkill {
    pub id: SkillId,
    pub rank: u8,
}

#[derive(Component, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillTree {
    available_points: u32,
    learned: Vec<LearnedSkill>,
}

impl SkillTree {
    pub fn from_persisted(
        class: CharacterClass,
        job_level: u16,
        learned: impl IntoIterator<Item = LearnedSkill>,
    ) -> Self {
        let mut sanitized: Vec<LearnedSkill> = Vec::new();
        for learned_skill in learned {
            let Some(definition) = skill_definition(learned_skill.id) else {
                continue;
            };
            if definition.class != class || learned_skill.rank == 0 {
                continue;
            }
            let rank = learned_skill.rank.min(definition.max_rank);
            if let Some(existing) = sanitized
                .iter_mut()
                .find(|existing| existing.id == learned_skill.id)
            {
                existing.rank = existing.rank.max(rank);
            } else {
                sanitized.push(LearnedSkill {
                    id: learned_skill.id,
                    rank,
                });
            }
        }
        sanitized.sort_by_key(|learned_skill| learned_skill.id);

        let earned_points = u32::from(job_level.saturating_sub(1));
        let spent_points = sanitized
            .iter()
            .map(|learned_skill| u32::from(learned_skill.rank))
            .sum::<u32>();
        Self {
            available_points: earned_points.saturating_sub(spent_points),
            learned: sanitized,
        }
    }

    pub fn available_points(&self) -> u32 {
        self.available_points
    }

    pub fn learned(&self) -> &[LearnedSkill] {
        &self.learned
    }

    pub fn rank(&self, skill_id: SkillId) -> u8 {
        self.learned
            .iter()
            .find(|learned_skill| learned_skill.id == skill_id)
            .map_or(0, |learned_skill| learned_skill.rank)
    }

    pub fn can_spend_point(
        &self,
        class: CharacterClass,
        skill_id: SkillId,
    ) -> Result<(), SkillSpendError> {
        let Some(definition) = skill_definition(skill_id) else {
            return Err(SkillSpendError::UnknownSkill);
        };
        if definition.class != class {
            return Err(SkillSpendError::WrongClass);
        }
        if self.available_points == 0 {
            return Err(SkillSpendError::NoAvailablePoints);
        }
        if self.rank(skill_id) >= definition.max_rank {
            return Err(SkillSpendError::AtMaximumRank);
        }
        if let Some(requirement) = definition.prerequisite {
            let current_rank = self.rank(requirement.skill_id);
            if current_rank < requirement.rank {
                return Err(SkillSpendError::MissingPrerequisite {
                    skill_id: requirement.skill_id,
                    required_rank: requirement.rank,
                    current_rank,
                });
            }
        }
        Ok(())
    }

    pub fn spend_point(
        &mut self,
        class: CharacterClass,
        skill_id: SkillId,
    ) -> Result<u8, SkillSpendError> {
        self.can_spend_point(class, skill_id)?;
        self.available_points -= 1;
        if let Some(learned_skill) = self
            .learned
            .iter_mut()
            .find(|learned_skill| learned_skill.id == skill_id)
        {
            learned_skill.rank += 1;
            return Ok(learned_skill.rank);
        }
        self.learned.push(LearnedSkill {
            id: skill_id,
            rank: 1,
        });
        self.learned.sort_by_key(|learned_skill| learned_skill.id);
        Ok(1)
    }

    pub fn grant_job_levels(&mut self, levels: u16) {
        self.available_points = self.available_points.saturating_add(u32::from(levels));
    }

    pub fn reset(&mut self) {
        self.available_points = 0;
        self.learned.clear();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillSpendError {
    UnknownSkill,
    WrongClass,
    NoAvailablePoints,
    AtMaximumRank,
    MissingPrerequisite {
        skill_id: SkillId,
        required_rank: u8,
        current_rank: u8,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_levels_create_one_skill_point_each_after_job_one() {
        let tree = SkillTree::from_persisted(CharacterClass::Mage, 5, []);

        assert_eq!(tree.available_points(), 4);
    }

    #[test]
    fn prerequisite_ranks_are_enforced_before_spending() {
        let mut tree = SkillTree::from_persisted(CharacterClass::Mage, 10, []);

        assert!(matches!(
            tree.spend_point(CharacterClass::Mage, SkillId(301)),
            Err(SkillSpendError::MissingPrerequisite {
                skill_id: SkillId(300),
                required_rank: 2,
                current_rank: 0,
            })
        ));
        tree.spend_point(CharacterClass::Mage, SkillId(300))
            .unwrap();
        tree.spend_point(CharacterClass::Mage, SkillId(300))
            .unwrap();
        assert_eq!(tree.spend_point(CharacterClass::Mage, SkillId(301)), Ok(1));
    }

    #[test]
    fn persisted_ranks_are_clamped_filtered_and_deducted() {
        let tree = SkillTree::from_persisted(
            CharacterClass::Mage,
            10,
            [
                LearnedSkill {
                    id: SkillId(300),
                    rank: 99,
                },
                LearnedSkill {
                    id: SkillId(200),
                    rank: 2,
                },
            ],
        );

        assert_eq!(tree.rank(SkillId(300)), 5);
        assert_eq!(tree.rank(SkillId(200)), 0);
        assert_eq!(tree.available_points(), 4);
    }

    #[test]
    fn skills_cannot_exceed_their_maximum_rank() {
        let mut tree = SkillTree::from_persisted(
            CharacterClass::Novice,
            10,
            [LearnedSkill {
                id: SkillId(100),
                rank: 5,
            }],
        );

        assert_eq!(
            tree.spend_point(CharacterClass::Novice, SkillId(100)),
            Err(SkillSpendError::AtMaximumRank)
        );
    }
}
