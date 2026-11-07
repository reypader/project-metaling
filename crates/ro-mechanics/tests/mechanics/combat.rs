//! Normal-attack damage, mob status, client distance and exp share (spec
//! `specs/combat/spec.md` MECH-CMB-01..06; catalog `mechanics/combat.md` §1.1, §5, §7.3).

use ro_mechanics::{
    calc_mob_status, calc_weapon_attack, check_distance_client, exp_share, shared_exp,
    AttackContext, AttackerView, BattleConfig, DamageResult, DefenderView, HitKind,
    MobStatusInputs, Primary, WeaponAttack,
};

fn stats(str: i32, agi: i32, vit: i32, int: i32, dex: i32, luk: i32) -> Primary {
    Primary {
        str,
        agi,
        vit,
        int,
        dex,
        luk,
    }
}

/// Mob status inputs from catalog `combat.md` §7 (db values).
fn mob(level: u16, stats: Primary, defense: i32) -> MobStatusInputs {
    MobStatusInputs {
        level,
        stats,
        defense,
        resistance: 0,
        max_hp: 1,
        attack_motion: 100,
        attack_delay: 100,
        damage_motion: 0,
        attack_range: 1,
    }
}

fn poring() -> MobStatusInputs {
    MobStatusInputs {
        max_hp: 55,
        attack_motion: 672,
        attack_delay: 1872,
        damage_motion: 480,
        ..mob(1, stats(6, 1, 1, 0, 6, 5), 2)
    }
}

fn defender(inputs: &MobStatusInputs) -> DefenderView {
    DefenderView::from_status(&calc_mob_status(inputs, &BattleConfig::default()))
}

fn knife() -> WeaponAttack {
    WeaponAttack {
        atk: 17,
        level: 1,
        ..WeaponAttack::default()
    }
}

fn bow() -> WeaponAttack {
    WeaponAttack {
        atk: 15,
        level: 1,
        dex_based: true,
        ..WeaponAttack::default()
    }
}

/// A player that never crits unless `cri` is set, with a sure hit (HIT 1000).
fn attacker(batk: i32, str: i32, dex: i32, weapon: Option<WeaponAttack>) -> AttackerView {
    AttackerView {
        hit: 1000,
        batk,
        str,
        dex,
        weapon,
        ..AttackerView::default()
    }
}

/// Fixed rolls: no lucky dodge or crit (`n` 1000 gives 999), a hit (`n` 100 gives 0), and the
/// weapon variance at its low (`high = false`) or high end.
fn rolls(high: bool) -> impl FnMut(u32) -> u32 {
    move |n| match n {
        1000 => 999,
        100 => 0,
        _ if high => n - 1,
        _ => 0,
    }
}

fn swing(a: &AttackerView, d: &DefenderView, high: bool) -> DamageResult {
    calc_weapon_attack(
        a,
        d,
        &AttackContext::NORMAL,
        &BattleConfig::default(),
        rolls(high),
    )
}

/// The damage range over the weapon variance.
fn range(a: &AttackerView, d: &DefenderView) -> (i32, i32) {
    let low = swing(a, d, false);
    let high = swing(a, d, true);
    assert_eq!((low.kind, high.kind), (HitKind::Normal, HitKind::Normal));
    (low.damage, high.damage)
}

/// MECH-CMB-01: `calc_mob_status(Poring)`.
#[test]
fn mech_cmb_01_poring_status() {
    let s = calc_mob_status(&poring(), &BattleConfig::default());
    assert_eq!((s.hit, s.flee, s.flee2, s.cri), (157, 102, 0, 0));
    assert_eq!((s.hard_def, s.soft_def), (2, 1));
    assert_eq!((s.amotion, s.adelay, s.dmotion), (672, 1872, 480));
    assert_eq!((s.max_hp, s.battle.luk), (55, 5));
    let pupa = calc_mob_status(
        &mob(4, stats(1, 1, 3, 1, 1, 1), 24),
        &BattleConfig::default(),
    );
    assert_eq!((pupa.flee, pupa.soft_def), (105, 3));
    let sphere = calc_mob_status(
        &mob(51, stats(1, 15, 35, 1, 1, 1), 41),
        &BattleConfig::default(),
    );
    assert_eq!(sphere.soft_def, 43);
}

