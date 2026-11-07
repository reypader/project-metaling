use crate::{Effect, HealShow, LevelValue, Who};

/// An effect inside a definition, with per-level numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectSpec {
    WeaponDamage {
        ratio: LevelValue,
        hit_pct: LevelValue,
        div: LevelValue,
    },
    /// A negative resolved amount heals 0.
    Heal {
        to: Who,
        hp: LevelValue,
        sp: LevelValue,
        show: HealShow,
    },
}

impl EffectSpec {
    /// The effect at `level`.
    pub fn resolve(self, level: u8) -> Effect {
        let amount = |value: LevelValue| u32::try_from(value.at(level)).unwrap_or(0);
        match self {
            Self::WeaponDamage {
                ratio,
                hit_pct,
                div,
            } => Effect::WeaponDamage {
                ratio: ratio.at(level),
                hit_pct: hit_pct.at(level),
                div: i16::try_from(div.at(level)).unwrap_or(i16::MAX),
            },
            Self::Heal { to, hp, sp, show } => Effect::Heal {
                to,
                hp: amount(hp),
                sp: amount(sp),
                show,
            },
        }
    }
}
