//! Taking gear off, ammo, and dropping worn items (ENG-EQ-07..10, 12).

use super::*;

/// ENG-EQ-07, take-off: success order, and the failure forms.
#[test]
fn eng_eq_07_take_off() {
    let mut engine = pair(vec![worn(KNIFE, EQP_HAND_R), item(HAT)]);
    for (index, reply) in [(0, off_failed(2)), (1, off_failed(2)), (200, off_failed(2))] {
        assert_eq!(seen_by(&take_off(&mut engine, index), A), vec![reply]);
    }
    assert_eq!(
        seen_by(&take_off(&mut engine, 3), A),
        vec![off_failed(3)],
        "not worn"
    );
    assert_eq!(
        seen_by(&take_off(&mut engine, 9), A),
        vec![off_failed(9)],
        "empty slot"
    );

    let out = take_off(&mut engine, 2);
    assert_eq!(
        seen_by(&out, A),
        vec![hands(0, 0), off(2, EQP_HAND_R), param(SP_ATK2, 0)]
    );
    assert_eq!(seen_by(&out, B), vec![hands(0, 0)]);
}

/// ENG-EQ-08, unequip all: in `EQI` order, one push per item, then `0x0bae`. A two-hander comes
/// off once; the request mask filters.
#[test]
fn eng_eq_08_unequip_all() {
    let mut engine = pair(vec![
        worn(KNIFE, EQP_HAND_R),
        worn(GUARD, EQP_HAND_L),
        worn(HAT, EQP_HEAD_TOP),
        worn(BRAVERY_BAG, EQP_GARMENT),
        worn(C_SAHKKAT, EQP_COSTUME_HEAD_TOP),
    ]);
    let out = unequip_all(&mut engine, UNEQUIP_ALL_WINDOW);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(5, EQP_GARMENT),
            look(LOOK_ROBE, 0, 0),
            off(4, EQP_HEAD_TOP),
            hands(KNIFE, 0),
            off(3, EQP_HAND_L),
            hands(0, 0),
            off(2, EQP_HAND_R),
            ServerEvent::UnequipAllAck { failed: false },
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
            param(SP_ATK2, 0),
        ],
        "the costume stays, and still covers the Hat's look; one diff for all four items"
    );
    let out = unequip_all(&mut engine, UNEQUIP_ALL_COSTUME);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(6, EQP_COSTUME_HEAD_TOP),
            look(LOOK_HEAD_TOP, 0, 0),
            ServerEvent::UnequipAllAck { failed: false },
        ]
    );

    let mut engine = pair(vec![worn(BOW, EQP_ARMS)]);
    let out = unequip_all(&mut engine, UNEQUIP_ALL_WINDOW);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            ServerEvent::UnequipAllAck { failed: false },
            param(SP_ATK2, 0),
            range(1),
        ]
    );
}

/// ENG-EQ-09: arrows on a bow: `0x0983` to the area, `0x013c` and `0x013b` 3 to self, no
/// `0x0999`, no push. The client asks with position 0 or `0x8000`.
#[test]
fn eng_eq_09_arrows_on_a_bow() {
    for position in [0, EQP_AMMO] {
        let mut engine = pair(vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, 0)]);
        let out = equip(&mut engine, 3, position);
        assert_eq!(
            seen_by(&out, A),
            vec![
                arrow_icon(),
                ServerEvent::AmmoEquipped { index: 3 },
                ServerEvent::ActionFailure { failure_type: 3 },
            ],
            "position {position:#x}"
        );
        assert_eq!(seen_by(&out, B), vec![arrow_icon()]);
    }
}

/// ENG-EQ-09: arrows with no bow, or with a katar (capture 4): `0x0291` 1691, then result 2.
#[test]
fn eng_eq_09_arrows_need_a_bow() {
    for weapon in [
        vec![],
        vec![worn(KNIFE, EQP_HAND_R)],
        vec![worn(JUR, EQP_ARMS)],
    ] {
        let mut inventory = vec![stack(ARROW, 100, 0)];
        inventory.extend(weapon);
        let mut engine = pair(inventory);
        assert_eq!(
            seen_by(&equip(&mut engine, 2, EQP_AMMO), A),
            vec![ServerEvent::ClientMessage { msg_id: 1691 }, refused(2, 2)]
        );
    }
}

/// ENG-EQ-09: taking the bow off takes the arrows off too, before the one push.
#[test]
fn eng_eq_09_bow_take_off_cascades() {
    let mut engine = pair(vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, EQP_AMMO)]);
    assert_eq!(
        seen_by(&take_off(&mut engine, 2), A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            off(3, EQP_AMMO),
            param(SP_ATK2, 0),
            range(1),
        ]
    );
}

