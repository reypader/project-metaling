//! Cross-map transfers and entering by warp (ENG-WARP-03, 05, 06, 08).

use super::*;

#[test]
fn eng_warp_03_cross_map_transfer_out() {
    let mut engine = instance(
        grid(200, 200, &[]),
        vec![warp_to("w", (60, 50), (1, 1), "geffen", (100, 100))],
    );
    enter(&mut engine, B, (55, 52), 0);
    enter(&mut engine, A, (50, 50), 0);
    let mut events = walk(&mut engine, A, (66, 50), 0, 1300);
    assert!(
        drain(&mut engine).is_empty(),
        "nothing before the trigger step"
    );
    // An intent queued for A in the tick that warps it is dropped with the entity.
    engine.push_intent(intent(A, ClientIntent::RequestTime { client_tick: 0 }));
    events.extend(tick(&mut engine, 1350));
    for now in (1400..=2000).step_by(50) {
        events.extend(tick(&mut engine, now));
    }

    assert_eq!(vanished(&to(&events, B), account(A)), [0], "observer B");
    assert!(
        map_moves(&events, A).is_empty(),
        "the destination sends 0x0091"
    );
    assert!(
        !to(&events, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::TimeSynchronized { .. })),
        "queued intent dropped"
    );
    assert_eq!(engine.character_position(char_id(A)), None);

    let transfers = drain(&mut engine);
    assert_eq!(transfers.len(), 1);
    let t = &transfers[0];
    let from_pos = MapPosition {
        x: 59,
        y: 50,
        dir: 6,
    };
    assert_eq!(
        t.spawn,
        CharacterSpawn {
            pos: from_pos,
            ..spawn_data(A, 0, 0)
        },
        "identity, look, level and speed kept"
    );
    assert_eq!(&*t.dest_map, MAP2);
    assert_eq!(t.dest, (100, 100));
    assert_eq!(t.rewarp_count, 0);
    assert_eq!(t.from_pos, from_pos);
    assert!(drain(&mut engine).is_empty(), "second drain yields nothing");
}

#[test]
fn eng_warp_03_destination_stays_unresolved() {
    let mut engine = instance(
        grid(100, 100, &[]),
        vec![warp_to("w", (50, 50), (1, 1), "geffen.gat", (0, 0))],
    );
    enter(&mut engine, A, (50, 50), 0);
    let transfers = drain(&mut engine);
    assert_eq!(transfers.len(), 1);
    assert_eq!(
        &*transfers[0].dest_map, MAP2,
        "a .gat destination is kept as-is"
    );
    assert_eq!(transfers[0].dest, (0, 0));
}

#[test]
fn eng_warp_08_enter_by_warp_spawns_hidden_and_emits_nothing() {
    let mut engine = map2(vec![]);
    enter(&mut engine, B, (102, 100), 0);
    let pos = engine
        .enter_by_warp(spawn_data(A, 1, 1), (100, 100), 0, 100)
        .expect("enter");
    assert_eq!((pos.x, pos.y), (100, 100));
    assert_eq!(engine.character_position(char_id(A)), Some(pos));
    assert!(tick(&mut engine, 100).is_empty(), "no events to anyone");

    let events = walk(&mut engine, A, (105, 100), 150, 300);
    assert!(events.is_empty(), "moves before ActorInit are ignored");

    let events = actor_init(&mut engine, A, 350);
    assert!(to(&events, B)
        .iter()
        .any(|e| matches!(e, ServerEvent::UnitSpawned { unit } if unit.gid == account(A))));
    assert!(to(&events, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::UnitStanding { unit } if unit.gid == account(B))));
    assert!(to(&events, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::ParameterChanged { .. })));
    let events = walk(&mut engine, A, (105, 100), 400, 400);
    assert!(to(&events, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::PlayerMoved { .. })));
}

#[test]
fn eng_warp_08_duplicate_is_an_error_and_changes_nothing() {
    let mut engine = map2(vec![]);
    let first = engine
        .enter_by_warp(spawn_data(A, 0, 0), (100, 100), 0, 0)
        .expect("enter");
    assert_eq!(
        engine.enter_by_warp(spawn_data(A, 0, 0), (120, 120), 0, 0),
        Err(EngineError::CharacterAlreadyEntered(char_id(A)))
    );
    assert_eq!(engine.character_position(char_id(A)), Some(first));
}

#[test]
fn eng_warp_08_random_destination_is_resolved() {
    let mut engine = map2(vec![]);
    let pos = engine
        .enter_by_warp(spawn_data(A, 0, 0), (0, 0), 0, 0)
        .expect("enter");
    assert_ne!((pos.x, pos.y), (0, 0));
    assert!(pos.x < 199 && pos.y < 199);
}

#[test]
fn eng_warp_08_no_free_cell_uses_the_map_centre() {
    // 5x5: the random range 1..=3 is all inside the warp area.
    let mut engine = MapInstance::new(MAP2, grid(5, 5, &[]));
    engine.spawn_npcs(&MapNpcs {
        npcs: vec![warp_to("w", (2, 2), (1, 1), "prontera", (9, 9))],
    });
    let pos = engine
        .enter_by_warp(spawn_data(A, 0, 0), (0, 0), 0, 0)
        .expect("enter");
    assert_eq!((pos.x, pos.y), (2, 2));
}

#[test]
fn eng_warp_05_arrival_chain_after_enter_by_warp() {
    let mut engine = map2(vec![warp("w", (100, 100), (1, 1), (50, 50))]);
    engine
        .enter_by_warp(spawn_data(A, 0, 0), (100, 100), 0, 0)
        .expect("enter");
    let transfers = {
        actor_init(&mut engine, A, 0);
        drain(&mut engine)
    };
    assert_eq!(transfers.len(), 1, "the arrival cell fires the warp back");
    assert_eq!(&*transfers[0].dest_map, MAP);
    assert_eq!(transfers[0].rewarp_count, 1, "landed inside an area");
}

// ---------- ENG-WARP-06 across transfers ----------

#[test]
fn eng_warp_06_guard_counts_across_transfers() {
    let mut maps = [
        instance(
            grid(150, 150, &[]),
            vec![warp_to("w1", (50, 50), (1, 1), "geffen", (100, 100))],
        ),
        map2(vec![warp_to(
            "w2",
            (100, 100),
            (1, 1),
            "prontera",
            (50, 50),
        )]),
    ];
    enter(&mut maps[0], A, (50, 50), 0);
    let mut warps = 0;
    let mut here = 0;
    for k in 1..30u64 {
        let transfers = drain(&mut maps[here]);
        let Some(t) = transfers.into_iter().next() else {
            break;
        };
        warps += 1;
        here = 1 - here;
        let target = &mut maps[here];
        assert_eq!(&*t.dest_map, target.map_name());
        target
            .enter_by_warp(t.spawn, t.dest, t.rewarp_count, k * 100)
            .expect("enter");
        actor_init(target, A, k * 100);
    }
    assert_eq!(warps, 11);
    assert_eq!(here, 1, "the 11th warp went to map 2");
    assert_eq!(
        pos(&maps[1], A),
        (100, 100),
        "stays at the 11th destination"
    );
    assert_eq!(maps[0].character_position(char_id(A)), None);
}
