//! Using potions (ENG-EFF-13..15).

use super::*;

/// ENG-EFF-13: a potion at reduced HP: `0x01c8` to the area with the remaining amount, then
/// `SP_WEIGHT` and `SP_HP` to self. The observer gets only the ack.
#[test]
fn eng_eff_13_potion_at_reduced_hp() {
    let mut w = potion_world(10, 9999, &[(HP_POTION, 5)]);
    let heal = potion_heal(40, w.battle_stat(A, SP_VIT));
    let now = w.now;
    let (to_a, to_b) = w.use_at(A, 2, now);
    let ack = item_ack(2, HP_POTION, A, 4, true);
    assert_eq!(
        to_a,
        [ack.clone(), param(SP_WEIGHT, 280), param(SP_HP, 10 + heal)]
    );
    assert_eq!(to_b, [ack]);
    assert_eq!(w.status(A).hp, 10 + heal as u32);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 4)));
}

/// ENG-EFF-13 (spec §2.7): walking, sitting and an auto-attack loop are not gates, and a potion
/// stops neither the walk (no `0x0088`) nor the swings.
#[test]
fn eng_eff_13_potion_keeps_walk_sit_and_attack() {
    let mut w = potion_world(10, 9999, &[(HP_POTION, 5)]);
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 51,
                dest_y: 58,
            },
        )],
    ));
    let (to_a, to_b) = w.use_at(A, 2, t + 50);
    assert_eq!(to_a.len(), 3, "ack, weight, HP only: {to_a:?}");
    assert!(matches!(to_a[0], ServerEvent::ItemUseAck { ok: true, .. }));
    assert_eq!(to_b, [item_ack(2, HP_POTION, A, 4, true)], "no stop");
    w.run_to(A, t + 3000);

    let t = w.now + 10;
    let sat = w.at(
        t,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: 0,
                action_type: 2,
            },
        )],
    );
    assert!(
        seen_by(&sat, B)
            .iter()
            .any(|e| matches!(e, ServerEvent::EntityAction { .. })),
        "A sat down"
    );
    let (to_a, _) = w.use_at(A, 2, t + 200);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 3, true), "sitting");
    drop(w.at(
        t + 400,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: 0,
                action_type: 3,
            },
        )],
    ));

    // Back next to the mob and swinging.
    let t = t + 500;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: NEXT_TO_MOB.0,
                dest_y: NEXT_TO_MOB.1,
            },
        )],
    ));
    w.run_to(A, t + 3000);
    let mob = w.mob_gid();
    let t = w.now + 10;
    let out = w.at(
        t,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 7,
            },
        )],
    );
    assert!(seen_by(&out, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::Damage { .. })));
    let (to_a, _) = w.use_at(A, 2, t + 10);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 2, true), "attacking");
    let later = w.run_to(A, t + 10 + 4 * w.amotion(A));
    assert!(
        later
            .iter()
            .any(|e| matches!(e, ServerEvent::Damage { .. })),
        "the loop goes on: {later:?}"
    );
}

/// ENG-EFF-14: at full HP the potion is still used, with only the ack and `SP_WEIGHT`; an SP
/// potion pushes `SP_SP`; the last potion of a stack (a percent heal here) empties the slot.
#[test]
fn eng_eff_14_full_hp_sp_and_last_potion() {
    let mut w = potion_world(
        9999,
        0,
        &[(HP_POTION, 5), (SP_POTION, 5), (PERCENT_POTION, 1)],
    );
    let mut weight = 5 * 70 + 5 * 150 + 10;
    let t = w.now;

    let (to_a, to_b) = w.use_at(A, 2, t);
    weight -= 70;
    let ack = item_ack(2, HP_POTION, A, 4, true);
    assert_eq!(to_a, [ack.clone(), param(SP_WEIGHT, weight)]);
    assert_eq!(to_b, [ack]);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 4)));

    let sp = potion_heal(20, w.battle_stat(A, SP_INT));
    let (to_a, _) = w.use_at(A, 3, t + 1000);
    weight -= 150;
    assert_eq!(
        to_a,
        [
            item_ack(3, SP_POTION, A, 4, true),
            param(SP_WEIGHT, weight),
            param(SP_SP, sp),
        ]
    );

    let _ = w.at(
        t + 1100,
        &[(
            A,
            ClientIntent::Chat {
                text: format!("{} : @heal -100000", name(A)).into_bytes(),
            },
        )],
    );
    let max_hp = w.status(A).max_hp as i32;
    let (to_a, to_b) = w.use_at(A, 4, t + 2000);
    weight -= 10;
    let ack = item_ack(4, PERCENT_POTION, A, 0, true);
    assert_eq!(
        to_a,
        [
            ack.clone(),
            param(SP_WEIGHT, weight),
            param(SP_HP, 1 + (max_hp * 10 / 100).max(1)),
        ]
    );
    assert_eq!(to_b, [ack]);
    assert_eq!(stack(&w, A, 2), None, "the last potion empties the slot");
}

/// ENG-EFF-15: refusals send the failure `0x01c8` (amount 0, result 0) to self only and consume
/// nothing: too soon after the last use, an item type that can't be used, no lifted `Use`, a
/// level, gender, job or class restriction. An empty slot or an index out of range is silent.
#[test]
fn eng_eff_15_potion_refused() {
    let mut w = potion_world(
        10,
        9999,
        &[
            (HP_POTION, 5),
            (NOT_USABLE, 1),
            (NO_USE, 1),
            (LEVEL_LOCKED, 1),
            (GENDER_LOCKED, 1),
            (JOB_LOCKED, 1),
            (CLASS_LOCKED, 1),
        ],
    );
    let t = w.now;
    let (to_a, _) = w.use_at(A, 2, t);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 4, true));
    // `item_use_interval` is 100 ms.
    let (to_a, to_b) = w.use_at(A, 2, t + 90);
    assert_eq!(to_a, [item_ack(2, HP_POTION, A, 0, false)], "too soon");
    assert_eq!(to_b, []);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 4)));
    let (to_a, _) = w.use_at(A, 2, t + 100);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 3, true));

    for (slot, item) in [
        (1, NOT_USABLE),
        (2, NO_USE),
        (3, LEVEL_LOCKED),
        (4, GENDER_LOCKED),
        (5, JOB_LOCKED),
        (6, CLASS_LOCKED),
    ] {
        let now = w.now + 1000;
        let index = slot as u16 + 2;
        let (to_a, to_b) = w.use_at(A, index, now);
        assert_eq!(to_a, [item_ack(index, item, A, 0, false)], "item {item}");
        assert_eq!(to_b, []);
        assert_eq!(stack(&w, A, slot), Some((item, 1)));
    }

    for index in [0, 1, 9, u16::MAX] {
        let now = w.now + 1000;
        assert_eq!(w.use_at(A, index, now), (vec![], vec![]), "index {index}");
    }
}

/// ENG-EFF-15 (catalog `mechanics/inventory.md` §7.2): a loaded stack of amount 0 is silent like
/// an empty slot: no ack, no heal, nothing removed (final review).
#[test]
fn eng_eff_15_zero_amount_stack_is_silent() {
    let mut w = potion_world(10, 9999, &[(HP_POTION, 0)]);
    let now = w.now;
    assert_eq!(w.use_at(A, 2, now), (vec![], vec![]));
    assert_eq!(w.status(A).hp, 10);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 0)));
}
