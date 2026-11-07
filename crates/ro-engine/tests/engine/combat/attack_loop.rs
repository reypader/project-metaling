//! The auto-attack loop: swings, range, stopping, step attacks and ammo (ENG-CMB-01..06).

use super::*;

/// ENG-CMB-01: action 7 on an adjacent Poring: `0x0088` ×2 then `0x08c8` to the attacker and the
/// observer; the HP lands at swing + amotion; the next swing at + adelay; no drift.
#[test]
fn eng_cmb_01_continuous_attack_on_adjacent_mob() {
    let a_at = (51, 50);
    let mut engine = world(
        vec![(A, a_at, Vec::new()), (B, (53, 53), Vec::new())],
        Vec::new(),
    );
    let events = attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000);
    let to_a = seen_by(&events, A);
    assert_eq!(to_a.len(), 3, "{to_a:?}");
    assert_eq!(to_a[..2], [stopped(A, a_at), stopped(A, a_at)]);
    let ServerEvent::Damage {
        source_id,
        target_id,
        tick: swing_tick,
        src_speed: amotion,
        dmg_speed,
        damage,
        div,
        damage_type,
        damage2,
    } = to_a[2].clone()
    else {
        panic!("third packet is the swing: {to_a:?}");
    };
    assert_eq!(
        (
            source_id,
            target_id,
            swing_tick,
            dmg_speed,
            div,
            damage_type,
            damage2
        ),
        (gid(A), mob_gid(&engine), 1000, MOB_DMOTION, 1, 0, 0)
    );
    assert!(damage > 0);
    assert_eq!(seen_by(&events, B), to_a, "the observer sees the same");

    // The HP changes at swing + amotion, not before.
    let amotion = u64::try_from(amotion).expect("positive amotion");
    let full = 1_000_000;
    assert_eq!(mob_hp(&engine), full);
    tick(&mut engine, 1000 + amotion - 1);
    assert_eq!(mob_hp(&engine), full);
    let landed = tick(&mut engine, 1000 + amotion);
    let hp = full - u32::try_from(damage).expect("positive");
    assert_eq!(mob_hp(&engine), hp);
    // The HP bar goes to the damage log only (captured).
    assert_eq!(
        seen_by(&landed, A),
        vec![ServerEvent::MobHpBar {
            gid: mob_gid(&engine),
            hp,
            max_hp: full
        }]
    );
    assert!(seen_by(&landed, B).is_empty(), "{landed:?}");

    // Ten swings at a 37 ms tick (it doesn't divide adelay = 2 × amotion, so most swings fire on a
    // late tick): each swing's tick is the previous due time + adelay, never the late tick.
    let adelay = 2 * amotion;
    let seen = run(&mut engine, A, 1000 + amotion, 1000 + 9 * adelay + 36, 37);
    let ticks: Vec<u32> = swings(&seen).iter().map(|s| s.0).collect();
    let expected: Vec<u32> = (1..=9).map(|k| (1000 + k * adelay) as u32).collect();
    assert_eq!(ticks, expected);
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, ServerEvent::UnitStopped { .. })),
        "loop swings are bare"
    );
}

/// ENG-CMB-02: action 0 swings once. A re-request before `canact` is dropped silently and ends
/// the loop. A request after `canact` but before `attackable` swings at `attackable`, bare.
#[test]
fn eng_cmb_02_single_swing_canact_and_deferred() {
    let mut engine = solo((51, 50));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A);
    let amotion = u64::try_from(swings(&events)[0].3).expect("amotion");
    let adelay = 2 * amotion;
    assert!(swings(&run(&mut engine, A, 1000, 1000 + 3 * adelay, 20)).is_empty());

    // Continuous, then a re-request at +100 ms (inside amotion + 100): nothing, loop over.
    let start = 10_000;
    attack(&mut engine, A, ACT_ATTACK_REPEAT, start);
    assert!(seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, start + 100), A).is_empty());
    assert!(swings(&run(&mut engine, A, start + 100, start + 3 * adelay, 20)).is_empty());

    // Single swing, then a request between canact (amotion + 100) and attackable (adelay).
    let start = 20_000;
    attack(&mut engine, A, ACT_ATTACK, start);
    let between = start + amotion + 120;
    // (The first swing's hit lands in this tick: only its HP bar.)
    assert!(without_bars(seen_by(
        &attack(&mut engine, A, ACT_ATTACK_REPEAT, between),
        A
    ))
    .is_empty());
    let seen = run(&mut engine, A, between, start + adelay, 10);
    assert_eq!(swings(&seen).len(), 1);
    assert_eq!(swings(&seen)[0].0, (start + adelay) as u32);
    assert!(!seen
        .iter()
        .any(|e| matches!(e, ServerEvent::UnitStopped { .. })));
    // It was a continuous request: the loop goes on.
    let seen = run(&mut engine, A, start + adelay, start + 2 * adelay, 10);
    assert_eq!(swings(&seen).len(), 1);
}

