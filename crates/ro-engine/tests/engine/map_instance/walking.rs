//! Walking: steps, AoI updates, move requests and redirects (ENG-04, 08, 10, 12).

use super::*;

// ---------- ENG-08 ----------

#[test]
fn eng_08_cell_changes_at_step_end() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 102,
            dest_y: 100,
        },
    );
    let events = h.tick(1_000);
    assert!(events.contains(&ScopedServerEvent::new(
        self_only(A),
        ServerEvent::PlayerMoved {
            start: (100, 100),
            dest: (102, 100),
            move_start_tick: 1_000
        }
    )));
    h.tick(1_050);
    assert_eq!(h.pos(A), (100, 100));
    h.tick(1_149);
    assert_eq!(h.pos(A), (100, 100));
    h.tick(1_150);
    assert_eq!(h.pos(A), (101, 100));
    assert!(h.engine.is_walking(char_id(A)));
    h.tick(1_300);
    assert_eq!(h.pos(A), (102, 100));
    assert!(!h.engine.is_walking(char_id(A)), "path removed at the end");
}

#[test]
fn eng_08_single_late_tick_runs_every_step_with_aoi_diff() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.enter(D, 118, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 104,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    let events = h.tick(2_000);
    assert_eq!(h.pos(A), (104, 100));
    assert!(!h.engine.is_walking(char_id(A)));
    // D comes into view on the 4th (final) step: both sides see each other standing.
    assert!(events.iter().any(|e| e.scope == self_only(A)
        && matches!(&e.event, ServerEvent::UnitStanding { unit } if unit.gid == account(D))));
    assert!(events.iter().any(|e| e.scope == self_only(D)
        && matches!(&e.event, ServerEvent::UnitStanding { unit }
                if unit.gid == account(A) && (unit.pos.x, unit.pos.y) == (104, 100))));
}

// ---------- ENG-04 (engine level) ----------

#[test]
fn eng_04_visibility_events_while_walking() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 103, 100);
    h.enter(D, 120, 100);
    h.enter(B, 100, 100); // observer O
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 115,
            dest_y: 100,
        },
    );
    h.tick(10_000);
    let mut seen = Vec::new();
    for t in (10_150..=12_000).step_by(150) {
        for e in h.tick(t) {
            seen.push((h.pos(A).0, e));
        }
    }
    let at = |x: u16| -> Vec<&ScopedServerEvent> {
        seen.iter()
            .filter(|(px, _)| *px == x)
            .map(|(_, e)| e)
            .collect()
    };
    let at106 = at(106);
    assert_eq!(at106.len(), 2, "{at106:#?}");
    assert!(at106.iter().any(|e| e.scope == self_only(A)
        && matches!(&e.event, ServerEvent::UnitStanding { unit } if unit.gid == account(D))));
    assert!(at106.iter().any(|e| e.scope == self_only(D)
        && matches!(&e.event, ServerEvent::UnitWalking { unit, start, dest, .. }
            if unit.gid == account(A) && *start == (106, 100) && *dest == (115, 100))));
    let at115 = at(115);
    assert_eq!(at115.len(), 2, "{at115:#?}");
    assert!(at115.contains(&&ScopedServerEvent::new(
        self_only(A),
        ServerEvent::UnitVanished {
            gid: account(B),
            vanish_type: 0
        }
    )));
    assert!(at115.contains(&&ScopedServerEvent::new(
        self_only(B),
        ServerEvent::UnitVanished {
            gid: account(A),
            vanish_type: 0
        }
    )));
    assert_eq!(seen.len(), 4, "no other visibility events: {seen:#?}");
}

// ---------- ENG-10 ----------

#[test]
fn eng_10_rejected_moves() {
    let mut h = Harness::new(grid(&[(105, 100)]));
    h.enter(A, 100, 100);
    h.enter(B, 101, 101);
    for (t, dest) in [(10, (250, 100)), (20, (105, 100)), (30, (130, 100))] {
        h.push(
            A,
            ClientIntent::RequestMove {
                dest_x: dest.0,
                dest_y: dest.1,
            },
        );
        assert!(h.tick(t).is_empty(), "move to {dest:?} must be silent");
        assert_eq!(h.pos(A), (100, 100));
        assert!(!h.engine.is_walking(char_id(A)));
    }
}

