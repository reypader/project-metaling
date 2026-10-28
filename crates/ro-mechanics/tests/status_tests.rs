//! `calc_status` against the captured status windows (spec MECH-01..04; catalog
//! `mechanics/stats.md` "Captured status values"). Job rows are small hand-written samples
//! holding only the values each case needs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_mechanics::{
    calc_status, BattleConfig, BonusTotals, ClassFlags, GearInput, HandInput, JobRow, Primary,
    RefineInput, Status, StatusInputs, WeaponClass,
};

fn primary(str: i32, agi: i32, vit: i32, int: i32, dex: i32, luk: i32) -> Primary {
    Primary {
        str,
        agi,
        vit,
        int,
        dex,
        luk,
    }
}

fn all(v: i32) -> Primary {
    primary(v, v, v, v, v, v)
}

/// A one-handed weapon at refine 0.
fn weapon(class: WeaponClass, atk: u32, aspd_base: u16) -> HandInput {
    HandInput {
        class,
        atk,
        aspd_base,
        ..HandInput::default()
    }
}

struct Case {
    allocated: Primary,
    base_level: u16,
    job: JobRow,
    class: ClassFlags,
    gear: GearInput,
}

impl Case {
    fn run(&self) -> Status {
        let zero = BonusTotals::default();
        calc_status(
            &StatusInputs {
                allocated: self.allocated,
                base_level: self.base_level,
                job: self.job,
                class: self.class,
                gear: &self.gear,
                base_bonus: &zero,
                battle_bonus: &zero,
            },
            &BattleConfig::default(),
        )
    }
}

/// Novice (job bonus `bonus`), Knife (Dagger base 55, ATK 17) and Cotton Shirt (DEF 10).
fn novice(base_level: u16, base_hp: u32, base_sp: u32, bonus: Primary, allocated: Primary) -> Case {
    Case {
        allocated,
        base_level,
        job: JobRow {
            bonus,
            base_hp,
            base_sp,
            max_weight: 20000,
            shield_aspd: 10,
        },
        class: ClassFlags::default(),
        gear: GearInput {
            right: weapon(WeaponClass::Melee, 17, 55),
            item_def: 10,
            ..GearInput::default()
        },
    }
}

/// The window values in capture order: amotion, ATK1/ATK2, MATK2/MATK1, DEF1/DEF2 (soft/hard),
/// MDEF1/MDEF2, HIT, FLEE1, FLEE2, CRIT, MaxHP, MaxSP, MaxWeight.
fn window(s: &Status) -> [i64; 16] {
    [
        s.amotion.into(),
        s.batk.into(),
        s.watk.into(),
        s.status_matk.into(),
        s.weapon_matk.into(),
        s.soft_def.into(),
        s.hard_def.into(),
        s.soft_mdef.into(),
        s.hard_mdef.into(),
        s.hit.into(),
        s.flee.into(),
        s.flee2_shown().into(),
        s.critical_shown().into(),
        s.max_hp.into(),
        s.max_sp.into(),
        s.max_weight.into(),
    ]
}

/// MECH-01: the captured Novice windows (lines 294-321, 402-416, 435-450, 681-695).
#[test]
fn mech_01_captured_novice_windows() {
    let b1 = novice(1, 40, 11, all(0), all(1)).run();
    assert_eq!(
        window(&b1),
        [590, 1, 17, 1, 0, 1, 10, 1, 0, 177, 102, 1, 1, 40, 11, 20300]
    );
    assert_eq!((b1.battle, b1.base), (all(1), all(1)));

    let b11 = novice(11, 90, 21, all(0), all(1)).run();
    assert_eq!(
        window(&b11),
        [590, 4, 17, 3, 0, 6, 10, 4, 0, 187, 112, 1, 1, 90, 21, 20300]
    );

    let j10 = novice(11, 90, 21, all(1), all(1)).run();
    assert_eq!(
        window(&j10),
        [590, 5, 17, 5, 0, 6, 10, 5, 0, 188, 113, 1, 1, 91, 21, 20300]
    );
    assert_eq!(j10.battle, all(2), "the job bonus is in the battle stats");

    let b6 = novice(6, 65, 16, all(1), primary(3, 4, 1, 12, 1, 1)).run();
    assert_eq!(
        window(&b6),
        [590, 6, 17, 20, 0, 5, 10, 15, 0, 183, 111, 1, 1, 66, 18, 20900]
    );
}