/// ENG-CMB-03: out of range gets one `0x0088` then `0x0139`, and the loop ends. Range 1 hits at
/// 2 cells straight and misses at (2,1). A wall between a bow and its target ends the attack
/// after one `0x0088`: no `0x0139`, no swing, no arrow spent.
#[test]
fn eng_cmb_03_range_and_line_of_sight() {
    let a_at = (53, 51);
    let mut engine = solo(a_at);
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(
        events,
        [
            stopped(A, a_at),
            ServerEvent::AttackOutOfRange {
                target_id: mob_gid(&engine),
                target_x: MOB.0,
                target_y: MOB.1,
                x: a_at.0,
                y: a_at.1,
                range: 1,
            }
        ]
    );
    assert!(run(&mut engine, A, 1000, 5000, 20).is_empty());

    let mut engine = solo((52, 50));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A);
    assert_eq!(swings(&events).len(), 1, "2 cells straight is in range 1");

    let mut engine = solo((52, 51));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A);
    assert!(matches!(
        events.as_slice(),
        [
            ServerEvent::UnitStopped { .. },
            ServerEvent::AttackOutOfRange { .. }
        ]
    ));

    // Out of range and behind a wall: the OFFICIAL_WALKPATH line check runs before the range
    // test, so the drop is silent (no `0x0139`).
    let a_at = (45, 50);
    let mut engine = solo(a_at);
    assert!(engine.set_cell(47, 50, CellType::Wall));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(events, [stopped(A, a_at)]);

    let archer = vec![worn(BOW, EQP_ARMS, 1), worn(ARROW, EQP_AMMO, 10)];
    let a_at = (45, 50);
    let mut engine = world(vec![(A, a_at, archer)], Vec::new());
    assert!(engine.set_cell(47, 50, CellType::Wall));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(events, [stopped(A, a_at)]);
    assert!(run(&mut engine, A, 1000, 5000, 20).is_empty());
    assert_eq!(
        engine.weight(char_id(A)).expect("weight").0,
        10 + 10,
        "no arrow spent"
    );
}

/// ENG-CMB-04: a move request, `0x0118`, sit, a warp and logout each stop the loop. A hit
/// already swung still lands, except after logout or a warp beyond its distance snapshot.
#[test]
fn eng_cmb_04_stops_and_pending_hits() {
    type Stop = fn(&mut MapInstance, u64);
    let cases: [(&str, Stop, bool); 5] = [
        (
            "move",
            |engine, now| {
                send(
                    engine,
                    A,
                    ClientIntent::RequestMove {
                        dest_x: 51,
                        dest_y: 47,
                    },
                    now,
                );
            },
            true,
        ),
        (
            "cancel",
            |engine, now| {
                send(engine, A, ClientIntent::CancelAttack, now);
            },
            true,
        ),
        (
            "sit",
            |engine, now| {
                send(
                    engine,
                    A,
                    ClientIntent::RequestAction {
                        target_id: 0,
                        action_type: ACT_SIT,
                    },
                    now,
                );
            },
            true,
        ),
        (
            "warp",
            |engine, now| {
                // Two cells onto a same-map warp to (150, 150): it fires at now + 300.
                send(
                    engine,
                    A,
                    ClientIntent::RequestMove {
                        dest_x: 51,
                        dest_y: 52,
                    },
                    now,
                );
            },
            false,
        ),
        (
            "logout",
            |engine, _| {
                engine.remove_character(char_id(A)).expect("remove");
            },
            false,
        ),
    ];
    for (name, stop, lands) in cases {
        let warp = NpcSpawn {
            name: "w".into(),
            unique_name: "w".into(),
            x: 51,
            y: 52,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state: NpcState::Normal,
            trigger: Some((0, 0)),
            duplicate_of: None,
            kind: NpcKind::Warp(WarpDest {
                map: "prt_fild08".into(),
                x: 150,
                y: 150,
                trigger_while_hidden: false,
            }),
            source: "test:warp".into(),
        };
        // B watches the swings so they are visible after A is gone.
        let mut engine = world(
            vec![(A, (51, 50), Vec::new()), (B, (53, 53), Vec::new())],
            vec![warp],
        );
        let first = swings(&seen_by(
            &attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000),
            A,
        ));
        let (amotion, damage) = (u64::try_from(first[0].3).expect("amotion"), first[0].1);
        stop(&mut engine, 1010);
        let seen = run(&mut engine, B, 1010, 1000 + 4 * amotion, 10);
        assert!(swings(&seen).is_empty(), "{name}: the loop stopped");
        let expected = if lands {
            1_000_000 - u32::try_from(damage).expect("positive")
        } else {
            1_000_000
        };
        assert_eq!(mob_hp(&engine), expected, "{name}: pending hit");
    }
}