#[test]
fn eng_10_click_on_own_cell_resyncs_mover_only() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.enter(B, 101, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 100,
            dest_y: 100,
        },
    );
    assert_eq!(
        h.tick(10),
        vec![ScopedServerEvent::new(
            self_only(A),
            ServerEvent::PlayerMoved {
                start: (100, 100),
                dest: (100, 100),
                move_start_tick: 10
            }
        )]
    );
    assert!(!h.engine.is_walking(char_id(A)));
}

#[test]
fn eng_10_last_move_in_a_tick_wins() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 90,
            dest_y: 100,
        },
    );
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 103,
            dest_y: 100,
        },
    );
    let events = h.tick(10);
    let moved: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.event, ServerEvent::PlayerMoved { .. }))
        .collect();
    assert_eq!(moved.len(), 1, "{events:#?}");
    assert!(matches!(
        moved[0].event,
        ServerEvent::PlayerMoved {
            dest: (103, 100),
            ..
        }
    ));
}

#[test]
fn eng_10_walk_start_notifies_aoi_excluding_mover() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.enter(B, 103, 100);
    h.push(
        A,
        ClientIntent::ChangeDirection {
            head_dir: 1,
            body_dir: 2,
        },
    );
    h.tick(5);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 100,
            dest_y: 106,
        },
    );
    let events = h.tick(10);
    assert_eq!(events.len(), 2, "{events:#?}");
    let walking = events
        .iter()
        .find(|e| matches!(e.event, ServerEvent::UnitWalking { .. }))
        .expect("observers get UnitWalking");
    assert_eq!(walking.scope, to(&[B]));
    match &walking.event {
        ServerEvent::UnitWalking {
            unit,
            start,
            dest,
            move_start_tick,
        } => {
            assert_eq!(unit.gid, account(A));
            assert_eq!(unit.head_dir, 0, "walking resets head dir");
            assert_eq!(
                (*start, *dest, *move_start_tick),
                ((100, 100), (100, 106), 10)
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn eng_12_redirect_applies_at_step_end() {
    let mut h = Harness::new(grid(&[]));
    start_walk(&mut h);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 101,
            dest_y: 105,
        },
    );
    assert!(h.tick(1_075).is_empty(), "nothing is sent at click time");
    let events = h.tick(1_150);
    assert_eq!(h.pos(A), (101, 100));
    assert!(events.contains(&ScopedServerEvent::new(
        self_only(A),
        ServerEvent::PlayerMoved {
            start: (101, 100),
            dest: (101, 105),
            move_start_tick: 1_150
        }
    )));
    assert!(events.iter().any(|e| e.scope == to(&[B])
        && matches!(&e.event, ServerEvent::UnitWalking { start, dest, .. }
            if *start == (101, 100) && *dest == (101, 105))));
    h.tick(3_000);
    assert_eq!(h.pos(A), (101, 105));
}

#[test]
fn eng_12_invalid_redirect_keeps_old_walk() {
    let mut h = Harness::new(grid(&[(101, 105)]));
    start_walk(&mut h);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 101,
            dest_y: 105,
        },
    );
    assert!(h.tick(1_075).is_empty());
    let events = h.tick(1_150);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e.event, ServerEvent::PlayerMoved { .. })),
        "{events:#?}"
    );
    h.tick(3_000);
    assert_eq!(h.pos(A), (104, 100));
}

