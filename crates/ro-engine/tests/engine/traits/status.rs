//! Trait values in the status pushes, and AP clamping (ENG-TRAIT-07, 08).

use super::*;

/// ENG-TRAIT-07: the pre-ready push appends only the non-zero trait values after `SP` and before
/// `WEIGHT` (captured 1401-1403: POW base 2 bonus 1, PATK 1, then WEIGHT).
#[test]
fn eng_trait_07_pre_ready_push_appends_non_zero_trait_values() {
    let mut world = World::new();
    let (burst, _) = world.login(spawn(
        DRAGON_KNIGHT,
        99,
        dk_status([2, 0, 0, 0, 0, 0], 5, 0),
    ));
    let traits: Vec<ServerEvent> = burst
        .iter()
        .filter(|e| is_trait_value(e))
        .cloned()
        .collect();
    assert_eq!(traits, vec![couple(SP_POW, 2, 1), p(SP_PATK, 1)]);
    let sp = burst
        .iter()
        .position(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_SP, .. }))
        .expect("SP pushed");
    let weight = burst
        .iter()
        .position(|e| {
            matches!(
                e,
                ServerEvent::ParameterChanged {
                    var_id: SP_WEIGHT,
                    ..
                }
            )
        })
        .expect("WEIGHT pushed");
    assert_eq!(index_of(&burst, &couple(SP_POW, 2, 1)), sp + 1);
    assert_eq!(index_of(&burst, &p(SP_PATK, 1)), sp + 2);
    assert_eq!(weight, sp + 3);

    // At B200 MaxAP 200 follows the derived values, then a non-zero AP.
    let mut world = World::new();
    let (burst, _) = world.login(spawn(
        DRAGON_KNIGHT,
        200,
        dk_status([0, 3, 0, 0, 0, 0], 0, 40),
    ));
    let traits: Vec<ServerEvent> = burst
        .iter()
        .filter(|e| is_trait_value(e))
        .cloned()
        .collect();
    assert_eq!(
        traits,
        vec![
            couple(SP_POW, 0, 1),
            couple(SP_POW + 1, 3, 0),
            p(SP_RES, 8),
            p(SP_MAXAP, 200),
            p(SP_AP, 40),
        ]
    );
}

/// ENG-TRAIT-07: the block after `0x00bd` carries the real POW..CRT, PATK..CRATE,
/// `SP_TRAITPOINT`, `SP_AP` (even at 0), `SP_MAXAP` and the next-point costs (captured
/// 1451-1471).
#[test]
fn eng_trait_07_initial_status_block_has_real_values() {
    let mut world = World::new();
    let (_, init) = world.login(spawn(
        DRAGON_KNIGHT,
        99,
        dk_status([2, 0, 0, 0, 0, 0], 5, 0),
    ));
    let aspd = init
        .iter()
        .position(|e| {
            matches!(
                e,
                ServerEvent::ParameterChanged {
                    var_id: SP_ASPD,
                    ..
                }
            )
        })
        .expect("the block's ASPD");
    let mut expected = vec![couple(SP_POW, 2, 1)];
    expected.extend((1..6).map(|i| couple(SP_POW + i, 0, 0)));
    expected.extend([
        p(SP_PATK, 1),
        p(226, 0),
        p(227, 0),
        p(228, 0),
        p(229, 0),
        p(230, 0),
        p(SP_TRAITPOINT, 5),
        p(SP_AP, 0),
        p(SP_MAXAP, 0),
    ]);
    expected.extend((0..6).map(|i| cost(SP_UPOW + i, 1)));
    assert_eq!(init[aspd + 1..aspd + 1 + expected.len()], expected[..]);

    // A non-trait job: every trait cost is 0.
    let mut world = World::new();
    let (_, init) = world.login(spawn(7, 99, CharacterStatus::default()));
    let costs: Vec<u8> = init
        .iter()
        .filter_map(|e| match e {
            ServerEvent::StatusCost { var_id, value } if *var_id >= SP_UPOW => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(costs, vec![0; 6]);
}

/// ENG-TRAIT-07: a map change's actor init sends the real trait `0x0141` ×6.
#[test]
fn eng_trait_07_map_change_sends_real_trait_couples() {
    let mut world = World::new();
    world
        .engine
        .enter_by_warp(
            spawn(DRAGON_KNIGHT, 99, dk_status([2, 0, 0, 0, 4, 0], 0, 0)),
            (60, 60),
            0,
            0,
        )
        .expect("arrives");
    let init = world.tick(&[(A, ClientIntent::NotifyActorInit)]);
    let traits: Vec<ServerEvent> = init
        .iter()
        .filter(|e| matches!(e, ServerEvent::CoupleStatus { var_id, .. } if *var_id >= SP_POW))
        .cloned()
        .collect();
    assert_eq!(
        traits,
        vec![
            couple(SP_POW, 2, 1),
            couple(SP_POW + 1, 0, 0),
            couple(SP_POW + 2, 0, 0),
            couple(SP_POW + 3, 0, 0),
            couple(SP_POW + 4, 4, 0),
            couple(SP_POW + 5, 0, 0),
        ]
    );
}

/// ENG-TRAIT-08: a stored AP above the new MaxAP (B200 → B199: 200 → 0) is clamped and pushed
/// with the HP/SP clamp pushes, before the diff; the diff then ends with `MAXAP` and `AP`.
#[test]
fn eng_trait_08_ap_clamp_after_a_level_drop() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 0, 150)));
    assert_eq!(world.status().ap, 150, "within MaxAP 200 at B200");
    let out = world.command("blvl -1");
    let clamp = index_of(&out, &p(SP_AP, 0));
    let max_ap = index_of(&out, &p(SP_MAXAP, 0));
    assert!(
        clamp < max_ap,
        "the clamp push comes before the diff: {out:?}"
    );
    assert_eq!(
        out.iter().filter(|e| **e == p(SP_AP, 0)).count(),
        2,
        "the clamp and the diff's AP: {out:?}"
    );
    assert_eq!(out[max_ap + 1], p(SP_AP, 0), "AP right after MAXAP");
    // MaxHP and MaxSP drop with the level too: the clamps go HP, SP, then AP.
    let var_at = |i: usize| match &out[i] {
        ServerEvent::ParameterChanged { var_id, .. } => *var_id,
        other => panic!("not a 0x00b0: {other:?}"),
    };
    assert_eq!((var_at(clamp - 2), var_at(clamp - 1)), (SP_HP, SP_SP));
    assert_eq!(world.status().ap, 0);
}

/// ENG-TRAIT-08: at spawn the clamp is silent: no `SP_AP` with the old value, and the stored AP
/// is the clamped one.
#[test]
fn eng_trait_08_spawn_clamps_ap_silently() {
    let mut world = World::new();
    let (burst, init) = world.login(spawn(DRAGON_KNIGHT, 199, dk_status([0; 6], 0, 150)));
    assert_eq!(world.status().ap, 0);
    assert!(
        !burst
            .iter()
            .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_AP, .. })),
        "AP 0 is not in the pre-ready push"
    );
    assert!(init.contains(&p(SP_AP, 0)), "the status block sends AP 0");
    assert!(!init.contains(&p(SP_AP, 150)));
}
