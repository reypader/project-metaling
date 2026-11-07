//! Equipping: slots, swaps, refusals and looks (ENG-EQ-01, 03..06, 11, 13).

use super::*;

/// ENG-EQ-11 (status part): spawning with worn gear builds the gear status from the masks:
/// a bow's range, and the hard DEF of a Guard and a Hat each counted once.
#[test]
fn eng_eq_11_spawn_builds_gear_status_from_the_masks() {
    let mut engine = instance();
    let burst = engine
        .spawn_character(
            spawn_data(A, vec![worn(BOW, EQP_ARMS), worn(HAT, EQP_HEAD_TOP)]),
            0,
        )
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_ATK2), Some(15), "the bow counts once");
    assert_eq!(param_of(&burst, SP_DEF2), Some(2));
    assert_eq!(param_of(&burst, SP_DEF1), Some(SOFT_DEF));
    assert_eq!(attack_ranges(&burst), vec![5]);

    let init = self_events(actor_init(&mut engine, A, 10), A);
    assert_eq!(attack_ranges(&init), vec![5], "the status block's 0x013a");

    let mut engine = instance();
    let burst = engine
        .spawn_character(
            spawn_data(
                A,
                vec![
                    worn(KNIFE, EQP_HAND_R),
                    worn(GUARD, EQP_HAND_L),
                    worn(HAT, EQP_HEAD_TOP),
                ],
            ),
            0,
        )
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_DEF2), Some(22));
    let window = window(&self_events(actor_init(&mut engine, A, 10), A));
    assert_eq!((window.atk_bonus, window.hard_def), (17, 22));
}

/// ENG-EQ-11 (looks): an observer's entry packet carries the spawned gear, and the cache follows
/// later equips.
#[test]
fn eng_eq_11_entry_packets_carry_the_looks() {
    let mut engine = instance();
    enter(
        &mut engine,
        spawn_data(
            A,
            vec![
                worn(KNIFE, EQP_HAND_R),
                worn(GUARD, EQP_HAND_L),
                worn(HAT, EQP_HEAD_TOP),
                worn(BRAVERY_BAG, EQP_GARMENT),
                item(GOGGLES),
            ],
        ),
        0,
    );
    let expected = UnitLooks {
        weapon: KNIFE,
        shield: GUARD,
        head_bottom: 0,
        head_top: 16,
        head_mid: 0,
        robe: 2,
    };
    assert_eq!(looks_seen_on_entry(&mut engine), expected);

    engine.remove_character(char_id(B)).expect("B leaves");
    equip(&mut engine, 6, EQP_HEAD_TOP | EQP_HEAD_MID);
    assert_eq!(
        looks_seen_on_entry(&mut engine),
        UnitLooks {
            head_top: 1,
            ..expected
        },
        "Goggles replaced the Hat"
    );
}

/// ENG-EQ-03, captured swap: Knife A worn, equip Knife B. Swapping back mirrors it.
#[test]
fn eng_eq_03_captured_swap() {
    let mut engine = pair(vec![worn(KNIFE, EQP_HAND_R), item(KNIFE)]);
    let out = equip(&mut engine, 3, EQP_HAND_R);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            off(2, EQP_HAND_R),
            ack(3, EQP_HAND_R, 0),
            hands(KNIFE, 0),
        ],
        "one net diff at the end of the tick: a Knife for a Knife changes nothing"
    );
    assert_eq!(seen_by(&out, B), vec![hands(0, 0), hands(KNIFE, 0)]);

    let out = equip(&mut engine, 2, EQP_HAND_R);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            off(3, EQP_HAND_R),
            ack(2, EQP_HAND_R, 0),
            hands(KNIFE, 0),
        ]
    );
}

/// ENG-EQ-04: a bow over Knife + Guard, per slot, then taking the bow off.
#[test]
fn eng_eq_04_two_hander_over_knife_and_guard() {
    let mut engine = pair(vec![
        worn(KNIFE, EQP_HAND_R),
        worn(GUARD, EQP_HAND_L),
        item(BOW),
    ]);
    let out = equip(&mut engine, 4, EQP_ARMS);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(KNIFE, 0),
            off(3, EQP_HAND_L),
            hands(0, BOW),
            off(2, EQP_HAND_R),
            ack(4, EQP_ARMS, 0),
            hands(BOW, 0),
            hands(BOW, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
            param(SP_ATK2, 15),
            range(5),
        ],
        "acks and looks as captured, then one net diff (ENG-STAT-02)"
    );
    assert_eq!(
        seen_by(&out, B),
        vec![hands(KNIFE, 0), hands(0, BOW), hands(BOW, 0), hands(BOW, 0)]
    );

    let out = take_off(&mut engine, 4);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(4, EQP_ARMS),
            param(SP_ATK2, 0),
            range(1),
        ],
        "two identical looks before the ack"
    );
}

