//! The computed status and its primary and trait stat groups.

use crate::{BonusSlot, BonusTotals};

/// The six primary stats.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Primary {
    pub str: i32,
    pub agi: i32,
    pub vit: i32,
    pub int: i32,
    pub dex: i32,
    pub luk: i32,
}

impl Primary {
    pub(crate) fn plus(self, other: Primary) -> Primary {
        Primary {
            str: self.str + other.str,
            agi: self.agi + other.agi,
            vit: self.vit + other.vit,
            int: self.int + other.int,
            dex: self.dex + other.dex,
            luk: self.luk + other.luk,
        }
    }

    pub(crate) fn from_bonus(bonus: &BonusTotals) -> Primary {
        Primary {
            str: bonus.get(BonusSlot::Str),
            agi: bonus.get(BonusSlot::Agi),
            vit: bonus.get(BonusSlot::Vit),
            int: bonus.get(BonusSlot::Int),
            dex: bonus.get(BonusSlot::Dex),
            luk: bonus.get(BonusSlot::Luk),
        }
    }

    /// rAthena keeps each stat in `0..=65535`.
    pub(crate) fn clamped(self) -> Primary {
        let c = |v: i32| v.clamp(0, i32::from(u16::MAX));
        Primary {
            str: c(self.str),
            agi: c(self.agi),
            vit: c(self.vit),
            int: c(self.int),
            dex: c(self.dex),
            luk: c(self.luk),
        }
    }
}

/// The six trait stats, POW..CRT in this order everywhere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Traits {
    pub pow: i32,
    pub sta: i32,
    pub wis: i32,
    pub spl: i32,
    pub con: i32,
    pub crt: i32,
}

impl Traits {
    pub(crate) fn plus(self, other: Traits) -> Traits {
        Traits {
            pow: self.pow + other.pow,
            sta: self.sta + other.sta,
            wis: self.wis + other.wis,
            spl: self.spl + other.spl,
            con: self.con + other.con,
            crt: self.crt + other.crt,
        }
    }

    /// rAthena keeps each trait in `0..=65535`.
    pub(crate) fn clamped(self) -> Traits {
        let c = |v: i32| v.clamp(0, i32::from(u16::MAX));
        Traits {
            pow: c(self.pow),
            sta: c(self.sta),
            wis: c(self.wis),
            spl: c(self.spl),
            con: c(self.con),
            crt: c(self.crt),
        }
    }
}

/// The computed status. `cri` and `flee2` keep rAthena's 1/10 units: the push diff compares
/// them, and the window shows them divided by 10.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    /// Job bonus + allocated + base-layer bonuses.
    pub base: Primary,
    /// Base + status effects. `0x0141` sends `bonus = battle - allocated`.
    pub battle: Primary,
    /// Job trait bonus + allocated POW..CRT (no item or SC sources yet).
    pub trait_base: Traits,
    /// `trait_base` + status effects (none yet).
    pub trait_battle: Traits,
    /// Status ATK (`SP_ATK1`).
    pub batk: i32,
    /// Weapon ATK with refine, plus equipment ATK (`SP_ATK2`).
    pub watk: i32,
    /// Status MATK, the window's left side (`SP_MATK2`).
    pub status_matk: i32,
    /// Weapon MATK with refine, the window's right side (`SP_MATK1`).
    pub weapon_matk: i32,
    /// `def2` (`SP_DEF1`).
    pub soft_def: i32,
    /// `def` (`SP_DEF2`).
    pub hard_def: i32,
    /// `mdef2` (`SP_MDEF1`).
    pub soft_mdef: i32,
    /// `mdef` (`SP_MDEF2`).
    pub hard_mdef: i32,
    pub hit: i32,
    pub flee: i32,
    /// Perfect dodge, 1/10.
    pub flee2: i32,
    /// Critical, 1/10.
    pub cri: i32,
    /// `amotion`, as `SP_ASPD` sends it.
    pub amotion: i32,
    /// ms between swings. Players: `2 × amotion`. Not pushed to the client.
    pub adelay: i32,
    /// The flinch time a hit on this unit shows (`0x08c8` `dmg_speed`). Not pushed.
    pub dmotion: i32,
    pub max_hp: u32,
    pub max_sp: u32,
    /// Trait-derived values (`SP_PATK`..`SP_CRATE`), each `0..=32767`. Combat reads them later.
    pub patk: i32,
    pub smatk: i32,
    pub res: i32,
    pub mres: i32,
    pub hplus: i32,
    pub crate_: i32,
    pub max_ap: u32,
    pub max_weight: u32,
    pub range: u16,
    /// Overrefine per hand (right, left): `RandomBonus / 100`, for combat.
    pub overrefine: [u16; 2],
}

impl Status {
    /// `SP_FLEE2` as the window shows it.
    pub fn flee2_shown(&self) -> i32 {
        self.flee2 / 10
    }

    /// `SP_CRITICAL` as the window shows it.
    pub fn critical_shown(&self) -> i32 {
        self.cri / 10
    }
}