/// ENG-EQ-09: a Knife over the bow: the bow comes off without a cascade, and the arrows come
/// off after the Knife's look because they don't pair with it. A whip keeps them.
#[test]
fn eng_eq_09_weapon_change_cascades_when_the_ammo_no_longer_fits() {
    let mut engine = pair(vec![
        worn(BOW, EQP_ARMS),
        stack(ARROW, 100, EQP_AMMO),
        item(KNIFE),
        item(WHIP),
    ]);
    assert_eq!(
        seen_by(&equip(&mut engine, 4, EQP_HAND_R), A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            ack(4, EQP_HAND_R, 0),
            hands(KNIFE, 0),
            off(3, EQP_AMMO),
            param(SP_ATK2, 17),
            range(1),
        ]
    );

    let mut engine = pair(vec![
        worn(BOW, EQP_ARMS),
        stack(ARROW, 100, EQP_AMMO),
        item(WHIP),
    ]);
    let out = seen_by(&equip(&mut engine, 4, EQP_HAND_R), A);
    assert!(
        !out.contains(&off(3, EQP_AMMO)),
        "a whip takes arrows: {out:?}"
    );
}

/// ENG-EQ-08: unequip-all with a bow and arrows removes the arrows once, by the cascade.
#[test]
fn eng_eq_08_unequip_all_removes_arrows_once() {
    let mut engine = pair(vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, EQP_AMMO)]);
    assert_eq!(
        seen_by(&unequip_all(&mut engine, UNEQUIP_ALL_WINDOW), A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            off(3, EQP_AMMO),
            ServerEvent::UnequipAllAck { failed: false },
            param(SP_ATK2, 0),
            range(1),
        ]
    );
}

/// ENG-EQ-12: worn arrows add `0x0983` + `0x013c` between the two inventory lists on every
/// actor init, to self only.
#[test]
fn eng_eq_12_map_entry_resends_the_ammo() {
    let mut engine = instance();
    engine
        .spawn_character(
            spawn_data(A, vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, EQP_AMMO)]),
            0,
        )
        .expect("spawn");
    enter(&mut engine, spawn_data(B, vec![]), 0);
    for now in [10, 20] {
        let out = actor_init(&mut engine, A, now);
        let lists: Vec<_> = seen_by(&out, A)
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    ServerEvent::InventoryNormalItems { .. }
                        | ServerEvent::InventoryEquipItems { .. }
                        | ServerEvent::StatusIconStarted { .. }
                        | ServerEvent::AmmoEquipped { .. }
                )
            })
            .map(|e| match e {
                ServerEvent::InventoryNormalItems { .. } => "normal",
                ServerEvent::InventoryEquipItems { .. } => "equip",
                ServerEvent::StatusIconStarted { .. } => "icon",
                _ => "ammo",
            })
            .collect();
        assert_eq!(
            lists,
            vec!["normal", "icon", "ammo", "equip"],
            "actor init at {now}"
        );
        assert!(
            !seen_by(&out, B).iter().any(|e| matches!(
                e,
                ServerEvent::StatusIconStarted { .. } | ServerEvent::AmmoEquipped { .. }
            )),
            "the observer gets neither"
        );
    }
}

/// ENG-EQ-10 (unit level; the client won't send it): dropping a worn weapon unequips it after
/// the floor item appears and before `SP_WEIGHT` and `0x00af`. Dropping part of worn arrows
/// keeps them worn.
#[test]
fn eng_eq_10_dropping_worn_items() {
    let mut engine = pair(vec![worn(KNIFE, EQP_HAND_R), stack(ARROW, 100, EQP_AMMO)]);
    let drop = |engine: &mut MapInstance, index, amount| {
        send(engine, A, ClientIntent::DropItem { index, amount }, 100)
    };
    let out = drop(&mut engine, 2, 1);
    let seen: Vec<_> = seen_by(&out, A)
        .into_iter()
        .map(|e| match e {
            ServerEvent::FloorItemDropped { .. } => param(0, -1),
            other => other,
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            param(0, -1),
            hands(0, 0),
            off(2, EQP_HAND_R),
            param(24, 10_000),
            ServerEvent::ItemThrown {
                index: 2,
                amount: 1
            },
            param(SP_ATK2, 0),
        ],
        "0x0add, then the unequip, then SP_WEIGHT and 0x00af; the diff at the end of the tick"
    );
    assert!(
        seen_by(&out, B).contains(&hands(0, 0)),
        "the observer sees the look"
    );

    let out = drop(&mut engine, 3, 5);
    assert!(
        !seen_by(&out, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::UnequipAck { .. })),
        "a partial drop keeps the arrows worn"
    );
    let out = actor_init(&mut engine, A, 200);
    assert!(seen_by(&out, A).contains(&ServerEvent::AmmoEquipped { index: 3 }));
}
