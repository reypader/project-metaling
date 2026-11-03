//! Weapon-skill damage, cast time and after-cast delay (spec `specs/effects/spec.md`
//! MECH-EFF-02, MECH-EFF-03). MECH-EFF-01 (`AttackContext::NORMAL` keeps every normal-attack
//! vector) is `combat_tests.rs`, which passes `NORMAL` with its expected values unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_mechanics::{
    calc_weapon_attack, cast_time, delayfix, AttackContext, AttackerView, BattleConfig,
    CastTimeData, Caster, DamageResult, DefenderView, HitKind,
};

/// The capture's caster (catalog `scripting/skills.md` §14): all stats 1, HIT 189, unarmed.
fn unarmed(batk: i32) -> AttackerView {
    AttackerView {
        hit: 189,
        cri: 10,
        batk,
        str: 1,
        dex: 1,
        ..AttackerView::default()
    }
}

/// A Poring: FLEE 102, DEF 2, soft DEF (level 1 + VIT 1) / 2 = 1, perfect dodge 0
/// (catalog `scripting/skills.md` §14.2).
fn poring() -> DefenderView {
    DefenderView {
        flee: 102,
        flee2: 0,
        luk: 5,
        hard_def: 2,
        soft_def: 1,
        ..DefenderView::default()
    }
}

/// Runs `calc_weapon_attack` with `roll` and records every `n` it asked for.
fn run(
    attacker: &AttackerView,
    defender: &DefenderView,
    context: &AttackContext,
    roll: impl Fn(u32) -> u32,
) -> (DamageResult, Vec<u32>) {
    let mut asked = Vec::new();
    let result = calc_weapon_attack(attacker, defender, context, &BattleConfig::default(), |n| {
        asked.push(n);
        roll(n)
    });
    (result, asked)
}

/// MECH-EFF-02: the unarmed capture rows, `scripting_1.txt` `#2462` (Bash lv 1 = 11 on a Poring,
/// ATK1 5) and `#5826` (Mammonite lv 10 = 46, ATK1 4). Bash: ratio 130, hit +5 %; Mammonite:
/// ratio 600.
#[test]
fn mech_eff_02_capture_rows() {
    let bash = AttackContext::skill(130, 5);
    let (hit, rolls) = run(&unarmed(5), &poring(), &bash, |_| 0);
    assert_eq!((hit.damage, hit.kind, hit.div), (11, HitKind::Normal, 1));
    // Unarmed: the hit roll is the only draw (no lucky dodge, no crit, no weapon variance).
    assert_eq!(rolls, [100]);

    let mammonite = AttackContext::skill(600, 0);
    let (hit, _) = run(&unarmed(4), &poring(), &mammonite, |_| 0);
    assert_eq!(hit.damage, 46);
}

/// MECH-EFF-02: skills skip the lucky dodge and crit rolls; a normal attack makes them.
#[test]
fn mech_eff_02_no_lucky_dodge_or_crit() {
    let dodgy = DefenderView {
        flee2: 1000,
        ..poring()
    };
    let critter = AttackerView {
        cri: 1000,
        ..unarmed(5)
    };
    let (hit, rolls) = run(&critter, &dodgy, &AttackContext::skill(130, 0), |_| 0);
    assert_eq!((hit.kind, rolls), (HitKind::Normal, vec![100]));
    let (normal, rolls) = run(&critter, &dodgy, &AttackContext::NORMAL, |_| 0);
    assert_eq!((normal.kind, rolls), (HitKind::LuckyDodge, vec![1000]));
}

/// MECH-EFF-02: the ratio multiplies after P.ATK. Equip ATK 1, P.ATK 50, ratio 400, no DEF:
/// `1 × 150 / 100 = 1`, then `× 400 / 100 = 4` (ratio first would give 6).
#[test]
fn mech_eff_02_ratio_after_patk() {
    let attacker = AttackerView {
        hit: 200,
        equip_atk: 1,
        patk: 50,
        ..AttackerView::default()
    };
    let (hit, _) = run(
        &attacker,
        &DefenderView::default(),
        &AttackContext::skill(400, 0),
        |_| 0,
    );
    assert_eq!(hit.damage, 4);
}