/// MECH-01: Swordman B37 J1 (lines 1729-1747) and the Knight relog (lines 4576-4602, 4632).
#[test]
fn mech_01_captured_swordman_and_knight() {
    let swordman = Case {
        allocated: primary(13, 21, 17, 19, 29, 16),
        base_level: 37,
        job: JobRow {
            bonus: all(0),
            base_hp: 713,
            base_sp: 84,
            max_weight: 28000,
            shield_aspd: 5,
        },
        class: ClassFlags::default(),
        gear: GearInput {
            right: weapon(WeaponClass::Melee, 17, 47),
            item_def: 10,
            ..GearInput::default()
        },
    }
    .run();
    assert_eq!(
        window(&swordman),
        [470, 33, 17, 47, 0, 31, 10, 37, 0, 246, 161, 2, 6, 834, 99, 31900]
    );

    let knight = Case {
        allocated: primary(99, 99, 1, 1, 9, 1),
        base_level: 99,
        job: JobRow {
            bonus: primary(0, 0, 1, 0, 0, 0),
            base_hp: 7978,
            base_sp: 307,
            max_weight: 28000,
            shield_aspd: 5,
        },
        class: ClassFlags::default(),
        gear: GearInput {
            right: HandInput::bare(40),
            item_def: 10,
            ..GearInput::default()
        },
    }
    .run();
    assert_eq!(
        window(&knight),
        [270, 125, 0, 26, 0, 70, 10, 27, 0, 283, 298, 1, 2, 8137, 310, 57700]
    );
}

/// MECH-01: the Bard B99 window (lines 45-72, 110). Its gear isn't known, so weapon ATK and
/// hard DEF are fed in as captured, and MaxSP (630, an unknown gear bonus on top of the
/// table's 610) is not checked.
#[test]
fn mech_01_captured_bard() {
    let bard = Case {
        allocated: primary(11, 10, 1, 1, 10, 1),
        base_level: 99,
        job: JobRow {
            bonus: primary(0, 0, 0, 0, 1, 0),
            base_hp: 4056,
            base_sp: 604,
            max_weight: 27000,
            shield_aspd: 7,
        },
        class: ClassFlags::default(),
        gear: GearInput {
            right: weapon(WeaponClass::Ranged, 142, 45),
            item_def: 60,
            ..GearInput::default()
        },
    }
    .run();
    let w = window(&bard);
    assert_eq!(
        w[..14],
        [470, 38, 142, 27, 0, 52, 60, 28, 0, 285, 209, 1, 2, 4096]
    );
    assert_eq!(bard.max_weight, 30300);
}

/// A bare level-1 Novice: amotion 440 (catalog worked check, ASPD 156).
#[test]
fn mech_01_bare_novice_aspd() {
    let mut case = novice(1, 40, 11, all(0), all(1));
    case.gear.right = HandInput::bare(40);
    assert_eq!(case.run().amotion, 440);
}

/// MECH-02: refine. Weapon `atk2` and MATK add `Bonus / 100` (no MATK on bows); armor adds
/// `(Σ Bonus + 50) / 100` once.
#[test]
fn mech_02_refine() {
    let mut case = novice(1, 40, 11, all(0), all(1));
    case.gear.right.refine = RefineInput {
        bonus: 2000,
        random_bonus: 900,
    };
    let knife10 = case.run();
    assert_eq!((knife10.watk, knife10.weapon_matk), (37, 20), "Knife +10");

    case.gear.right = HandInput {
        class: WeaponClass::Melee,
        atk: 53,
        refine: RefineInput {
            bonus: 1200,
            random_bonus: 0,
        },
        aspd_base: 47,
        ..HandInput::default()
    };
    let blade6 = case.run();
    assert_eq!((blade6.watk, blade6.weapon_matk), (65, 12), "Blade +6");

    case.gear.right = HandInput {
        class: WeaponClass::Melee,
        matk: 30,
        refine: RefineInput {
            bonus: 2000,
            random_bonus: 0,
        },
        aspd_base: 65,
        ..HandInput::default()
    };
    assert_eq!(case.run().weapon_matk, 50, "Rod +10");

    case.gear.right = HandInput {
        class: WeaponClass::Bow,
        atk: 30,
        matk: 5,
        refine: RefineInput {
            bonus: 1000,
            random_bonus: 0,
        },
        aspd_base: 50,
    };
    let bow = case.run();
    assert_eq!(
        (bow.watk, bow.weapon_matk),
        (40, 5),
        "a bow gets ATK refine, no MATK"
    );

    case.gear = GearInput {
        right: HandInput::bare(40),
        item_def: 22,
        armor_refine: 1800 + 1800,
        ..GearInput::default()
    };
    assert_eq!(case.run().hard_def, 22 + 36, "Hat +10 and Silk Robe +10");
    case.gear.armor_refine = 120 * 3;
    assert_eq!(
        case.run().hard_def,
        22 + 4,
        "rounded once on the sum: (360 + 50) / 100, not 1 + 1 + 1"
    );
}

