//! `MapInstance` scenarios ENG-04 and ENG-07..ENG-12 (spec §4).

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, ClientIntent, MapPosition, ScopedServerEvent, ServerEvent,
    SessionId, SessionIntent, Sex,
};
use ro_db::{CellType, MapGrid};
use ro_engine::{EngineError, MapInstance};

use crate::common::{account, char_id, spawn_data, A, B, C, MAP};

mod intents;
mod lifecycle;
mod sitting;
mod walking;

const D: u32 = 4;

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