/// ENG-CMB-05: a request while walking out of `range - 1` attacks half a step after the next
/// step ends, with range + 1: a mob at (2,1) from where the walk ends is hit.
#[test]
fn eng_cmb_05_step_action() {
    let mut engine = solo((40, 49));
    // Straight east, 8 cells at 150 ms each: steps end at 1150, 1300, ... 2200 at (48, 49).
    send(
        &mut engine,
        A,
        ClientIntent::RequestMove {
            dest_x: 48,
            dest_y: 49,
        },
        1000,
    );
    // Mid-walk at (43, 49), far from the mob: stored, nothing sent.
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1500), A);
    assert!(events.is_empty(), "{events:?}");
    let seen = run(&mut engine, A, 1500, 2274, 1);
    assert!(
        swings(&seen).is_empty(),
        "nothing before half a step after the last step"
    );
    let events = seen_by(&tick(&mut engine, 2275), A);
    // The walk ended at 2200 on (48, 49): (2, 1) from the mob, in range 1 + 1.
    assert_eq!(events[..2], [stopped(A, (48, 49)), stopped(A, (48, 49))]);
    assert_eq!(swings(&events).len(), 1);
    assert_eq!(swings(&events)[0].0, 2275);

    // Still walking when the target comes within range - 1: the walk stops with a third
    // `0x0088`. Walk past the mob along y = 49 from (44, 49) to (56, 49).
    let mut engine = solo((44, 49));
    send(
        &mut engine,
        A,
        ClientIntent::RequestMove {
            dest_x: 56,
            dest_y: 49,
        },
        1000,
    );
    attack(&mut engine, A, ACT_ATTACK_REPEAT, 1100);
    // (49, 49) is diagonal to the mob (range 0 fails); (50, 49) is straight below it at 1900.
    let seen = run(&mut engine, A, 1100, 1974, 1);
    assert!(swings(&seen).is_empty());
    let events = seen_by(&tick(&mut engine, 1975), A);
    assert_eq!(
        events[..3],
        [
            stopped(A, (50, 49)),
            stopped(A, (50, 49)),
            stopped(A, (50, 49))
        ]
    );
    assert_eq!(swings(&events)[0].0, 1975);
    assert!(!engine.is_walking(char_id(A)));
}

