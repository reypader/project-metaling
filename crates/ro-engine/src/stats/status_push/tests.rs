use ro_mechanics::{Primary, Traits};

use super::*;

fn p(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

/// STR..LUK allocated 1, no traits, HP/SP/AP `vitals`.
fn persisted(vitals: (u32, u32, u32)) -> Persisted {
    let mut allocated = [0; 12];
    allocated[..6].fill(1);
    Persisted {
        allocated,
        hp: vitals.0,
        sp: vitals.1,
        ap: vitals.2,
    }
}

fn diff(old: &Status, new: &Status) -> Vec<ServerEvent> {
    let same = persisted((0, 0, 0));
    status_diff(old, new, (&same, &same))
}

/// ENG-STAT-01: only changed fields are pushed, in the catalog order.
#[test]
fn eng_stat_01_diff_pushes_changes_in_order() {
    let old = Status::default();
    assert!(diff(&old, &old).is_empty(), "no change, no packets");

    let atk = Status { watk: 17, ..old };
    assert_eq!(diff(&old, &atk), vec![p(42, 17)]);

    let def = Status {
        hard_def: 20,
        ..old
    };
    assert_eq!(diff(&old, &def), vec![p(45, 0), p(46, 20)]);

    let soft = Status { soft_def: 3, ..old };
    assert_eq!(diff(&old, &soft), vec![p(46, 0), p(45, 3)]);

    let range = Status { range: 5, ..old };
    assert_eq!(
        diff(&old, &range),
        vec![ServerEvent::AttackRange { range: 5 }]
    );

    let all = Status {
        hit: 1,
        flee: 2,
        amotion: 3,
        batk: 4,
        hard_def: 5,
        watk: 6,
        soft_def: 7,
        flee2: 80,
        cri: 90,
        weapon_matk: 10,
        hard_mdef: 11,
        soft_mdef: 12,
        range: 13,
        max_hp: 14,
        max_sp: 15,
        ..old
    };
    assert_eq!(
        status_diff(&old, &all, (&persisted((0, 0, 0)), &persisted((14, 15, 0)))),
        vec![
            p(49, 1),
            p(50, 2),
            p(53, 3),
            p(41, 4),
            p(45, 7),
            p(46, 5),
            p(42, 6),
            p(46, 5),
            p(45, 7),
            p(51, 8),
            p(52, 9),
            p(44, 0),
            p(43, 10),
            p(47, 12),
            p(48, 11),
            p(48, 11),
            p(47, 12),
            ServerEvent::AttackRange { range: 13 },
            p(6, 14),
            p(8, 15),
            p(5, 14),
            p(7, 15),
        ]
    );
}

/// ENG-STAT-01 (extended): `0x0141` first, for each changed battle stat, with `bonus` =
/// battle − allocated; CRITICAL and FLEE2 compare in 1/10 units (captured LUK re-send).
#[test]
fn eng_stat_01_couple_stats_and_internal_units() {
    let old = Status {
        battle: Primary {
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
        },
        cri: 13,
        flee2: 11,
        ..Status::default()
    };
    let new = Status {
        battle: Primary {
            luk: 3,
            ..old.battle
        },
        cri: 19,
        flee2: 13,
        ..old
    };
    let mut more_luk = persisted((40, 11, 0));
    more_luk.allocated[5] = 2;
    assert_eq!(
        status_diff(&old, &new, (&persisted((40, 11, 0)), &more_luk)),
        vec![
            ServerEvent::CoupleStatus {
                var_id: 18,
                base: 2,
                bonus: 1,
            },
            p(51, 1),
            p(52, 1),
        ],
        "shown values unchanged, both re-sent"
    );
}

/// ENG-STAT-01 with traits (traits spec §2.3): after `SP` come the changed POW..CRT
/// `0x0141`s (allocated or battle), then PATK..CRATE, then `MAXAP` and `AP`.
#[test]
fn eng_stat_01_trait_block_after_sp() {
    let old = Status {
        trait_battle: Traits {
            pow: 1,
            ..Traits::default()
        },
        ..Status::default()
    };
    let new = Status {
        hit: 5,
        max_sp: 9,
        trait_battle: Traits {
            pow: 2,
            con: 4,
            ..old.trait_battle
        },
        patk: 1,
        smatk: 2,
        res: 3,
        mres: 4,
        hplus: 5,
        crate_: 6,
        max_ap: 200,
        ..old
    };
    let before = persisted((10, 10, 0));
    let mut after = persisted((10, 20, 30));
    after.allocated[6] = 1; // POW
    after.allocated[10] = 4; // CON
    after.allocated[11] = 3; // CRT: allocated changed, battle (0) the same
    let couple = |var_id, base, bonus| ServerEvent::CoupleStatus {
        var_id,
        base,
        bonus,
    };
    assert_eq!(
        status_diff(&old, &new, (&before, &after)),
        vec![
            p(49, 5),
            p(8, 9),
            p(7, 20),
            couple(219, 1, 1),
            couple(223, 4, 0),
            couple(224, 3, -3),
            p(225, 1),
            p(226, 2),
            p(227, 3),
            p(228, 4),
            p(229, 5),
            p(230, 6),
            p(233, 200),
            p(232, 30),
        ]
    );
    assert!(diff(&new, &new).is_empty());
}

#[test]
fn window_costs_use_the_job_cap() {
    let status = CharacterStatus {
        str: 99,
        agi: 98,
        ..CharacterStatus::default()
    };
    let derived = Status {
        cri: 15,
        flee2: 11,
        amotion: 440,
        ..Status::default()
    };
    let window = status_window(&status, &derived, 99);
    assert_eq!(window.costs[..3], [0, 11, 2]);
    assert_eq!(
        (window.critical, window.perfect_dodge, window.aspd),
        (1, 1, 440)
    );
    assert_eq!(status_window(&status, &derived, 130).costs[0], 11);
}
