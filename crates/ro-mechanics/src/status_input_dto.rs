//! The inputs of the status calculation: job row, weapon class, refines, hands and gear.

use crate::{BonusTotals, ClassFlags, Primary, Traits};

/// The job's data at the character's levels, mapped from `ro_db::Job` by the engine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobRow {
    /// Cumulative job-level bonus at the current job level.
    pub bonus: Primary,
    /// The same bonus for POW..CRT.
    pub trait_bonus: Traits,
    /// The base HP/SP table at the current base level.
    pub base_hp: u32,
    pub base_sp: u32,
    /// The base AP table at the current base level.
    pub base_ap: u32,
    pub max_weight: u32,
    /// The job's ASPD base for a shield.
    pub shield_aspd: u16,
}

/// How a hand's weapon enters the formulas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WeaponClass {
    /// Bare hand.
    #[default]
    Fist,
    /// Any melee weapon but a katar.
    Melee,
    /// Doubles CRITICAL.
    Katar,
    /// Musical, whip and guns: DEX-based status ATK and ASPD.
    Ranged,
    /// Ranged, and gets no refine MATK.
    Bow,
}

impl WeaponClass {
    /// DEX takes STR's place in status ATK and the weapon bonus, and ASPD uses `dex² / 7`.
    pub fn is_ranged(self) -> bool {
        matches!(self, Self::Ranged | Self::Bow)
    }
}

/// A refine table entry (`ro_db::RefineDb`), 1/100 units; zero for no refine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefineInput {
    pub bonus: u32,
    pub random_bonus: u32,
}

/// One hand's weapon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HandInput {
    pub class: WeaponClass,
    /// Item ATK and MATK.
    pub atk: u32,
    pub matk: u32,
    pub refine: RefineInput,
    /// The job's ASPD base for this weapon type (`Fist` when bare).
    pub aspd_base: u16,
}

impl HandInput {
    /// No weapon: the job's `Fist` base.
    pub fn bare(fist_aspd_base: u16) -> Self {
        Self {
            aspd_base: fist_aspd_base,
            ..Self::default()
        }
    }
}

/// The worn gear, as the engine summarizes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GearInput {
    /// The right-hand weapon (a two-hander counts here), or bare.
    pub right: HandInput,
    /// A second, different weapon in the left hand (dual wield).
    pub left: Option<HandInput>,
    /// A shield is worn.
    pub shield: bool,
    /// Σ item DEF of every worn item, each once.
    pub item_def: i32,
    /// Σ armor refine `Bonus` (1/100) over every worn Armor-type item.
    pub armor_refine: u32,
    /// Σ item hard MDEF (0 until the item-bonus plan).
    pub item_mdef: i32,
    /// The equipment part of the VIT and INT bonus: MaxHP/MaxSP add it flat.
    pub vit: i32,
    pub int: i32,
    /// Attack range in cells.
    pub range: u16,
}

pub struct StatusInputs<'a> {
    /// Allocated STR..LUK (the DB columns).
    pub allocated: Primary,
    /// Allocated POW..CRT (the DB columns).
    pub traits: Traits,
    pub base_level: u16,
    pub job: JobRow,
    pub class: ClassFlags,
    pub gear: &'a GearInput,
    /// Gear, cards and passives.
    pub base_bonus: &'a BonusTotals,
    /// Active status effects.
    pub battle_bonus: &'a BonusTotals,
}