/// ENG-CMB-06: a bow with 2 arrows spends them in the captured order; the second takes the
/// stack off first; the third swing gets `0x013b` 0 and stops. A gun with no bullets gets
/// `0x0110` cause 84.
#[test]
fn eng_cmb_06_ammo() {
    let a_at = (46, 50);
    let archer = vec![worn(BOW, EQP_ARMS, 1), worn(ARROW, EQP_AMMO, 2)];
    let mut engine = world(
        vec![(A, a_at, archer), (B, (53, 53), Vec::new())],
        Vec::new(),
    );
    let all = attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000);
    let to_a = seen_by(&all, A);
    let arrows = 3; // wire index of slot 1
    assert_eq!(
        to_a[..4],
        [
            stopped(A, a_at),
            stopped(A, a_at),
            ServerEvent::ItemDeleted {
                index: arrows,
                amount: 1,
                reason: 1,
            },
            ServerEvent::ParameterChanged {
                var_id: SP_WEIGHT,
                value: 10 + 1,
            },
        ]
    );
    assert!(matches!(to_a[4], ServerEvent::Damage { .. }));
    assert_eq!(to_a.len(), 5, "{to_a:?}");
    let to_b = seen_by(&all, B);
    assert_eq!(to_b.len(), 3, "the observer sees no ammo packets: {to_b:?}");

    let amotion = u64::try_from(swings(&to_a)[0].3).expect("amotion");
    let adelay = 2 * amotion;
    // Each tick below also lands the previous swing's hit (its HP bar comes first).
    let second = without_bars(seen_by(&tick(&mut engine, 1000 + adelay), A));
    assert_eq!(
        second[..3],
        [
            ServerEvent::UnequipAck {
                index: arrows,
                location: EQP_AMMO,
                failed: false,
            },
            ServerEvent::ItemDeleted {
                index: arrows,
                amount: 1,
                reason: 1,
            },
            ServerEvent::ParameterChanged {
                var_id: SP_WEIGHT,
                value: 10,
            },
        ]
    );
    assert_eq!(
        swings(&second).len(),
        1,
        "the last arrow still fires: {second:?}"
    );

    let third = without_bars(seen_by(&tick(&mut engine, 1000 + 2 * adelay), A));
    assert_eq!(third, [ServerEvent::ActionFailure { failure_type: 0 }]);
    assert!(run(&mut engine, A, 1000 + 2 * adelay, 1000 + 5 * adelay, 20).is_empty());

    // A fresh click with no ammo: `0x0088` ×2 then `0x013b` 0.
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 20_000), A);
    assert_eq!(
        events,
        [
            stopped(A, a_at),
            stopped(A, a_at),
            ServerEvent::ActionFailure { failure_type: 0 }
        ]
    );

    let gunner = vec![worn(REVOLVER, EQP_HAND_R, 1)];
    let mut engine = world(vec![(A, a_at, gunner)], Vec::new());
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(
        events,
        [
            stopped(A, a_at),
            stopped(A, a_at),
            ServerEvent::SkillFail {
                skill_id: 0,
                cause: 84,
            }
        ]
    );
}

/// ENG-CMB-05: a move and an attack in one tick run in order (captured chase: `0x035f` to the
/// cell next to the target, then `0x0437` 40 ms later; both can land in one tick). The walk
/// starts, the request becomes a step action, and the swing comes on arrival.
#[test]
fn eng_cmb_05_move_then_attack_in_one_tick() {
    // (2, 1) from the mob: out of range 1 while standing.
    let mut engine = solo((52, 51));
    engine.push_intent(intent(
        A,
        ClientIntent::RequestMove {
            dest_x: 51,
            dest_y: 51,
        },
    ));
    let target_id = mob_gid(&engine);
    engine.push_intent(intent(
        A,
        ClientIntent::RequestAction {
            target_id,
            action_type: ACT_ATTACK_REPEAT,
        },
    ));
    let events = seen_by(&tick(&mut engine, 1000), A);
    assert!(
        events
            .iter()
            .all(|e| matches!(e, ServerEvent::PlayerMoved { .. })),
        "only the walk starts: {events:?}"
    );
    // One straight step ends at 1150; the step action fires half a step later.
    let seen = run(&mut engine, A, 1000, 1224, 1);
    assert!(swings(&seen).is_empty(), "{seen:?}");
    let events = seen_by(&tick(&mut engine, 1225), A);
    assert_eq!(events[..2], [stopped(A, (51, 51)), stopped(A, (51, 51))]);
    assert_eq!(swings(&events).len(), 1);
}

/// ENG-CMB-04: a successful pickup stops the attack silently (`pc_takeitem` →
/// `unit_stop_attack`).
#[test]
fn eng_cmb_04_pickup_stops_the_loop() {
    let knife = Some(InventoryItem::new(ItemId(KNIFE), 1));
    let mut engine = world(
        vec![(A, (51, 50), vec![knife]), (B, (53, 53), Vec::new())],
        Vec::new(),
    );
    let dropped = send(
        &mut engine,
        A,
        ClientIntent::DropItem {
            index: 2,
            amount: 1,
        },
        100,
    );
    let floor_id = dropped
        .iter()
        .find_map(|e| match &e.event {
            ServerEvent::FloorItemDropped { item } => Some(item.id),
            _ => None,
        })
        .expect("dropped");
    let first = swings(&seen_by(
        &attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000),
        A,
    ));
    let adelay = 2 * u64::try_from(first[0].3).expect("amotion");
    send(&mut engine, A, ClientIntent::PickUpItem { floor_id }, 1010);
    assert!(
        engine.weight(char_id(A)).expect("weight").0 > 0,
        "picked up"
    );
    assert!(swings(&run(&mut engine, B, 1010, 1000 + 3 * adelay, 10)).is_empty());
}
