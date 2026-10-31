//! Renewal normal-attack damage (`specs/combat/spec.md` §2.5; catalog `mechanics/combat.md`
//! §1-§5, checked against the `combat_sequence_1.txt` vectors in §7.3) and the client's
//! circular attack range (`auto_attack.md` §2). Pure: the engine builds the views from `Status`
//! and the gear, and passes the random source.

use crate::{BattleConfig, Status};

/// The weapon variance and the cap of `atkmax` (`battle_calc_base_weapon_attack`).
const VARIANCE_PERCENT: f64 = 5.0;
const ATK_MAX: f64 = 65535.0;
/// DEF reduction constants: `damage × (4000 + def) / (4000 + 10 × def)`; `def = -400` would
/// divide by zero and is raised to -399.
const DEF_BASE: i64 = 4000;
const DEF_FACTOR: i64 = 10;
const DEF_SINGULAR: i64 = -400;
/// Res reduction: `damage × res / (res + 400) × 0.8`.
const RES_BASE: f64 = 400.0;
const RES_FACTOR: f64 = 0.8;
/// The crit multiplier: `1.4 + 0.01 × crate`.
const CRIT_BASE: f64 = 1.4;
const CRIT_PER_CRATE: f64 = 0.01;
/// Crit and lucky dodge roll per mille; hit per cent.
const PER_MILLE: u32 = 1000;
const PER_CENT: u32 = 100;
/// Crit chance loses this many 0.1 % per point of defender LUK (a player attacking).
const CRIT_LUK_FACTOR: i32 = 2;

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
    fn missed(kind: HitKind) -> Self {
        Self {
            damage: 0,
            damage2: 0,
            div: 1,
            kind,
        }
    }
}

/// A normal weapon attack. `roll(n)` returns `rnd() % n`; it is called in rAthena's order:
/// lucky dodge (only when the defender has `flee2`), crit (only with a positive chance), hit
/// (not after a crit), then the weapon variance and the overrefine.
pub fn calc_weapon_attack(
    attacker: &AttackerView,
    defender: &DefenderView,
    config: &BattleConfig,
    mut roll: impl FnMut(u32) -> u32,
) -> DamageResult {
    if defender.flee2 > 0 && below(roll(PER_MILLE), defender.flee2) {
        return DamageResult::missed(HitKind::LuckyDodge);
    }
    let cri = attacker.cri - defender.luk * CRIT_LUK_FACTOR;
    let crit = cri > 0 && below(roll(PER_MILLE), cri);
    if !crit {
        let hitrate = (attacker.hit - defender.flee).clamp(
            i32::try_from(config.min_hitrate).unwrap_or(0),
            i32::try_from(config.max_hitrate).unwrap_or(100),
        );
        if !below(roll(PER_CENT), hitrate) {
            return DamageResult::missed(HitKind::Miss);
        }
    }
    let kind = if crit { HitKind::Crit } else { HitKind::Normal };
    let infinite = if attacker.long_range {
        defender.ignore_ranged
    } else {
        defender.ignore_melee
    };
    if infinite {
        return DamageResult {
            damage: 1,
            damage2: 0,
            div: 1,
            kind,
        };
    }

    let status_atk = i64::from(attacker.batk) * i64::from(attacker.attr.status) / 100 * 2;
    let weapon_atk = attacker.weapon.map_or(0, |weapon| {
        weapon_roll(attacker, &weapon, crit, &mut roll) * i64::from(attacker.attr.weapon) / 100
    });
    let equip_atk = i64::from(attacker.equip_atk) * i64::from(attacker.attr.equip) / 100;
    // `percentAtk` is `(weapon + equip) × atk_rate / 100`: 0 until item bonuses exist.
    let mut damage = status_atk + weapon_atk + equip_atk;
    damage = damage * (100 + i64::from(attacker.patk)) / 100;
    // The skill ratio of a normal attack is 100.

    if defender.res > 0 {
        let res = f64::from(defender.res);
        // C: `damage -= damage * (res / (res + 400.0)) * 0.8` on an int64.
        damage = (damage as f64 - damage as f64 * (res / (res + RES_BASE)) * RES_FACTOR) as i64;
    }
    let def = match i64::from(defender.hard_def) {
        DEF_SINGULAR => DEF_SINGULAR + 1,
        def => def,
    };
    damage =
        damage * (DEF_BASE + def) / (DEF_BASE + DEF_FACTOR * def) - i64::from(defender.soft_def);
    damage = damage.max(1);
    if crit {
        damage = (damage as f64 * (CRIT_BASE + CRIT_PER_CRATE * f64::from(attacker.crate_))) as i64;
    }
    DamageResult {
        damage: i32::try_from(damage).unwrap_or(i32::MAX),
        damage2: 0,
        div: 1,
        kind,
    }
}

/// The weapon part before the element: the variance roll (`atkmax` on a crit), the overrefine,
/// then the size modifier.
fn weapon_roll(
    attacker: &AttackerView,
    weapon: &WeaponAttack,
    crit: bool,
    roll: &mut impl FnMut(u32) -> u32,
) -> i64 {
    let atk = f64::from(weapon.atk);
    let watk = f64::from(weapon.atk) + f64::from(weapon.refine_atk);
    let stat = if weapon.dex_based {
        attacker.dex
    } else {
        attacker.str
    };
    let variance = VARIANCE_PERCENT * atk * f64::from(weapon.level) / 100.0;
    let stat_bonus = atk * f64::from(stat) / 200.0;
    let atkmin = (watk - variance + stat_bonus).max(0.0) as i64;
    let atkmax = (watk + variance + stat_bonus).min(ATK_MAX) as i64;
    let mut damage = if crit || atkmax <= atkmin {
        atkmax
    } else {
        let span = u32::try_from(atkmax - atkmin + 1).unwrap_or(u32::MAX);
        atkmin + i64::from(roll(span))
    };
    if weapon.overrefine > 0 {
        damage += i64::from(roll(u32::from(weapon.overrefine))) + 1;
    }
    // `weapon_damage_rate` (card bonuses) is 0 until item bonuses exist.
    damage * i64::from(attacker.size_mod) / 100
}

/// `value < threshold` for a roll against a signed chance.
fn below(value: u32, threshold: i32) -> bool {
    i64::from(value) < i64::from(threshold)
}

/// The client's circular distance (`check_distance_client`): `floor(max(√(dx² + dy²) − 0.1,
/// 0))`. Straight lines reach one cell further than diagonals.
pub fn client_distance(dx: i32, dy: i32) -> i32 {
    let (dx, dy) = (f64::from(dx), f64::from(dy));
    ((dx * dx + dy * dy).sqrt() - 0.1).max(0.0) as i32
}

/// Whether a cell `(dx, dy)` away is within `range` by the client's distance.
pub fn check_distance_client(dx: i32, dy: i32, range: u16) -> bool {
    client_distance(dx, dy) <= i32::from(range)
}
