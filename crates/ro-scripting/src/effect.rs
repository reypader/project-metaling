//! The effect vocabulary: per-level specs inside definitions, and the resolved effects the engine
//! applies (DESIGN §2).

use std::ops::Deref;

use ro_core::ItemId;
use smallvec::SmallVec;

/// Which unit an effect lands on, relative to the cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Caster,
    Target,
}

/// How a heal shows on the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealShow {
    /// The skill animation `0x09cb` with the unclamped amount, then the HP/SP pushes.
    Skill,
    /// Only the HP/SP pushes (potions).
    Silent,
}

/// A resolved effect, in the order the engine applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// A physical hit on the target through `ro_mechanics::calc_weapon_attack`: the skill ratio
    /// in percent, the hit-rate modifier in percent, and the hits shown.
    WeaponDamage {
        ratio: i32,
        hit_pct: i32,
        div: i16,
    },
    /// Restores HP and SP. The amounts are unclamped; the engine clamps to the max.
    Heal {
        to: Who,
        hp: u32,
        sp: u32,
        show: HealShow,
    },
    SpendSp(u32),
    SpendHp(u32),
    SpendZeny(u32),
    ConsumeItem {
        item: ItemId,
        amount: u32,
    },
}

/// The caller-owned output of a resolve. Reused between casts, so the hot path doesn't
/// allocate for short lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectList(SmallVec<[Effect; 8]>);

impl EffectList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, effect: Effect) {
        self.0.push(effect);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Keeps the first `len` effects.
    pub fn truncate(&mut self, len: usize) {
        self.0.truncate(len);
    }
}

impl Deref for EffectList {
    type Target = [Effect];

    fn deref(&self) -> &[Effect] {
        &self.0
    }
}

/// A per-level number: `base + per_level × level`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelValue {
    pub base: i32,
    pub per_level: i32,
}

impl LevelValue {
    /// The same value at every level.
    pub const fn fixed(value: i32) -> Self {
        Self {
            base: value,
            per_level: 0,
        }
    }

    pub const fn new(base: i32, per_level: i32) -> Self {
        Self { base, per_level }
    }

    /// The value at `level`, saturating.
    pub fn at(self, level: u8) -> i32 {
        self.base
            .saturating_add(self.per_level.saturating_mul(i32::from(level)))
    }
}

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