#[test]
fn eng_12_failed_repath_stops_the_walk() {
    let mut h = Harness::new(grid(&[]));
    start_walk(&mut h);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 101,
            dest_y: 105,
        },
    );
    assert!(h.tick(1_075).is_empty());
    assert!(h.engine.set_cell(101, 105, CellType::Wall));
    let events = h.tick(1_150);
    assert_eq!(h.pos(A), (101, 100));
    let stopped: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.event, ServerEvent::UnitStopped { .. }))
        .collect();
    assert_eq!(stopped.len(), 1, "{events:#?}");
    assert_eq!(stopped[0].scope, to(&[A, B]));
    assert_eq!(
        stopped[0].event,
        ServerEvent::UnitStopped {
            gid: account(A),
            x: 101,
            y: 100
        }
    );
    assert!(!h.engine.is_walking(char_id(A)));
    h.tick(3_000);
    assert_eq!(h.pos(A), (101, 100));
    assert!(events.iter().all(|e| unit_gid(&e.event).is_some()));
}

#[test]
fn eng_04_mover_entering_view_of_walker_gets_walking_form() {
    let mut h = Harness::new(grid(&[]));
    h.enter(A, 100, 100);
    h.enter(D, 115, 100);
    h.push(
        D,
        ClientIntent::RequestMove {
            dest_x: 115,
            dest_y: 110,
        },
    );
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 102,
            dest_y: 100,
        },
    );
    h.tick(1_000);
    let events = h.tick(1_150);
    assert!(
        events.iter().any(|e| e.scope == self_only(A)
            && matches!(&e.event, ServerEvent::UnitWalking { unit, dest, .. }
                if unit.gid == account(D) && *dest == (115, 110))),
        "{events:#?}"
    );
}

#[test]
fn eng_12_redirect_to_own_cell_walks_back_one_cell() {
    let mut h = Harness::new(grid(&[]));
    start_walk(&mut h);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 100,
            dest_y: 100,
        },
    );
    assert!(h.tick(1_075).is_empty());
    let events = h.tick(1_150);
    assert!(
        events.contains(&ScopedServerEvent::new(
            self_only(A),
            ServerEvent::PlayerMoved {
                start: (101, 100),
                dest: (100, 100),
                move_start_tick: 1_150
            }
        )),
        "{events:#?}"
    );
    h.tick(1_300);
    assert_eq!(h.pos(A), (100, 100));
    assert!(!h.engine.is_walking(char_id(A)));
}

#[test]
fn eng_12_empty_repath_stops_for_observers() {
    let mut h = Harness::new(grid(&[]));
    start_walk(&mut h);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 101,
            dest_y: 100,
        },
    );
    assert!(h.tick(1_075).is_empty());
    let events = h.tick(1_150);
    assert!(
        events.contains(&ScopedServerEvent::new(
            self_only(A),
            ServerEvent::PlayerMoved {
                start: (101, 100),
                dest: (101, 100),
                move_start_tick: 1_150
            }
        )),
        "{events:#?}"
    );
    assert!(
        events.contains(&ScopedServerEvent::new(
            to(&[B]),
            ServerEvent::UnitStopped {
                gid: account(A),
                x: 101,
                y: 100
            }
        )),
        "{events:#?}"
    );
    assert!(!h.engine.is_walking(char_id(A)));
}

#[test]
fn eng_12_click_after_a_due_step_validates_from_the_reached_cell() {
    let mut h = Harness::new(grid(&[]));
    start_walk(&mut h);
    // The first step ended at 1150; this click arrives at 1160.
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 101,
            dest_y: 105,
        },
    );
    let events = h.tick(1_160);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e.event, ServerEvent::PlayerMoved { .. })),
        "redirect is applied at the NEXT step end, not before the click: {events:#?}"
    );
    let events = h.tick(1_300);
    assert!(
        events.contains(&ScopedServerEvent::new(
            self_only(A),
            ServerEvent::PlayerMoved {
                start: (102, 100),
                dest: (101, 105),
                move_start_tick: 1_300
            }
        )),
        "{events:#?}"
    );
}

#[test]
fn eng_12_set_cell_copies_a_shared_grid() {
    let shared = grid(&[]);
    let mut h = Harness::new(Arc::clone(&shared));
    assert!(h.engine.set_cell(10, 10, CellType::Wall));
    assert!(!h.engine.set_cell(200, 10, CellType::Wall), "out of bounds");
    assert!(
        shared.is_walkable(10, 10),
        "other holders keep the original grid"
    );
}