/// ENG-EQ-05: headgear, garment, shoes.
#[test]
fn eng_eq_05_goggles_over_a_hat() {
    let mut engine = pair(vec![worn(HAT, EQP_HEAD_TOP), item(GOGGLES)]);
    let out = equip(&mut engine, 3, EQP_HEAD_TOP | EQP_HEAD_MID);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(2, EQP_HEAD_TOP),
            look(LOOK_HEAD_TOP, 0, 0),
            ack(3, EQP_HEAD_TOP | EQP_HEAD_MID, 1),
            look(LOOK_HEAD_TOP, 1, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 5),
        ],
        "only LOOK_HEAD_TOP; the net diff after the ack"
    );
    assert_eq!(
        seen_by(&out, B),
        vec![look(LOOK_HEAD_TOP, 0, 0), look(LOOK_HEAD_TOP, 1, 0)]
    );
}

#[test]
fn eng_eq_05_garment_and_shoes() {
    let mut engine = pair(vec![item(BRAVERY_BAG), item(SHOES)]);
    let out = equip(&mut engine, 2, EQP_GARMENT);
    assert_eq!(
        seen_by(&out, A),
        vec![
            ack(2, EQP_GARMENT, 2),
            look(LOOK_ROBE, 2, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 20),
        ]
    );
    let out = take_off(&mut engine, 2);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(2, EQP_GARMENT),
            look(LOOK_ROBE, 0, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
        ]
    );

    let out = equip(&mut engine, 3, EQP_SHOES);
    assert_eq!(
        seen_by(&out, A),
        vec![
            ack(3, EQP_SHOES, 0),
            look(LOOK_SHOES, SHOES, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 10),
        ],
        "LOOK_SHOES carries the item id"
    );
    let out = take_off(&mut engine, 3);
    assert_eq!(
        seen_by(&out, A),
        vec![
            look(LOOK_SHOES, 0, 0),
            off(3, EQP_SHOES),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
        ],
        "the shoes look comes before the ack"
    );
}

/// ENG-EQ-05, costumes (capture 4 lines 800–972): the costume top covers the normal top's
/// look; head looks go out only when the resolved view changes.
#[test]
fn eng_eq_05_costume_top_over_normal_tops() {
    let mut engine = pair(vec![
        worn(SAHKKAT, EQP_HEAD_TOP),
        item(C_SAHKKAT),
        item(CAP),
        worn(FLU_MASK, EQP_HEAD_LOW),
    ]);
    let costume = equip(&mut engine, 3, EQP_COSTUME_HEAD_TOP);
    assert_eq!(
        seen_by(&costume, A),
        vec![ack(3, EQP_COSTUME_HEAD_TOP, 67)],
        "same view as the Sahkkat underneath: no look"
    );

    let out = equip(&mut engine, 4, EQP_HELM);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(5, EQP_HEAD_LOW),
            look(LOOK_HEAD_BOTTOM, 0, 0),
            off(2, EQP_HEAD_TOP),
            ack(4, EQP_HELM, 326),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 4),
        ],
        "the costume covers the top: no LOOK_HEAD_TOP either way"
    );

    let out = take_off(&mut engine, 3);
    assert_eq!(
        seen_by(&out, A),
        vec![off(3, EQP_COSTUME_HEAD_TOP), look(LOOK_HEAD_TOP, 326, 0)],
        "removing the costume shows the normal top"
    );

    let out = equip(&mut engine, 2, EQP_HEAD_TOP);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(4, EQP_HELM),
            look(LOOK_HEAD_TOP, 0, 0),
            ack(2, EQP_HEAD_TOP, 67),
            look(LOOK_HEAD_TOP, 67, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 14),
        ],
        "a low + mid + top headgear sends only LOOK_HEAD_TOP when it comes off"
    );
}