fn swordman_b48(gear: GearInput) -> Status {
    Case {
        allocated: primary(13, 21, 19, 19, 32, 16),
        base_level: 48,
        job: JobRow {
            bonus: primary(1, 0, 1, 0, 1, 0),
            base_hp: 1100,
            base_sp: 106,
            max_weight: 28000,
            shield_aspd: 5,
        },
        class: ClassFlags::default(),
        gear,
    }
    .run()
}

/// MECH-03: ASPD steps, shields, dual wield and the class caps.
#[test]
fn mech_03_aspd() {
    // Novice B11+ J10 with the Knife, at the captured battle AGI/DEX (allocated + 1).
    for (agi, dex, amotion) in [
        (1, 1, 590),
        (6, 2, 580),
        (12, 2, 570),
        (17, 2, 560),
        (22, 9, 550),
        (22, 29, 540),
    ] {
        let case = novice(26, 165, 36, all(1), primary(1, agi - 1, 1, 1, dex - 1, 1));
        assert_eq!(case.run().amotion, amotion, "AGI {agi} DEX {dex}");
    }

    // Swordman B48 J11 (lines 1953-2097): Blade 460, + Guard 510, Guard alone 440, bare 390.
    let blade = weapon(WeaponClass::Melee, 53, 47);
    let bare = HandInput::bare(40);
    let with = |right: HandInput, shield: bool| {
        swordman_b48(GearInput {
            right,
            shield,
            ..GearInput::default()
        })
        .amotion
    };
    assert_eq!(with(blade, false), 460);
    assert_eq!(with(blade, true), 510);
    assert_eq!(with(bare, true), 440);
    assert_eq!(with(bare, false), 390);

    // Dual wield: the left weapon adds a quarter of its base.
    let dual = swordman_b48(GearInput {
        right: blade,
        left: Some(weapon(WeaponClass::Melee, 30, 40)),
        ..GearInput::default()
    });
    assert_eq!(dual.amotion, 460 + 100, "47 + 40/4 = 57");
    assert_eq!(dual.watk, 53 + 30, "both hands' ATK");

    // Caps: amotion never goes below 2000 - cap*10 (190 -> 100, 193 -> 70).
    let fast = |class: ClassFlags| {
        Case {
            allocated: primary(1, 400, 1, 1, 400, 1),
            base_level: 99,
            job: JobRow {
                bonus: all(0),
                base_hp: 40,
                base_sp: 10,
                max_weight: 20000,
                shield_aspd: 5,
            },
            class,
            gear: GearInput {
                right: HandInput::bare(40),
                ..GearInput::default()
            },
        }
        .run()
        .amotion
    };
    assert_eq!(fast(ClassFlags::default()), 100);
    let third = ClassFlags {
        third: true,
        ..ClassFlags::default()
    };
    assert_eq!(fast(third), 70);
    let summoner = ClassFlags {
        summoner: true,
        ..ClassFlags::default()
    };
    assert_eq!(fast(summoner), 70);
}

/// MECH-03: the katar doubles CRITICAL; a ranged weapon swaps STR and DEX in status ATK.
#[test]
fn mech_03_katar_and_ranged() {
    let mut case = novice(99, 40, 10, all(0), primary(50, 1, 1, 1, 10, 1));
    case.gear.right = weapon(WeaponClass::Katar, 30, 42);
    let katar = case.run();
    case.gear.right = weapon(WeaponClass::Melee, 30, 42);
    let melee = case.run();
    assert_eq!(katar.cri, melee.cri * 2);
    case.gear.right = weapon(WeaponClass::Bow, 30, 50);
    let bow = case.run();
    // Melee: (500 + 100/5 + 10/3 + 990/4) / 10; ranged swaps STR and DEX.
    assert_eq!(melee.batk, (500 + 20 + 3 + 247) / 10);
    assert_eq!(bow.batk, (100 + 100 + 3 + 247) / 10);
}