/// MECH-CMB-02: the worked skeleton with fixed rolls.
#[test]
fn mech_cmb_02_miss_hit_crit_def_and_infinite_defense() {
    let target = defender(&poring());
    let config = BattleConfig::default();
    // HIT 177 vs FLEE 102: hitrate 75. A roll of 75 misses, 74 hits.
    let novice = AttackerView {
        hit: 177,
        ..attacker(1, 1, 1, None)
    };
    let miss = calc_weapon_attack(&novice, &target, &AttackContext::NORMAL, &config, |n| {
        if n == 100 {
            75
        } else {
            0
        }
    });
    assert_eq!(
        (miss.damage, miss.kind, miss.kind.wire_type()),
        (0, HitKind::Miss, 0)
    );
    let hit = calc_weapon_attack(&novice, &target, &AttackContext::NORMAL, &config, |n| {
        if n == 100 {
            74
        } else {
            0
        }
    });
    // Bare hands: 2 × batk 1 = 2; 2 × 4002 / 4020 = 1, − 1 = 0, then the minimum 1.
    assert_eq!(
        (hit.damage, hit.kind, hit.div, hit.damage2),
        (1, HitKind::Normal, 1, 0)
    );
    // The hit rate is clamped to min_hitrate: HIT 1 still hits on a roll of 4.
    let blind = AttackerView { hit: 1, ..novice };
    let lucky = calc_weapon_attack(&blind, &target, &AttackContext::NORMAL, &config, |n| {
        if n == 100 {
            4
        } else {
            0
        }
    });
    assert_eq!(lucky.kind, HitKind::Normal);

    // Crit: CRI 31 (0.1 %) − 2 × Pupa LUK 1 = 29; a roll of 28 crits, takes atkmax (18) and
    // × 1.4 after DEF: (98 + 18) × 4024 / 4240 − 3 = 107, × 1.4 = 149 (captured line 2960).
    let critter = AttackerView {
        cri: 31,
        ..attacker(49, 12, 15, Some(knife()))
    };
    let pupa = defender(&mob(4, stats(1, 1, 3, 1, 1, 1), 24));
    let crit = calc_weapon_attack(&critter, &pupa, &AttackContext::NORMAL, &config, |n| {
        if n == 1000 {
            28
        } else {
            0
        }
    });
    assert_eq!(
        (crit.damage, crit.kind, crit.kind.wire_type()),
        (149, HitKind::Crit, 10)
    );
    let no_crit = calc_weapon_attack(&critter, &pupa, &AttackContext::NORMAL, &config, |n| {
        if n == 1000 {
            29
        } else {
            0
        }
    });
    assert_eq!(no_crit.kind, HitKind::Normal, "29 is not below 29");

    // Infinite defense: IgnoreMelee takes 1 from a melee hit, a crit included; a ranged attack
    // and a miss are unaffected.
    let plant = DefenderView {
        ignore_melee: true,
        ..defender(&mob(1, stats(1, 0, 0, 1, 0, 0), 100))
    };
    assert_eq!(swing(&critter, &plant, true).damage, 1);
    let crit = calc_weapon_attack(&critter, &plant, &AttackContext::NORMAL, &config, |_| 0);
    assert_eq!((crit.damage, crit.kind), (1, HitKind::Crit));
    let archer = AttackerView {
        long_range: true,
        equip_atk: 25,
        ..attacker(53, 12, 15, Some(bow()))
    };
    assert!(
        swing(&archer, &plant, true).damage > 1,
        "IgnoreMelee: ranged hits normally"
    );
    let ranged_plant = DefenderView {
        ignore_melee: false,
        ignore_ranged: true,
        ..plant
    };
    assert_eq!(swing(&archer, &ranged_plant, true).damage, 1);
    let missed = calc_weapon_attack(&blind, &plant, &AttackContext::NORMAL, &config, |n| {
        if n == 100 {
            99
        } else {
            0
        }
    });
    assert_eq!((missed.damage, missed.kind), (0, HitKind::Miss));

    // Lucky dodge only when the defender has perfect dodge.
    let dodgy = DefenderView {
        flee2: 50,
        ..target
    };
    let dodged = calc_weapon_attack(&novice, &dodgy, &AttackContext::NORMAL, &config, |n| {
        if n == 1000 {
            49
        } else {
            0
        }
    });
    assert_eq!(
        (dodged.damage, dodged.kind, dodged.kind.wire_type()),
        (0, HitKind::LuckyDodge, 11)
    );
}