/// MECH-EFF-02: Bash's hit modifier applies to `hit − flee` before the [5, 100] clamp
/// (catalog `scripting/skills.md` §9).
#[test]
fn mech_eff_02_hit_modifier_before_clamp() {
    let defender = DefenderView::default();
    // 60 + 60 × 50 / 100 = 90: roll 89 hits, roll 90 misses.
    let sixty = AttackerView {
        hit: 60,
        batk: 5,
        ..AttackerView::default()
    };
    let context = AttackContext::skill(100, 50);
    assert_eq!(
        run(&sixty, &defender, &context, |_| 89).0.kind,
        HitKind::Normal
    );
    assert_eq!(
        run(&sixty, &defender, &context, |_| 90).0.kind,
        HitKind::Miss
    );
    // −90 − 45 = −135, clamped to 5 (after the clamp it would be 5 + 2 = 7): roll 6 misses.
    let blind = DefenderView {
        flee: 100,
        ..defender
    };
    let ten = AttackerView { hit: 10, ..sixty };
    assert_eq!(run(&ten, &blind, &context, |_| 4).0.kind, HitKind::Normal);
    let miss = run(&ten, &blind, &context, |_| 6).0;
    assert_eq!((miss.kind, miss.damage), (HitKind::Miss, 0));
}

fn player(dex: i32, int: i32) -> Caster {
    Caster::Player { dex, int }
}

fn data(cast_time: u32, fixed_cast_time: i32) -> CastTimeData {
    CastTimeData {
        cast_time,
        fixed_cast_time,
        ignore_dex: false,
    }
}

/// MECH-EFF-03: the worked examples of catalog `scripting/skills.md` §4.1 (rAthena
/// `skill_vfcastfix` at default config, float32 quotient and square root).
#[test]
fn mech_eff_03_cast_time_examples() {
    let config = BattleConfig::default();
    for (data, caster, expected) in [
        (data(1000, 0), player(1, 1), 924),
        // An explicit −1 splits 20 % fixed: 800 × 0.924765 = 739, + 200.
        (data(1000, -1), player(1, 1), 939),
        (data(1000, 0), player(50, 30), 504),
        // A set fixed part is never reduced: 502 + 500.
        (data(2000, 500), player(99, 99), 1002),
        (data(1000, 0), player(200, 70), 58),
        (data(1000, 0), player(265, 0), 0),
        // Above the scale the factor goes negative; the variable part stays at 0.
        (data(1000, 300), player(300, 300), 300),
        (data(0, 0), player(1, 1), 0),
    ] {
        assert_eq!(
            cast_time(data, caster, &config),
            expected,
            "{data:?} {caster:?}"
        );
    }
}

/// MECH-EFF-03: `IgnoreDex`, a zero split rate, and the mob/NPC passthrough.
#[test]
fn mech_eff_03_cast_time_rules() {
    let config = BattleConfig::default();
    let ignore = CastTimeData {
        ignore_dex: true,
        ..data(1000, 0)
    };
    assert_eq!(cast_time(ignore, player(99, 99), &config), 1000);
    let no_split = BattleConfig {
        default_fixed_castrate: 0,
        ..BattleConfig::default()
    };
    // Rate 0: nothing fixed, the whole 1000 is variable.
    assert_eq!(cast_time(data(1000, -1), player(1, 1), &no_split), 924);
    // Mobs and NPCs: the data cast time, no stats, no fixed part.
    assert_eq!(cast_time(data(1500, 700), Caster::MobOrNpc, &config), 1500);
}

/// MECH-EFF-03: `skill_delayfix` at default config (catalog `scripting/skills.md` §4): Heal's
/// 500, none for the other slice skills, a negative delay is `−delay + amotion`.
#[test]
fn mech_eff_03_delayfix() {
    assert_eq!(delayfix(500, 440), 500);
    assert_eq!(delayfix(0, 590), 0);
    assert_eq!(delayfix(-100, 440), 540);
}