/// MECH-04: MaxHP/MaxSP: table x VIT/INT %, x1.25 for upper and 4th classes, rates and caps.
#[test]
fn mech_04_max_hp_sp() {
    let job = |base_hp, base_sp| JobRow {
        bonus: all(0),
        base_hp,
        base_sp,
        max_weight: 28000,
        shield_aspd: 5,
    };
    let run = |allocated, base_level, job, class| {
        Case {
            allocated,
            base_level,
            job,
            class,
            gear: GearInput {
                right: HandInput::bare(40),
                ..GearInput::default()
            },
        }
        .run()
    };
    // Lord Knight B48 (line 2259): 2049 x 1.19 x 1.25 = 3047; SP 154 x 1.19 x 1.25 = 229.
    let upper = ClassFlags {
        upper: true,
        ..ClassFlags::default()
    };
    let lk = run(primary(13, 21, 19, 19, 32, 16), 48, job(2049, 154), upper);
    assert_eq!((lk.max_hp, lk.max_sp), (3047, 229));
    // Rune Knight B48 after a reset (line 2666): no x1.25 for a non-trans 3rd class.
    let third = ClassFlags {
        third: true,
        ..ClassFlags::default()
    };
    let rk = run(primary(1, 1, 1, 1, 1, 1), 48, job(2049, 154), third);
    assert_eq!(rk.max_hp, 2069);
    // Shadow Cross B170 (lines 3753, 3789): fallback table 18513 / 1034, 4th class x1.25.
    let fourth = ClassFlags {
        third: true,
        fourth: true,
        trait_job: true,
        ..ClassFlags::default()
    };
    let sc = run(primary(3, 130, 4, 3, 130, 6), 170, job(18513, 1034), fourth);
    assert_eq!((sc.max_hp, sc.max_sp), (24066, 1331));
    let sc60 = run(
        primary(3, 130, 10, 8, 130, 6),
        170,
        job(18513, 1034),
        fourth,
    );
    assert_eq!((sc60.max_hp, sc60.max_sp), (25455, 1395));

    // Level-bracket caps and the hp_rate knob.
    let huge = job(400_000, 2_000_000);
    let b99 = run(all(1), 99, huge, ClassFlags::default());
    assert_eq!((b99.max_hp, b99.max_sp), (330_000, 1_000_000));
    let b120 = run(all(1), 120, job(700_000, 10), ClassFlags::default());
    assert_eq!(b120.max_hp, 660_000);
    let zero = BonusTotals::default();
    let gear = GearInput {
        right: HandInput::bare(40),
        ..GearInput::default()
    };
    let config = BattleConfig {
        hp_rate: 50,
        ..BattleConfig::default()
    };
    let halved = calc_status(
        &StatusInputs {
            allocated: all(1),
            base_level: 1,
            job: job(1000, 10),
            class: ClassFlags::default(),
            gear: &gear,
            base_bonus: &zero,
            battle_bonus: &zero,
        },
        &config,
    );
    assert_eq!(halved.max_hp, 505, "1010 x 50 / 100");
}

/// Bonus slots feed the base and battle layers; `0x0141` reports `battle - allocated`.
#[test]
fn bonus_layers() {
    use ro_mechanics::BonusSlot;
    let mut base_bonus = BonusTotals::default();
    base_bonus.add(BonusSlot::Str, 3);
    base_bonus.add(BonusSlot::Hit, 7);
    let mut battle_bonus = BonusTotals::default();
    battle_bonus.add(BonusSlot::Agi, 5);
    let case = novice(1, 40, 11, all(1), all(1));
    let status = calc_status(
        &StatusInputs {
            allocated: case.allocated,
            base_level: case.base_level,
            job: case.job,
            class: case.class,
            gear: &case.gear,
            base_bonus: &base_bonus,
            battle_bonus: &battle_bonus,
        },
        &BattleConfig::default(),
    );
    assert_eq!(status.base, primary(5, 2, 2, 2, 2, 2));
    assert_eq!(status.battle, primary(5, 7, 2, 2, 2, 2));
    // Level 1 + DEX 2 + LUK 2 / 3 + 175 + the Hit slot.
    assert_eq!(status.hit, 1 + 2 + 175 + 7);
    assert_eq!(status.flee, 1 + 7 + 100, "FLEE reads the battle AGI");
}
