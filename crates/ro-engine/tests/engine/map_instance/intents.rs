//! Intent order and name requests (ENG-07).

use super::*;

// ---------- ENG-07 ----------

#[test]
fn eng_07_intents_processed_fifo() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.enter(B, 102, 100);
    h.push(
        A,
        ClientIntent::RequestTime {
            client_tick: 50_000,
        },
    );
    h.push(
        A,
        ClientIntent::ChangeDirection {
            head_dir: 2,
            body_dir: 4,
        },
    );
    h.push(
        A,
        ClientIntent::RequestEntityName {
            entity_id: account(B),
        },
    );
    let events = h.tick(5_000);
    assert_eq!(events.len(), 3, "{events:#?}");
    assert_eq!(
        events[0],
        ScopedServerEvent::new(
            self_only(A),
            ServerEvent::TimeSynchronized { server_tick: 5_000 }
        )
    );
    assert_eq!(events[1].scope, to(&[B]));
    assert_eq!(
        events[1].event,
        ServerEvent::DirectionChanged {
            gid: account(A),
            head_dir: 2,
            body_dir: 4
        }
    );
    assert_eq!(events[2].scope, self_only(A));
    match &events[2].event {
        ServerEvent::AckEntityName { gid, name, .. } => {
            assert_eq!(*gid, account(B));
            assert_eq!(name, "Char2");
        }
        other => panic!("expected AckEntityName, got {other:?}"),
    }
    let p = h.engine.character_position(char_id(A)).expect("present");
    assert_eq!(p.dir, 4);
}

#[test]
fn eng_07_unknown_character_intent_is_dropped() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(C, ClientIntent::RequestTime { client_tick: 1 });
    h.engine.push_intent(SessionIntent::new(
        SessionId(77),
        None,
        None,
        ClientIntent::RequestTime { client_tick: 1 },
    ));
    assert!(h.tick(10).is_empty());
}

#[test]
fn eng_07_name_request_unknown_or_out_of_range_is_ignored() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.enter(B, 115, 100);
    h.push(A, ClientIntent::RequestEntityName { entity_id: 12345 });
    h.push(
        A,
        ClientIntent::RequestEntityName {
            entity_id: account(B),
        },
    );
    assert!(h.tick(10).is_empty());
}

#[test]
fn eng_07_enter_map_intent_is_not_accepted() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::EnterMap {
            account_id: AccountId(account(A)),
            char_id: char_id(A),
            login_id1: 0,
            client_tick: 0,
            sex: Sex::Female,
        },
    );
    assert!(h.tick(10).is_empty());
}

#[test]
fn eng_07_name_request_for_invisible_unit_is_ignored() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.engine
        .spawn_character(spawn_data(B, 101, 100), 0)
        .expect("spawn");
    h.push(
        A,
        ClientIntent::RequestEntityName {
            entity_id: account(B),
        },
    );
    assert!(h.tick(10).is_empty());
}
