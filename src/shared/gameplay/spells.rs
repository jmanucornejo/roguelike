use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellDefinition {
    pub cast_time: Duration,
    pub targeting: SpellTargeting,
    pub damage: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellTargeting {
    Ground,
    DirectMonster,
}

pub fn spell_definition(spell_id: u16) -> Option<SpellDefinition> {
    let definition = match spell_id {
        1 => SpellDefinition {
            cast_time: Duration::ZERO,
            targeting: SpellTargeting::Ground,
            damage: None,
        },
        2 => SpellDefinition {
            cast_time: Duration::from_secs(4),
            targeting: SpellTargeting::Ground,
            damage: None,
        },
        3 => SpellDefinition {
            cast_time: Duration::from_secs(3),
            targeting: SpellTargeting::DirectMonster,
            damage: Some(20),
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
                damage: Some(20),
            })
        );
    }

    #[test]
    fn unknown_spells_are_rejected() {
        assert_eq!(spell_definition(99), None);
    }
}
