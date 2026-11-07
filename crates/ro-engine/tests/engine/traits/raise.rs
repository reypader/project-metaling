//! Raising trait stats (ENG-TRAIT-01, 02).

use super::*;

/// ENG-TRAIT-01: trait-up success in the captured order (`rathena_traits_sequence.txt`
/// 488-506, 515-521): the diff (`ATK1` +5, then POW `0x0141` base = allocated, bonus = the job's
/// POW 1, then `PATK` once POW totals 3), `0x00be` `SP_UPOW` 1, `SP_TRAITPOINT`, then `0x00bc`
/// with the allocated value.
#[test]
fn eng_trait_01_pow_up_in_the_captured_order() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 7, 0)));

    let first = raise(&mut world, SP_POW as u16, 1);
    assert_eq!(
        first[first.len() - 4..],
        [
            couple(SP_POW, 1, 1),
            cost(SP_UPOW, 1),
            p(SP_TRAITPOINT, 6),
            ack(SP_POW as u16, true, 1),
        ],
        "{first:?}"
    );
    assert_eq!(first.len(), 5, "ATK1, then the tail: {first:?}");

    let second = raise(&mut world, SP_POW as u16, 1);
    assert_eq!(
        second[1..],
        [
            couple(SP_POW, 2, 1),
            p(SP_PATK, 1),
            cost(SP_UPOW, 1),
            p(SP_TRAITPOINT, 5),
            ack(SP_POW as u16, true, 2),
        ],
        "{second:?}"
    );
    assert_eq!(
        param_value(&second, SP_ATK1) - param_value(&first, SP_ATK1),
        5,
        "status ATK +5 per POW"
    );
    let status = world.status();
    assert_eq!((status.traits[0], status.trait_point), (2, 5));
}

/// ENG-TRAIT-01: CON pushes `HIT` and `FLEE1` (+2 each) before its own `0x0141` (captured
/// 516-521).
#[test]
fn eng_trait_01_con_up_pushes_hit_and_flee_first() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 7, 0)));
    let before = world.engine.progress_snapshot(char_id(A)).expect("on map");
    assert_eq!(before.status.trait_point, 7);
    let out = raise(&mut world, SP_CON, 1);
    let vars: Vec<u16> = out
        .iter()
        .map(|e| match e {
            ServerEvent::ParameterChanged { var_id, .. } => *var_id,
            ServerEvent::CoupleStatus { var_id, .. } => *var_id as u16,
            ServerEvent::StatusCost { var_id, .. } => *var_id,
            ServerEvent::StatusChangeAck { var_id, .. } => *var_id,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        vars,
        vec![SP_HIT, SP_FLEE1, SP_CON, SP_UPOW + 4, SP_TRAITPOINT, SP_CON]
    );
    assert_eq!(out[2], couple(SP_CON.into(), 1, 0));
    assert_eq!(out[5], ack(SP_CON, true, 1));
}

/// ENG-TRAIT-01: an amount above the pool is capped to it, not refused (source-only).
#[test]
fn eng_trait_01_amount_is_capped_by_the_points() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 3, 0)));
    let out = raise(&mut world, SP_POW as u16, 5);
    assert_eq!(
        out[out.len() - 3..],
        [
            cost(SP_UPOW, 1),
            p(SP_TRAITPOINT, 0),
            ack(SP_POW as u16, true, 3),
        ]
    );
    let status = world.status();
    assert_eq!((status.traits[0], status.trait_point), (3, 0));

    // ...and by the cap (110): the last point costs 0 next.
    let mut world = World::new();
    world.login(spawn(
        DRAGON_KNIGHT,
        200,
        dk_status([108, 0, 0, 0, 0, 0], 9, 0),
    ));
    let out = raise(&mut world, SP_POW as u16, 5);
    assert_eq!(
        out[out.len() - 3..],
        [
            cost(SP_UPOW, 0),
            p(SP_TRAITPOINT, 7),
            ack(SP_POW as u16, true, 110),
        ]
    );
}

/// ENG-TRAIT-02: every failure sends only `0x00bc` ok 0 value 0 for the requested var, at once,
/// and changes nothing (source-only: the client blocks 0-point requests).
#[test]
fn eng_trait_02_failures_send_only_the_refusal() {
    let cases = [
        ("non-trait job", 7, [0; 6], 5, SP_POW as u16, 1),
        ("no points", DRAGON_KNIGHT, [0; 6], 0, SP_POW as u16, 1),
        (
            "at the cap",
            DRAGON_KNIGHT,
            [110, 0, 0, 0, 0, 0],
            5,
            SP_POW as u16,
            1,
        ),
        ("a primary stat", DRAGON_KNIGHT, [0; 6], 5, 13, 1),
        ("amount 0", DRAGON_KNIGHT, [0; 6], 5, SP_POW as u16, 0),
    ];
    for (case, job, traits, points, var, amount) in cases {
        let mut world = World::new();
        world.login(spawn(job, 200, dk_status(traits, points, 0)));
        let out = raise(&mut world, var, amount);
        assert_eq!(out, vec![ack(var, false, 0)], "{case}");
        let status = world.status();
        assert_eq!(
            (status.traits, status.trait_point),
            (traits, points),
            "{case}"
        );
    }
}

/// ENG-TRAIT-02: a negative amount is dropped with no reply.
#[test]
fn eng_trait_02_negative_amount_is_dropped() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 5, 0)));
    let out = raise(&mut world, SP_POW as u16, -1);
    assert!(out.is_empty(), "{out:?}");
    let status = world.status();
    assert_eq!((status.traits, status.trait_point), ([0; 6], 5));
}
