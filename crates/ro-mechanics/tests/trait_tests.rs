//! Trait stats, their derived values, MaxAP and trait-point costs (spec `specs/traits`
//! MECH-TRAIT-01..04; catalog `mechanics/trait_stats.md` §1, §2, §4 and the captured values in
//! §7). Job rows are small hand-written samples.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_mechanics::{
    calc_status, next_trait_cost, raise_trait, BattleConfig, BonusTotals, ClassFlags, GearInput,
    HandInput, JobRow, Primary, StatRaise, Status, StatusInputs, Traits,
};

fn traits(pow: i32, sta: i32, wis: i32, spl: i32, con: i32, crt: i32) -> Traits {
    Traits {
        pow,
        sta,
        wis,
        spl,
        con,
        crt,
    }
}

fn ones() -> Primary {
    Primary {
        str: 1,
        agi: 1,
        vit: 1,
        int: 1,
        dex: 1,
        luk: 1,
    }
}

const DK: ClassFlags = ClassFlags {
    upper: false,
    baby: false,
    third: true,
    fourth: true,
    trait_job: true,
    summoner: false,
    extended: false,
};

/// The base AP the shipped data gives every job: 0 below base level 200, then 200.
fn base_ap(base_level: u16) -> u32 {
    if base_level >= 200 {
        200
    } else {
        0
    }
}

fn run(
    allocated: Traits,
    base_level: u16,
    job: JobRow,
    class: ClassFlags,
    config: &BattleConfig,
) -> Status {
    let zero = BonusTotals::default();
    let gear = GearInput {
        right: HandInput::bare(40),
        ..GearInput::default()
    };
    calc_status(
        &StatusInputs {
            allocated: ones(),
            traits: allocated,
            base_level,
            job,
            class,
            gear: &gear,
            base_bonus: &zero,
            battle_bonus: &zero,
        },
        config,
    )
}

/// The captured Dragon Knight at J1 (job bonus STR +1, POW +1), STR..LUK all allocated 1.
fn dk(base_level: u16, allocated: Traits) -> Status {
    let job = JobRow {
        bonus: Primary {
            str: 1,
            ..Primary::default()
        },
        trait_bonus: traits(1, 0, 0, 0, 0, 0),
        base_ap: base_ap(base_level),
        ..JobRow::default()
    };
    run(allocated, base_level, job, DK, &BattleConfig::default())
}

/// MECH-TRAIT-01: the captured Dragon Knight values (`rathena_traits_sequence.txt`, catalog
/// `trait_stats.md` §7).
#[test]
fn mech_trait_01_captured_dragon_knight() {
    let none = Traits::default();
    // batk = (2*10 + 1*2 + 1*10/3 + lv*10/4) / 10 + 5 * POW, POW = job bonus + allocated.
    let b200 = dk(200, none);
    assert_eq!(b200.batk, 57, "B200, POW 1 from the job bonus");
    assert_eq!(b200.trait_base, traits(1, 0, 0, 0, 0, 0));
    assert_eq!(b200.trait_battle, b200.trait_base);
    assert_eq!((b200.hit, b200.flee), (376, 301));
    let b205 = dk(205, traits(3, 0, 0, 0, 4, 0));
    assert_eq!(b205.batk, 73, "B205 with POW 4");
    // HIT/FLEE +2 per CON (lines 516-541): 389/314 at CON 4.
    assert_eq!((b205.hit, b205.flee), (389, 314));
    // PATK = POW/3 + CON/5: 0 at POW 2, 1 at POW 3 (line 497), still 1 at POW 4 with CON 4.
    assert_eq!(dk(200, traits(1, 0, 0, 0, 0, 0)).patk, 0);
    assert_eq!(dk(200, traits(2, 0, 0, 0, 0, 0)).patk, 1);
    assert_eq!(b205.patk, 1);
    // RES = STA + STA/3*5 for STA 1..7.
    let res: Vec<i32> = (1..=7)
        .map(|sta| dk(205, traits(0, sta, 0, 0, 0, 0)).res)
        .collect();
    assert_eq!(res, vec![1, 2, 8, 9, 10, 16, 17]);
    // HPLUS = CRT; CRATE = CRT/3: 1..5 at CRT 3, 6, 9, 12, 15.
    for crt in 1..=17 {
        let status = dk(205, traits(0, 0, 0, 0, 0, crt));
        assert_eq!(status.hplus, crt);
        assert_eq!(status.crate_, crt / 3);
    }
    assert_eq!(dk(205, traits(0, 0, 0, 0, 0, 15)).crate_, 5);
    // The traits feed nothing else: MaxHP, ASPD, CRIT and DEF match the trait-less DK.
    let plain = dk(205, none);
    let full = dk(205, traits(20, 20, 20, 20, 20, 20));
    assert_eq!(
        (full.max_hp, full.max_sp, full.amotion, full.cri),
        (plain.max_hp, plain.max_sp, plain.amotion, plain.cri)
    );
    assert_eq!(
        (
            full.soft_def,
            full.hard_def,
            full.soft_mdef,
            full.status_matk
        ),
        (
            plain.soft_def,
            plain.hard_def,
            plain.soft_mdef,
            plain.status_matk
        )
    );
}

