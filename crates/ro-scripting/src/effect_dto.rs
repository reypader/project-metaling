//! The effect vocabulary: per-level specs inside definitions, and the resolved effects the engine
//! applies (DESIGN §2).

use ro_core::ItemId;

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
