//! Stat-point costs and per-class caps (spec MECH-05; catalog `mechanics/stats.md` "Stat point
//! costs and limits", `mechanics/battle_config.md` §8; captured raises from
//! `captures/attribute_sequence.txt`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_mechanics::{next_stat_cost, raise_stat, stat_cost, BattleConfig, ClassFlags, StatRaise};

/// Catalog examples of `PC_STATUS_POINT_COST`.
#[test]
fn mech_05_cost_per_step() {
    for (low, cost) in [
        (1, 2),
        (10, 2),
        (11, 3),
        (50, 6),
        (99, 11),
        (100, 16),
        (105, 20),
        (129, 36),
    ] {
        assert_eq!(stat_cost(low), cost, "{low} -> {}", low + 1);
    }
    let total = |to: u16| (1..to).map(stat_cost).sum::<u32>();
    assert_eq!(total(99), 628);
    assert_eq!(total(130), 1419);
}

#[test]
fn mech_05_next_cost_is_zero_at_the_cap() {
    assert_eq!(next_stat_cost(98, 99), 11);
    assert_eq!(
        next_stat_cost(99, 99),
        0,
        "captured: INT 99 shows 0 (line 243)"
    );
    assert_eq!(next_stat_cost(120, 99), 0);
    assert_eq!(next_stat_cost(129, 130), 36);
}

/// An over-affordable request raises as far as points and the cap allow (captured lines 236,
/// 4508, 4531).
#[test]
fn mech_05_raise_as_far_as_possible() {
    assert_eq!(
        raise_stat(3, 99, 1198, 99),
        StatRaise {
            amount: 96,
            cost: 624
        },
        "INT 3 +99 at cap 99: 1198 -> 574 points"
    );
    assert_eq!(
        raise_stat(1, 231, 1273, 99),
        StatRaise {
            amount: 98,
            cost: 628
        },
        "STR +231: stops at 99"
    );
    assert_eq!(
        raise_stat(1, 99, 17, 99),
        StatRaise {
            amount: 8,
            cost: 16
        },
        "DEX +99 with 17 points: 1 point left"
    );
    assert_eq!(
        raise_stat(99, 1, 500, 99),
        StatRaise::default(),
        "at the cap"
    );
    assert_eq!(
        raise_stat(10, 1, 1, 99),
        StatRaise::default(),
        "too few points"
    );
    assert_eq!(
        raise_stat(10, 0, 100, 99),
        StatRaise::default(),
        "nothing asked"
    );
}

/// Per-class caps in the catalog's first-match order, with the shipped values.
#[test]
fn mech_05_caps_per_class() {
    let config = BattleConfig::default();
    let cap = |class: ClassFlags| config.max_stat(&class);
    assert_eq!(cap(ClassFlags::default()), 99);
    let upper = ClassFlags {
        upper: true,
        ..ClassFlags::default()
    };
    assert_eq!(cap(upper), 99);
    let baby = ClassFlags {
        baby: true,
        ..ClassFlags::default()
    };
    assert_eq!(cap(baby), 80);
    let third = ClassFlags {
        third: true,
        ..ClassFlags::default()
    };
    assert_eq!(cap(third), 130);
    assert_eq!(
        cap(ClassFlags {
            third: true,
            baby: true,
            ..ClassFlags::default()
        }),
        117
    );
    assert_eq!(
        cap(ClassFlags {
            third: true,
            fourth: true,
            trait_job: true,
            ..ClassFlags::default()
        }),
        130
    );
    let custom = BattleConfig {
        max_baby_parameter: 70,
        max_fourth_parameter: 150,
        ..BattleConfig::default()
    };
    assert_eq!(custom.max_stat(&baby), 70);
    assert_eq!(
        custom.max_stat(&ClassFlags {
            baby: true,
            trait_job: true,
            ..ClassFlags::default()
        }),
        70,
        "baby is checked before the 4th-class rule"
    );
    assert_eq!(
        custom.max_stat(&ClassFlags {
            third: true,
            fourth: true,
            trait_job: true,
            ..ClassFlags::default()
        }),
        150
    );
}
