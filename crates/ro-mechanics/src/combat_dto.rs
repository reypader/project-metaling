//! The inputs and output of the weapon-attack formula: attacker and defender views, the
//! attack context, and the damage result.

use crate::Status;

/// The right-hand weapon's attack values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeaponAttack {
    /// Item ATK (`rhw.atk`).
    pub atk: u32,
    /// The refine ATK (`rhw.atk2`, refine `Bonus / 100`).
    pub refine_atk: u32,
    /// Weapon level.
    pub level: u8,
    /// Overrefine: adds `roll(overrefine) + 1` when non-zero.
    pub overrefine: u16,
    /// DEX replaces STR in the weapon bonus (bow, instrument, whip and guns).
    pub dex_based: bool,
}

/// Element rates per damage part, in percent. All 100 until `attr_fix` is imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrRates {
    /// Status ATK (always the neutral attack element against the defender).
    pub status: u16,
    /// Weapon ATK (the weapon's element).
    pub weapon: u16,
    /// Equip and ammo ATK (the weapon's or the ammo's element).
    pub equip: u16,
}

impl Default for AttrRates {
    fn default() -> Self {
        Self {
            status: 100,
            weapon: 100,
            equip: 100,
        }
    }
}

/// What an attack reads from the attacker, plus the target-dependent rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackerView {
    pub hit: i32,
    /// 0.1 %; 0 when the attacker type can't crit (`calc_status` applies `enable_critical`).
    pub cri: i32,
    pub crate_: i32,
    pub patk: i32,
    /// Status ATK (`SP_ATK1`).
    pub batk: i32,
    /// Battle STR and DEX, for the weapon bonus.
    pub str: i32,
    pub dex: i32,
    /// `None` bare-handed: the weapon part is 0.
    pub weapon: Option<WeaponAttack>,
    /// Equip ATK plus the ammo `Attack` for a bow or gun: flat, no variance, no size modifier.
    pub equip_atk: i32,
    /// `BF_LONG`: an ammo-using weapon (bow, guns). Whips and instruments are short.
    pub long_range: bool,
    /// The weapon's size modifier against the target, percent (100 until `size_fix`).
    pub size_mod: u16,
    pub attr: AttrRates,
}

impl Default for AttackerView {
    fn default() -> Self {
        Self {
            hit: 0,
            cri: 0,
            crate_: 0,
            patk: 0,
            batk: 0,
            str: 0,
            dex: 0,
            weapon: None,
            equip_atk: 0,
            long_range: false,
            size_mod: 100,
            attr: AttrRates::default(),
        }
    }
}

impl AttackerView {
    /// The status part; the engine fills `weapon`, `equip_atk`, `long_range` and the rates.
    pub fn from_status(status: &Status) -> Self {
        Self {
            hit: status.hit,
            cri: status.cri,
            crate_: status.crate_,
            patk: status.patk,
            batk: status.batk,
            str: status.battle.str,
            dex: status.battle.dex,
            ..Self::default()
        }
    }
}

/// What an attack reads from the defender.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DefenderView {
    pub flee: i32,
    /// Perfect dodge, 0.1 %.
    pub flee2: i32,
    pub luk: i32,
    pub hard_def: i32,
    pub soft_def: i32,
    pub res: i32,
    /// Mode `IgnoreMelee`: a landed short-range hit does 1.
    pub ignore_melee: bool,
    /// Mode `IgnoreRanged`: a landed long-range hit does 1.
    pub ignore_ranged: bool,
}

impl DefenderView {
    /// The status part; the engine sets the infinite-defense modes.
    pub fn from_status(status: &Status) -> Self {
        Self {
            flee: status.flee,
            flee2: status.flee2,
            luk: status.battle.luk,
            hard_def: status.hard_def,
            soft_def: status.soft_def,
            res: status.res,
            ignore_melee: false,
            ignore_ranged: false,
        }
    }
}

/// What kind of weapon attack this is: a normal attack, or a weapon skill's modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackContext {
    /// Skill ratio in percent, applied to the ATK sum after P.ATK, before Res and DEF.
    pub ratio: i32,
    /// Hit-rate modifier in percent of `hit − flee`, before the min/max clamp (Bash).
    pub hit_pct: i32,
    /// Whether the attack may crit. Skills can't unless their data says so.
    pub can_crit: bool,
    /// Whether the defender's perfect dodge (`flee2`) applies. Never for skills.
    pub can_lucky_dodge: bool,
}

impl AttackContext {
    /// A normal attack: ratio 100, no hit modifier, crit and lucky dodge on.
    pub const NORMAL: Self = Self {
        ratio: 100,
        hit_pct: 0,
        can_crit: true,
        can_lucky_dodge: true,
    };

    /// A weapon skill: no crit and no lucky dodge.
    pub fn skill(ratio: i32, hit_pct: i32) -> Self {
        Self {
            ratio,
            hit_pct,
            can_crit: false,
            can_lucky_dodge: false,
        }
    }
}

/// How a swing landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitKind {
    Normal,
    Crit,
    Miss,
    LuckyDodge,
}

impl HitKind {
    /// The `0x08c8` `type`: `DMG_NORMAL` 0 (a miss too), `DMG_CRITICAL` 10, `DMG_LUCY_DODGE` 11.
    pub fn wire_type(self) -> u8 {
        match self {
            Self::Normal | Self::Miss => 0,
            Self::Crit => 10,
            Self::LuckyDodge => 11,
        }
    }

    pub fn is_hit(self) -> bool {
        matches!(self, Self::Normal | Self::Crit)
    }
}

/// One swing's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageResult {
    /// 0 on a miss or lucky dodge; at least 1 on a hit.
    pub damage: i32,
    /// The left hand: 0 until dual wield.
    pub damage2: i32,
    pub div: u16,
    pub kind: HitKind,
}

impl DamageResult {
    pub(crate) fn missed(kind: HitKind) -> Self {
        Self {
            damage: 0,
            damage2: 0,
            div: 1,
            kind,
        }
    }
}
