//! Sitting, standing and other actions (ENG-11).

use super::*;

#[test]
fn eng_11_sit_and_stand() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(A, act(A, 2));
    let events = h.tick(10);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].scope, to(&[A]));
    assert_eq!(events[0].event, act_event(A, 2));
    assert_eq!(h.engine.dead_sit(char_id(A)), Some(2));

    h.push(A, act(A, 2));
    assert_eq!(
        h.tick(20)[0].event,
        act_event(A, 2),
        "repeated sit re-broadcasts"
    );
    assert_eq!(h.engine.dead_sit(char_id(A)), Some(2));

    h.push(A, act(A, 3));
    let events = h.tick(30);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, act_event(A, 3));
    assert_eq!(h.engine.dead_sit(char_id(A)), Some(0));
}

#[test]
fn eng_11_sitting_unit_enters_view_with_sit_state() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(A, act(A, 2));
    h.tick(10);
    h.engine
        .spawn_character(spawn_data(B, 101, 100), 20)
        .expect("spawn");
    h.push(B, ClientIntent::NotifyActorInit);
    let events = h.tick(20);
    assert!(events.iter().any(|e| e.scope == self_only(B)
        && matches!(&e.event, ServerEvent::UnitStanding { unit }
            if unit.gid == account(A) && unit.dead_sit == 2)));
}

#[test]
fn eng_11_move_while_sitting_is_ignored() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(A, act(A, 2));
    h.tick(10);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 103,
            dest_y: 100,
        },
    );
    assert!(h.tick(20).is_empty());
    assert!(!h.engine.is_walking(char_id(A)));
}

#[test]
fn eng_11_sit_while_walking_stops_first() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 105,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    h.tick(1_150);
    assert_eq!(h.pos(A), (101, 100));
    h.push(A, act(A, 2));
    let events = h.tick(1_200);
    assert_eq!(events.len(), 2, "{events:#?}");
    assert_eq!(events[0].scope, to(&[A]));
    assert_eq!(
        events[0].event,
        ServerEvent::UnitStopped {
            gid: account(A),
            x: 101,
            y: 100
        }
    );
    assert_eq!(events[1].event, act_event(A, 2));
    assert!(!h.engine.is_walking(char_id(A)));
    h.tick(2_000);
    assert_eq!(h.pos(A), (101, 100));
}

#[test]
fn eng_11_other_actions_are_ignored() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    for t in [0u8, 1, 7, 99] {
        h.push(A, act(A, t));
    }
    assert!(h.tick(10).is_empty());
}

#[test]
fn eng_11_stand_while_walking_stops_first() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 105,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    h.push(A, act(A, 3));
    let events = h.tick(1_050);
    assert_eq!(events.len(), 2, "{events:#?}");
    assert_eq!(
        events[0].event,
        ServerEvent::UnitStopped {
            gid: account(A),
            x: 100,
            y: 100
        }
    );
    assert_eq!(events[1].event, act_event(A, 3));
    assert!(!h.engine.is_walking(char_id(A)));
}

#[test]
fn eng_11_attack_actions_do_not_stop_walking() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 105,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    h.push(A, act(A, 0));
    h.push(A, act(A, 7));
    assert!(h.tick(1_050).is_empty());
    assert!(h.engine.is_walking(char_id(A)));
}
