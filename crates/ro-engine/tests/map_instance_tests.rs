//! `MapInstance` scenarios ENG-04 and ENG-07..ENG-12 (spec §4).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition,
    ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{CellType, MapGrid};
use ro_engine::{EngineError, MapInstance};

const MAP: &str = "prontera.gat";
const A: u32 = 1;
const B: u32 = 2;
const C: u32 = 3;
const D: u32 = 4;

fn account(n: u32) -> u32 {
    2_000_000 + n
}
fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn spawn_data(n: u32, x: u16, y: u16) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(account(n)),
        char_id: char_id(n),
        name: format!("Char{n}"),
        sex: Sex::Female,
        job_class: 0,
        base_level: 10,
        hair_style: 2,
        hair_color: 3,
        pos: MapPosition { x, y, dir: 0 },
        speed: 150,
    }
}

fn grid(walls: &[(u16, u16)]) -> Arc<MapGrid> {
    let mut g = MapGrid::all_walkable(200, 200).expect("valid dims");
    for &(x, y) in walls {
        assert!(g.set(x, y, CellType::Wall));
    }
    Arc::new(g)
}

fn intent(n: u32, intent: ClientIntent) -> SessionIntent {
    SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(account(n))),
        Some(char_id(n)),
        intent,
    )
}

struct Harness {
    engine: MapInstance,
}

impl Harness {
    fn new(walkability: Arc<MapGrid>) -> Self {
        Self {
            engine: MapInstance::new(MAP, walkability),
        }
    }

    /// Spawns and makes visible (ActorInit) at t=0, discarding the entry events.
    fn enter(&mut self, n: u32, x: u16, y: u16) {
        self.engine
            .spawn_character(spawn_data(n, x, y), 0)
            .expect("spawn");
        self.engine
            .push_intent(intent(n, ClientIntent::NotifyActorInit));
        self.tick(0);
    }

    fn push(&mut self, n: u32, i: ClientIntent) {
        self.engine.push_intent(intent(n, i));
    }

    fn tick(&mut self, now: u64) -> Vec<ScopedServerEvent> {
        let mut out = Vec::new();
        self.engine
            .step_tick(now, &mut out)
            .expect("tick never fails");
        out
    }

    fn pos(&self, n: u32) -> (u16, u16) {
        let p = self.engine.character_position(char_id(n)).expect("present");
        (p.x, p.y)
    }

    fn aoi(&self, x: u16, y: u16) -> Vec<CharId> {
        let mut out = vec![CharId(999)];
        self.engine
            .query_aoi_characters(MapPosition { x, y, dir: 0 }, &mut out);
        out.sort();
        out
    }
}

fn self_only(n: u32) -> BroadcastScope {
    BroadcastScope::SelfOnly(char_id(n))
}

/// AoI recipients as the engine resolves them at emission: sorted char ids.
fn to(ns: &[u32]) -> BroadcastScope {
    let mut chars: Vec<CharId> = ns.iter().map(|&n| char_id(n)).collect();
    chars.sort();
    BroadcastScope::Characters(chars)
}

fn unit_gid(event: &ServerEvent) -> Option<u32> {
    match event {
        ServerEvent::UnitWalking { unit, .. }
        | ServerEvent::UnitStanding { unit }
        | ServerEvent::UnitSpawned { unit } => Some(unit.gid),
        ServerEvent::UnitVanished { gid, .. }
        | ServerEvent::UnitStopped { gid, .. }
        | ServerEvent::DirectionChanged { gid, .. }
        | ServerEvent::AckEntityName { gid, .. } => Some(*gid),
        _ => None,
    }
}

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

// ---------- ENG-09 ----------

#[test]
fn eng_09_lifecycle() {
    let mut h = Harness::new(grid(&[]));
    assert_eq!(h.engine.map_name(), MAP);
    h.enter(B, 103, 100);
    assert_eq!(
        h.engine.spawn_character(spawn_data(A, 100, 100), 0),
        Ok(Vec::new())
    );
    assert_eq!(
        h.aoi(100, 100),
        vec![char_id(B)],
        "not visible before ActorInit"
    );

    h.push(A, ClientIntent::NotifyActorInit);
    let events = h.tick(20);
    let kinds: Vec<(BroadcastScope, &ServerEvent)> =
        events.iter().map(|e| (e.scope.clone(), &e.event)).collect();
    assert_eq!(kinds.len(), 4, "{events:#?}");
    assert_eq!(kinds[0].0, self_only(A));
    assert!(matches!(
        kinds[0].1,
        ServerEvent::ParameterChanged { var_id: 24, .. }
    ));
    assert_eq!(kinds[1].0, self_only(A));
    assert!(matches!(
        kinds[1].1,
        ServerEvent::ParameterChanged { var_id: 25, .. }
    ));
    assert_eq!(kinds[2].0, to(&[B]));
    match kinds[2].1 {
        ServerEvent::UnitSpawned { unit } => {
            assert_eq!(unit.gid, account(A));
            assert_eq!(unit.char_id, Some(char_id(A)));
            assert_eq!(unit.name, "Char1");
            assert_eq!(unit.clevel, 10);
            assert_eq!(unit.speed, 150);
        }
        other => panic!("expected UnitSpawned, got {other:?}"),
    }
    assert_eq!(kinds[3].0, self_only(A));
    assert!(matches!(kinds[3].1, ServerEvent::UnitStanding { unit } if unit.gid == account(B)));
    assert_eq!(h.aoi(100, 100), vec![char_id(A), char_id(B)]);

    h.push(A, ClientIntent::NotifyActorInit);
    assert!(h.tick(40).is_empty(), "second ActorInit is ignored");

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

// ---------- ENG-11 ----------

fn act(n: u32, action_type: u8) -> ClientIntent {
    let _ = n;
    ClientIntent::RequestAction {
        target_id: 0,
        action_type,
    }
}

fn act_event(n: u32, action_type: u8) -> ServerEvent {
    ServerEvent::EntityAction {
        source_id: account(n),
        target_id: 0,
        action_type,
        action_tick: 0,
    }
}

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

// ---------- ENG-12 ----------

fn start_walk(h: &mut Harness) {
    h.enter(A, 100, 100);
    h.enter(B, 103, 102);
    h.push(
        A,
        ClientIntent::RequestMove {
            dest_x: 104,
            dest_y: 100,
        },
    );
    h.tick(1_000);
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

// ---------- review follow-ups ----------

fn entries_about(events: &[ScopedServerEvent], to_n: u32, about_n: u32) -> usize {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(to_n),
            BroadcastScope::Characters(list) => list.contains(&char_id(to_n)),
            _ => false,
        })
        .filter(|e| {
            matches!(&e.event,
                ServerEvent::UnitSpawned { unit } | ServerEvent::UnitStanding { unit }
                | ServerEvent::UnitWalking { unit, .. } if unit.gid == account(about_n))
        })
        .count()
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