/// ENG-EQ-05: a low + mid + top headgear over three items: each displaced item's look, then one
/// `LOOK_HEAD_TOP` (capture 4 lines 908–924).
#[test]
fn eng_eq_05_helm_over_three_headgears() {
    let mut engine = pair(vec![
        worn(FLU_MASK, EQP_HEAD_LOW),
        worn(ONE_EYED_GLASS, EQP_HEAD_MID),
        worn(HAT, EQP_HEAD_TOP),
        item(CAP),
    ]);
    let out = equip(&mut engine, 5, EQP_HELM);
    assert_eq!(
        seen_by(&out, B),
        vec![
            look(LOOK_HEAD_BOTTOM, 0, 0),
            look(LOOK_HEAD_MID, 0, 0),
            look(LOOK_HEAD_TOP, 0, 0),
            look(LOOK_HEAD_TOP, 326, 0),
        ]
    );
    let pushes: Vec<_> = seen_by(&out, A)
        .into_iter()
        .filter(|e| {
            matches!(
                e,
                ServerEvent::ParameterChanged {
                    var_id: SP_DEF2,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(
        pushes,
        vec![param(SP_DEF2, 4)],
        "one net diff: Hat 2 + Glass 0 + Mask 0 → Cap 4"
    );
}

/// ENG-EQ-06, failures that reach the client: not identified, broken, already worn, wrong
/// position → result 2; empty, out-of-range or unknown items → nothing.
#[test]
fn eng_eq_06_refusals() {
    let mut unidentified = InventoryItem::new(ItemId(KNIFE), 1);
    unidentified.identified = false;
    let mut broken = InventoryItem::new(ItemId(KNIFE), 1);
    broken.attribute = 1;
    let mut engine = pair(vec![
        Some(unidentified),
        Some(broken),
        worn(KNIFE, EQP_HAND_R),
        item(HAT),
        None,
        item(UNKNOWN),
    ]);
    for (index, position) in [
        (2, EQP_HAND_R),
        (3, EQP_HAND_R),
        (4, EQP_HAND_R),
        (5, EQP_HAND_R),
    ] {
        let out = equip(&mut engine, index, position);
        assert_eq!(seen_by(&out, A), vec![refused(index, 2)], "index {index}");
        assert!(seen_by(&out, B).is_empty());
    }
    for index in [0, 1, 6, 7, 200] {
        let out = equip(&mut engine, index, EQP_HAND_R);
        assert!(seen_by(&out, A).is_empty(), "index {index}: no ack");
    }
}

/// ENG-EQ-01 in the flow (capture 4 lines 493–550): accessories go right, then left, then
/// replace the right one.
#[test]
fn eng_eq_01_accessory_slots_in_the_flow() {
    let mut engine = pair(vec![item(CLIP), item(CLIP), item(CLIP)]);
    assert_eq!(
        seen_by(&equip(&mut engine, 2, EQP_ACC), A),
        vec![ack(2, EQP_ACC_R, 0)]
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 3, EQP_ACC), A),
        vec![ack(3, EQP_ACC_L, 0)]
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 4, EQP_ACC), A),
        vec![off(2, EQP_ACC_R), ack(4, EQP_ACC_R, 0)]
    );
}

/// ENG-EQ-13, dual wield (capture 3 lines 721–740, 835–852).
#[test]
fn eng_eq_13_dual_wield() {
    let mut engine = assassin(vec![
        worn(KNIFE, EQP_HAND_R),
        item(DAMASCUS),
        item(GLADIUS),
        item(JUR),
    ]);
    assert_eq!(
        seen_by(&equip(&mut engine, 3, EQP_ARMS), A),
        vec![
            ack(3, EQP_HAND_L, 0),
            hands(KNIFE, DAMASCUS),
            param(SP_ATK2, 135),
        ]
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 4, EQP_ARMS), A),
        vec![
            hands(0, DAMASCUS),
            off(2, EQP_HAND_R),
            ack(4, EQP_HAND_R, 0),
            hands(GLADIUS, DAMASCUS),
            param(SP_ATK2, 223),
        ],
        "both hands taken: the right one is replaced"
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 5, EQP_ARMS), A),
        vec![
            hands(GLADIUS, 0),
            off(3, EQP_HAND_L),
            hands(0, JUR),
            off(4, EQP_HAND_R),
            ack(5, EQP_ARMS, 0),
            hands(JUR, 0),
            hands(JUR, 0),
            param(SP_ATK2, 125),
            param(SP_CRITICAL, 2),
        ],
        "a katar doubles CRITICAL (ENG-STAT-03)"
    );
}
