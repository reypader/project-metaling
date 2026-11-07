//! Renewal weapon-attack damage for normal attacks and weapon skills (`specs/combat/spec.md`
//! §2.5, `specs/effects/spec.md` §2.6; catalog `mechanics/combat.md` §1-§5, checked against the
//! `combat_sequence_1.txt` vectors in §7.3, and `scripting/skills.md` §9) and the client's
//! circular attack range (`auto_attack.md` §2). Pure: the engine builds the views from `Status`
//! and the gear, and passes the random source.

use crate::{
    AttackContext, AttackerView, BattleConfig, DamageResult, DefenderView, HitKind, WeaponAttack,
};

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

/// A weapon attack. `roll(n)` returns `rnd() % n`; it is called in rAthena's order: lucky
/// dodge (only when `context` allows it and the defender has `flee2`), crit (only when allowed
/// and with a positive chance), hit (not after a crit), then the weapon variance and the
/// overrefine. A skill (`AttackContext::skill`) therefore rolls the hit, then the variance.
pub fn calc_weapon_attack(
    attacker: &AttackerView,
    defender: &DefenderView,
    context: &AttackContext,
    config: &BattleConfig,
    mut roll: impl FnMut(u32) -> u32,
) -> DamageResult {
    if context.can_lucky_dodge && defender.flee2 > 0 && below(roll(PER_MILLE), defender.flee2) {
        return DamageResult::missed(HitKind::LuckyDodge);
    }
    let cri = attacker.cri - defender.luk * CRIT_LUK_FACTOR;
    let crit = context.can_crit && cri > 0 && below(roll(PER_MILLE), cri);
    if !crit {
        let mut hitrate = attacker.hit - defender.flee;
        // C int math: truncates toward zero, also for a negative `hit − flee`.
        hitrate += hitrate * context.hit_pct / 100;
        let hitrate = hitrate.clamp(
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
    // `masteryAtk` would be added here; there are no mastery skills yet. The ratio covers it.
    damage = damage * i64::from(context.ratio) / 100;

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
