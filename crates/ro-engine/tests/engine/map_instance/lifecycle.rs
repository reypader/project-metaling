//! Spawn, actor init and removal (ENG-09).

use super::*;

// ---------- ENG-09 ----------

#[test]
fn eng_09_lifecycle() {
    let mut h = Harness::new(grid(&[]));
    assert_eq!(h.engine.map_name(), MAP);
    h.enter(B, 103, 100);
    let burst = h
        .engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawns");
    // The login burst before the first 0x007d, all to self (specs/map-entry-settings ENG-SET-07).
    assert_eq!(burst.len(), 31, "{burst:#?}");
    assert!(burst.iter().all(|e| e.scope == self_only(A)));
    assert_eq!(
        h.aoi(100, 100),
        vec![char_id(B)],
        "not visible before ActorInit"
    );

    h.push(A, ClientIntent::NotifyActorInit);
    let events = h.tick(20);
    let kinds: Vec<(BroadcastScope, &ServerEvent)> =
        events.iter().map(|e| (e.scope.clone(), &e.event)).collect();
    // 9 entry events, then the 10 login settings events and the 35-event status block
    // (specs/map-entry-settings ENG-SET-01, ENG-SET-06, ENG-SET-08).
    assert_eq!(kinds.len(), 54, "{events:#?}");
    // The weapon look, then the (empty) inventory lists and weights (specs/items ENG-INV-04).
    for (scope, _) in &kinds[..6] {
        assert_eq!(*scope, self_only(A));
    }
    assert!(matches!(
        kinds[0].1,
        ServerEvent::SpriteChanged {
            look_type: 2,
            value: 0,
            value2: 0,
            ..
        }
    ));
    assert!(matches!(kinds[1].1, ServerEvent::InventoryStart));
    assert!(matches!(kinds[3].1, ServerEvent::EquipSwitchList));
    assert!(matches!(
        kinds[4].1,
        ServerEvent::ParameterChanged {
            var_id: 24,
            value: 0
        }
    ));
    assert!(matches!(
        kinds[5].1,
        ServerEvent::ParameterChanged {
            var_id: 25,
            value: 20_300
        }
    ));
    assert_eq!(kinds[6].0, to(&[B]));
    match kinds[6].1 {
        ServerEvent::UnitSpawned { unit } => {
            assert_eq!(unit.gid, account(A));
            assert_eq!(unit.char_id, Some(char_id(A)));
            assert_eq!(&*unit.name, "Char1");
            assert_eq!(unit.clevel, 10);
            assert_eq!(unit.speed, 150);
        }
        other => panic!("expected UnitSpawned, got {other:?}"),
    }
    assert_eq!(kinds[7].0, self_only(A));
    assert!(matches!(kinds[7].1, ServerEvent::MapProperty { .. }));
    assert_eq!(kinds[8].0, self_only(A));
    assert!(matches!(kinds[8].1, ServerEvent::UnitStanding { unit } if unit.gid == account(B)));
    assert_eq!(h.aoi(100, 100), vec![char_id(A), char_id(B)]);

    h.push(A, ClientIntent::NotifyActorInit);
    let again = h.tick(40);
    assert_eq!(
        again.len(),
        5,
        "a second ActorInit only resends the inventory: {again:#?}"
    );
    assert!(again.iter().all(|e| e.scope == self_only(A)));

    let events = h.engine.remove_character(char_id(A)).expect("remove");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].scope, to(&[B]));
    assert_eq!(
        events[0].event,
        ServerEvent::UnitVanished {
            gid: account(A),
            vanish_type: 2
        }
    );
    assert_eq!(h.aoi(100, 100), vec![char_id(B)]);
    assert_eq!(h.engine.character_position(char_id(A)), None);
}

#[test]
fn eng_09_duplicate_spawn_and_unknown_remove() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    assert_eq!(
        h.engine.spawn_character(spawn_data(A, 50, 50), 0),
        Err(EngineError::CharacterAlreadyEntered(char_id(A)))
    );
    assert_eq!(
        h.engine.remove_character(char_id(C)),
        Err(EngineError::UnknownCharacter(char_id(C)))
    );
}

#[test]
fn eng_09_remove_before_actor_init_sends_nothing() {
    let mut h = Harness::new(grid(&[]));
    h.enter(B, 101, 100);
    h.engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn");
    assert_eq!(h.engine.remove_character(char_id(A)), Ok(Vec::new()));
}

#[test]
fn eng_09_spawn_out_of_bounds_is_rejected() {
    let mut h = Harness::new(grid(&[]));
    assert_eq!(
        h.engine.spawn_character(spawn_data(A, 250, 10), 0),
        Err(EngineError::InvalidCell { x: 250, y: 10 })
    );
}

#[test]
fn eng_09_gameplay_before_actor_init_is_ignored() {
    let mut h = Harness::new(grid(&[]));
    h.engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn");
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 102,
            dest_y: 100,
        },
    );
    h.push(A, ClientIntent::RequestTime { client_tick: 1 });
    let events = h.tick(10);
    assert_eq!(
        events,
        vec![ScopedServerEvent::new(
            self_only(A),
            ServerEvent::TimeSynchronized { server_tick: 10 }
        )]
    );
}

#[test]
fn eng_09_same_tick_actor_init_sends_one_entry_each_way() {
    let mut h = Harness::new(grid(&[]));
    for n in [A, B] {
        h.engine
            .spawn_character(spawn_data(n, 100 + n as u16, 100), 0)
            .expect("spawn");
        h.push(n, ClientIntent::NotifyActorInit);
    }
    let events = h.tick(10);
    assert_eq!(entries_about(&events, B, A), 1, "{events:#?}");
    assert_eq!(entries_about(&events, A, B), 1, "{events:#?}");
}

#[test]
fn eng_09_actor_init_sees_walking_unit_mid_walk() {
    let mut h = Harness::new(grid(&[]));
    h.enter(B, 100, 100);
    h.push(
        B,
        ClientIntent::RequestMove {
            dest_x: 105,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    h.tick(1_160);
    h.engine
        .spawn_character(spawn_data(A, 102, 102), 0)
        .expect("spawn");
    h.push(A, ClientIntent::NotifyActorInit);
    let events = h.tick(1_200);
    assert!(
        events.iter().any(|e| e.scope == self_only(A)
            && matches!(&e.event, ServerEvent::UnitWalking { unit, start, dest, move_start_tick }
                if unit.gid == account(B) && *start == (101, 100) && *dest == (105, 100)
                    && *move_start_tick == 1_200)),
        "{events:#?}"
    );
}

#[test]
fn eng_09_remove_mid_walk_then_respawn_starts_clean() {
    let mut h = Harness::new(grid(&[]));
    h.enter(B, 101, 101);
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 104,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    h.push(A, ClientIntent::NotifyActorInit);
    h.push(A, ClientIntent::RequestTime { client_tick: 1 });
    h.engine.remove_character(char_id(A)).expect("remove");
    h.engine
        .spawn_character(spawn_data(A, 50, 50), 1_100)
        .expect("respawn");
    let events = h.tick(2_000);
    assert!(
        events.is_empty(),
        "old intents and walk are gone: {events:#?}"
    );
    assert_eq!(h.pos(A), (50, 50));
    assert!(!h.engine.is_walking(char_id(A)));
    assert_eq!(
        h.aoi(50, 50),
        Vec::<CharId>::new(),
        "not visible until its own ActorInit"
    );
}
