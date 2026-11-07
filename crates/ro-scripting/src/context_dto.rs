//! What a hook reads: copies of the units, the cast, and the engine's skill levels and RNG.

use ro_core::ItemId;

use crate::SkillLevels;

/// A unit as a hook sees it. Small and `Copy`; the engine fills it from its status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitView {
    pub base_level: u16,
    /// rAthena job id; 0 for a mob.
    pub job: u16,
    /// Battle stats (with gear and bonuses).
    pub str: i32,
    pub agi: i32,
    pub vit: i32,
    pub int: i32,
    pub dex: i32,
    pub luk: i32,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    /// Status MATK as Heal reads it: `INT + INT/2 + DEX/5 + LUK/3 + BaseLv/4 + 5·SPL` for a
    /// player (catalog `scripting/skills.md` §7).
    pub matk_base: i32,
    /// The right-hand weapon's MATK and weapon level; 0 without a weapon.
    pub weapon_matk: i32,
    pub weapon_level: u8,
}

/// Where a hook call comes from (settled 2026-11-01, spec §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastSource {
    /// A learned skill, cast with `0x0438`.
    Learned,
    /// A skill cast that an item started (`itemskill`, e.g. Fly Wing). Not supported yet.
    ItemSkill(ItemId),
    /// A mob skill. Not supported yet.
    Mob,
    /// An item's own use script (a potion).
    ItemUse(ItemId),
}

/// One hook call's input.
#[derive(Clone, Copy)]
pub struct HookCtx<'a> {
    pub caster: UnitView,
    /// The caster again for self skills and item use.
    pub target: UnitView,
    /// 0 for item use.
    pub skill: u16,
    /// The cast level; 0 for item use.
    pub level: u8,
    pub source: CastSource,
    /// Hook arguments from data, e.g. the `itemheal` ranges `[hp_min, hp_max, sp_min, sp_max]`.
    pub args: [i32; 4],
    pub skills: &'a dyn SkillLevels,
}
