//! Battle inputs read from the worn gear: per-hand weapon values and the gear totals.

use ro_db::{RefineBonus, WeaponType};

/// One hand's weapon as the status reads it (spec `specs/stats/spec.md` §2.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WeaponGear {
    pub(crate) weapon_type: WeaponType,
    /// Item ATK and MATK.
    pub(crate) atk: u32,
    pub(crate) matk: u32,
    /// Weapon level (the damage variance).
    pub(crate) level: u8,
    /// Refine table entry (1/100 units); zero unrefined or not `Refineable`.
    pub(crate) refine: RefineBonus,
}

/// The gear part of the status: per-hand weapons, shield, hard DEF of every worn item once (ammo
/// skipped), the armor refine sum and the attack range. Rebuilt only on a gear change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GearStatus {
    /// The right-hand weapon. A two-hander that holds only the left hand so far counts here, as
    /// in rAthena (DEFERRED "Per-hand values").
    pub(crate) right: Option<WeaponGear>,
    /// A second, different weapon in the left hand (dual wield).
    pub(crate) left: Option<WeaponGear>,
    /// Armor in the left hand.
    pub(crate) shield: bool,
    /// Σ item `Defense`.
    pub(crate) hard_def: u32,
    /// Σ refine `Bonus` (1/100) over every worn, refined, `Refineable` armor.
    pub(crate) armor_refine: u32,
    pub(crate) range: u16,
}
