use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellDefinition {
    pub cast_time: Duration,
}

pub fn spell_definition(spell_id: u16) -> Option<SpellDefinition> {
    let cast_time = match spell_id {
        1 | 3 => Duration::ZERO,
        2 => Duration::from_secs(4),
        _ => return None,
    };

    Some(SpellDefinition { cast_time })
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
    fn first_and_third_spells_are_instant() {
        assert_eq!(spell_definition(1).unwrap().cast_time, Duration::ZERO);
        assert_eq!(spell_definition(3).unwrap().cast_time, Duration::ZERO);
    }

    #[test]
    fn unknown_spells_are_rejected() {
        assert_eq!(spell_definition(99), None);
    }
}