/// MECH-CMB-02: renewal Res, P.ATK and the overrefine roll.
#[test]
fn mech_cmb_02_res_patk_and_overrefine() {
    let target = DefenderView {
        hard_def: 0,
        soft_def: 0,
        ..defender(&poring())
    };
    let base = attacker(50, 1, 1, None);
    assert_eq!(swing(&base, &target, false).damage, 100);
    let patk = AttackerView { patk: 10, ..base };
    assert_eq!(swing(&patk, &target, false).damage, 110);
    // Res 50: damage −= damage × (50 / 450.0) × 0.8 in C, i.e. trunc(100 − 8.89) = 91 (not
    // 100 − trunc(8.89) = 92).
    let resist = DefenderView { res: 50, ..target };
    assert_eq!(swing(&base, &resist, false).damage, 91);
    // Overrefine 5: + roll(5) + 1, so 1..=5 on top of the weapon.
    let refined = attacker(
        0,
        0,
        0,
        Some(WeaponAttack {
            atk: 10,
            refine_atk: 7,
            level: 4,
            overrefine: 5,
            ..WeaponAttack::default()
        }),
    );
    // atk 10, wlv 4: variance 2.0, so 15..19 before the overrefine.
    assert_eq!(range(&refined, &target), (16, 24));
}

/// Phase, attacker, target, computed range, captured values.
type Vector<'a> = (
    &'a str,
    AttackerView,
    &'a DefenderView,
    (i32, i32),
    &'a [i32],
);

/// MECH-CMB-03: captured vectors (catalog `combat.md` §7.3, `combat_sequence_1.txt`; mob stats
/// from rAthena `db/re/mob_db.yml` as listed there). Each captured value lies in the
/// computed range; the Dagger-vs-Medium (Poring) cases pass `size_mod` 75.
#[test]
fn mech_cmb_03_captured_vectors() {
    let poring = defender(&poring());
    let lunatic = defender(&mob(3, stats(1, 3, 3, 1, 1, 1), 18));
    let pupa = defender(&mob(4, stats(1, 1, 3, 1, 1, 1), 24));
    let sphere = defender(&mob(51, stats(1, 15, 35, 1, 1, 1), 41));
    let dagger_vs_medium = |a: AttackerView| AttackerView { size_mod: 75, ..a };
    let archer = |batk, dex| AttackerView {
        long_range: true,
        equip_atk: 25,
        ..attacker(batk, 12, dex, Some(bow()))
    };
    let cases: [Vector<'_>; 10] = [
        (
            "P1",
            dagger_vs_medium(attacker(1, 1, 1, Some(knife()))),
            &poring,
            (12, 12),
            &[12],
        ),
        (
            "P2-P6",
            dagger_vs_medium(attacker(2, 1, 2, Some(knife()))),
            &poring,
            (14, 14),
            &[14],
        ),
        (
            "P7",
            attacker(15, 11, 15, Some(knife())),
            &lunatic,
            (42, 43),
            &[42, 43],
        ),
        (
            "P8",
            attacker(48, 11, 15, Some(knife())),
            &pupa,
            (104, 105),
            &[104, 105],
        ),
        (
            "P9",
            attacker(49, 12, 15, Some(knife())),
            &pupa,
            (106, 107),
            &[106, 107],
        ),
        ("P10", archer(53, 15), &pupa, (135, 136), &[135, 136]),
        ("P11", archer(20, 15), &pupa, (72, 73), &[72, 73]),
        ("P12", archer(10, 5), &pupa, (52, 54), &[54]),
        ("P13", archer(13, 8), &sphere, (16, 18), &[16, 18]),
        (
            "B",
            attacker(1, 1, 1, Some(knife())),
            &pupa,
            (14, 15),
            &[14, 15],
        ),
        // The P9 crit (line 2960, 149) is checked in MECH-CMB-02.
    ];
    for (phase, a, d, expected, captured) in cases {
        let (low, high) = range(&a, d);
        assert_eq!((low, high), expected, "{phase}");
        for value in captured {
            assert!((low..=high).contains(value), "{phase}: captured {value}");
        }
    }
}

/// MECH-CMB-03: with the engine's default `size_mod` 100 the Knife deals more than rAthena
/// against a Poring (12): the known deviation until the size table is imported.
#[test]
fn mech_cmb_03_default_size_mod_deviation() {
    let poring = defender(&poring());
    assert_eq!(range(&attacker(1, 1, 1, Some(knife())), &poring), (16, 17));
    assert_eq!(AttackerView::default().size_mod, 100);
}