/// MECH-TRAIT-02: capture 3's Hyper Novice J52 (`attribute_sequence_3.txt` lines 962-973) has
/// only its job bonus POW..CRT 9/4/4/8/8/3, giving PATK..CRATE 4/3/9/9/3/1. SMATK and MRES on
/// hand values follow the catalog formulas.
#[test]
fn mech_trait_02_hyper_novice_bonus_and_smatk_mres() {
    let hyper = ClassFlags {
        trait_job: true,
        ..ClassFlags::default()
    };
    let job = JobRow {
        trait_bonus: traits(9, 4, 4, 8, 8, 3),
        base_ap: 200,
        ..JobRow::default()
    };
    let status = run(Traits::default(), 201, job, hyper, &BattleConfig::default());
    assert_eq!(status.trait_battle, traits(9, 4, 4, 8, 8, 3));
    assert_eq!(
        (
            status.patk,
            status.smatk,
            status.res,
            status.mres,
            status.hplus,
            status.crate_
        ),
        (4, 3, 9, 9, 3, 1)
    );
    // SPL 9, WIS 6, CON 0: SMATK 9/3 = 3; MRES 6 + 6/3*5 = 16.
    let status = run(
        traits(0, 0, 6, 9, 0, 0),
        201,
        JobRow::default(),
        hyper,
        &BattleConfig::default(),
    );
    assert_eq!((status.smatk, status.mres), (3, 16));
}

/// MECH-TRAIT-03: MaxAP is the table value, scaled by `ap_rate` and capped at `max_ap`.
#[test]
fn mech_trait_03_max_ap() {
    assert_eq!(dk(200, Traits::default()).max_ap, 200);
    assert_eq!(dk(199, Traits::default()).max_ap, 0);
    let job = JobRow {
        base_ap: 200,
        ..JobRow::default()
    };
    let at = |config: &BattleConfig| run(Traits::default(), 200, job, DK, config).max_ap;
    let half = BattleConfig {
        ap_rate: 50,
        ..BattleConfig::default()
    };
    assert_eq!(at(&half), 100);
    let capped = BattleConfig {
        max_ap: 150,
        ..BattleConfig::default()
    };
    assert_eq!(at(&capped), 150);
    // No job guard: a 3rd class at B200 with the table's 200 has MaxAP 200 (catalog §2).
    let third = ClassFlags {
        third: true,
        ..ClassFlags::default()
    };
    assert_eq!(
        run(Traits::default(), 200, job, third, &BattleConfig::default()).max_ap,
        200
    );
}

/// MECH-TRAIT-04: one point per trait level, the cap from the class (catalog §4).
#[test]
fn mech_trait_04_trait_cost_and_cap() {
    let config = BattleConfig::default();
    assert_eq!(config.max_trait(&DK), 110);
    assert_eq!(config.max_trait(&ClassFlags::default()), 0);
    assert_eq!(next_trait_cost(0, 110), 1);
    assert_eq!(next_trait_cost(109, 110), 1);
    assert_eq!(next_trait_cost(110, 110), 0);
    assert_eq!(next_trait_cost(0, 0), 0, "non-trait job");
    let raise = |current, requested, points, cap| raise_trait(current, requested, points, cap);
    assert_eq!(raise(0, 1, 7, 110), StatRaise { amount: 1, cost: 1 });
    assert_eq!(
        raise(0, 5, 3, 110),
        StatRaise { amount: 3, cost: 3 },
        "capped by the points"
    );
    assert_eq!(
        raise(108, 5, 10, 110),
        StatRaise { amount: 2, cost: 2 },
        "capped by the cap"
    );
    assert_eq!(raise(110, 1, 10, 110), StatRaise::default());
    assert_eq!(raise(0, 1, 0, 110), StatRaise::default());
    assert_eq!(raise(0, 1, 5, 0), StatRaise::default(), "non-trait job");
    assert_eq!(raise(0, 0, 5, 110), StatRaise::default());
}
