use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellDefinition {
    pub cast_time: Duration,
    pub targeting: SpellTargeting,
    pub max_range: Option<u32>,
    pub effect: SpellEffect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellTargeting {
    GroundArea,
    DirectMonster,
    SelfOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellEffect {
    None,
    Damage {
        amount: u32,
        area_radius: Option<u32>,
    },
    AttackSpeedBuff {
        duration: Duration,
        attack_period_percent: u8,
    },
}

pub fn spell_definition(spell_id: u16) -> Option<SpellDefinition> {
    let definition = match spell_id {
        1 => SpellDefinition {
            cast_time: Duration::ZERO,
            targeting: SpellTargeting::GroundArea,
            max_range: Some(12),
            effect: SpellEffect::None,
        },
        2 => SpellDefinition {
            cast_time: Duration::from_secs(4),
            targeting: SpellTargeting::GroundArea,
            max_range: Some(12),
            effect: SpellEffect::Damage {
                amount: 15,
                area_radius: Some(3),
            },
        },
        3 => SpellDefinition {
            cast_time: Duration::from_secs(3),
            targeting: SpellTargeting::DirectMonster,
            max_range: Some(12),
            effect: SpellEffect::Damage {
                amount: 20,
                area_radius: None,
            },
        },
        4 => SpellDefinition {
            cast_time: Duration::ZERO,
            targeting: SpellTargeting::SelfOnly,
            max_range: None,
            effect: SpellEffect::AttackSpeedBuff {
                duration: Duration::from_secs(10),
                attack_period_percent: 70,
            },
        },
        _ => return None,
    };

    Some(definition)
}

pub fn spell_cooldown(attack_period_seconds: f32) -> Duration {
    Duration::from_secs_f32(attack_period_seconds.max(0.001))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_spell_has_a_four_second_cast_time() {
        assert_eq!(
            spell_definition(2).unwrap().cast_time,
            Duration::from_secs(4)
        );
    }

    #[test]
    fn first_spell_is_instant() {
        assert_eq!(spell_definition(1).unwrap().cast_time, Duration::ZERO);
    }

    #[test]
    fn third_spell_is_a_three_second_direct_damage_spell() {
        assert_eq!(
            spell_definition(3),
            Some(SpellDefinition {
                cast_time: Duration::from_secs(3),
                targeting: SpellTargeting::DirectMonster,
                max_range: Some(12),
                effect: SpellEffect::Damage {
                    amount: 20,
                    area_radius: None,
                },
            })
        );
    }

    #[test]
    fn second_spell_is_a_three_unit_ground_area_attack() {
        assert_eq!(
            spell_definition(2).unwrap().effect,
            SpellEffect::Damage {
                amount: 15,
                area_radius: Some(3),
            }
        );
    }

    #[test]
    fn fourth_spell_is_an_instant_ten_second_self_buff() {
        assert_eq!(
            spell_definition(4),
            Some(SpellDefinition {
                cast_time: Duration::ZERO,
                targeting: SpellTargeting::SelfOnly,
                max_range: None,
                effect: SpellEffect::AttackSpeedBuff {
                    duration: Duration::from_secs(10),
                    attack_period_percent: 70,
                },
            })
        );
    }

    #[test]
    fn unknown_spells_are_rejected() {
        assert_eq!(spell_definition(99), None);
    }
}