/// MECH-CMB-04: DEX replaces STR in the weapon bonus for a DEX-based weapon; arrow ATK is flat
/// equip ATK (no variance, no size modifier).
#[test]
fn mech_cmb_04_ranged_stat_swap_and_arrow_atk() {
    let open = DefenderView {
        hard_def: 0,
        soft_def: 0,
        ..defender(&poring())
    };
    let heavy = WeaponAttack {
        atk: 100,
        level: 1,
        ..WeaponAttack::default()
    };
    // atk 100, wlv 1: variance 5; bonus 100 × 40 / 200 = 20 from the counted stat.
    let str_based = attacker(0, 40, 0, Some(heavy));
    assert_eq!(range(&str_based, &open), (115, 125));
    let dex_based = attacker(
        0,
        40,
        0,
        Some(WeaponAttack {
            dex_based: true,
            ..heavy
        }),
    );
    assert_eq!(range(&dex_based, &open), (95, 105), "STR ignored");
    let dex = AttackerView {
        dex: 40,
        ..dex_based
    };
    assert_eq!(range(&dex, &open), (115, 125));

    let arrows = AttackerView {
        equip_atk: 25,
        ..attacker(0, 0, 0, None)
    };
    assert_eq!(range(&arrows, &open), (25, 25), "no variance");
    let small = AttackerView {
        size_mod: 50,
        ..arrows
    };
    assert_eq!(range(&small, &open), (25, 25), "no size modifier");
    let halved = AttackerView {
        size_mod: 50,
        equip_atk: 25,
        ..attacker(0, 0, 0, Some(heavy))
    };
    assert_eq!(
        range(&halved, &open),
        (25 + 47, 25 + 52),
        "size applies to the weapon only"
    );
}

/// MECH-CMB-05 (distance part): the client's circular distance.
#[test]
fn mech_cmb_05_check_distance_client() {
    for (dx, dy) in [(0, 0), (1, 0), (0, -1), (1, 1), (-1, -1), (2, 0), (0, -2)] {
        assert!(check_distance_client(dx, dy, 1), "({dx},{dy}) within 1");
    }
    for (dx, dy) in [(2, 1), (-1, 2), (2, 2), (3, 0)] {
        assert!(!check_distance_client(dx, dy, 1), "({dx},{dy}) beyond 1");
    }
    assert!(check_distance_client(3, 0, 2));
    assert!(check_distance_client(2, 2, 2));
    assert!(!check_distance_client(3, 1, 2));
    assert!(check_distance_client(5, 1, 5), "bow range 5");
    assert!(
        check_distance_client(6, 0, 5),
        "straight lines reach one further"
    );
    assert!(!check_distance_client(5, 4, 5));
}

/// MECH-CMB-06: exp share, the attacker bonus and its cap, and the minimum 1.
#[test]
fn mech_cmb_06_exp_share() {
    let config = BattleConfig::default();
    assert_eq!(exp_share(55, 55, 1, &config), 1.0);
    assert_eq!(exp_share(80, 55, 1, &config), 1.0, "clamped to 1");
    assert!((exp_share(70, 100, 2, &config) - 0.7 * 1.25).abs() < 1e-12);
    assert!((exp_share(30, 100, 2, &config) - 0.3 * 1.25).abs() < 1e-12);
    let eleven = 1.0 + 11.0 * 0.25;
    assert!((exp_share(10, 100, 12, &config) - 0.1 * eleven).abs() < 1e-12);
    assert!(
        (exp_share(10, 100, 30, &config) - 0.1 * eleven).abs() < 1e-12,
        "capped at 12 attackers"
    );
    assert_eq!(shared_exp(150, 1.0), 150);
    assert_eq!(shared_exp(150, 0.001), 1, "at least 1");
    assert_eq!(shared_exp(0, 1.0), 0, "a db exp of 0 stays 0");
    // Captured step 8 (Pupa 157/55): A 107 and B 29 of 136 damage. rAthena sent B 43 base
    // after its 105 % level penalty; without the penalty B gets 41.
    let a = exp_share(107, 136, 2, &config);
    let b = exp_share(29, 136, 2, &config);
    assert_eq!((shared_exp(157, a), shared_exp(55, a)), (154, 54));
    assert_eq!((shared_exp(157, b), shared_exp(55, b)), (41, 14));
    // rAthena's order (`mob.cpp` `mob_dead`): the integer (count - 1) × 25 first, then
    // `per × that / 100.`. 4 attackers at 3/5 of JobExp 40 truncate to 42 that way; scaling
    // per × 3 × 25 / 100 in floats gives 41 (final review finding 1).
    let four = exp_share(3, 5, 4, &config);
    assert_eq!(shared_exp(40, four), 42);
}
