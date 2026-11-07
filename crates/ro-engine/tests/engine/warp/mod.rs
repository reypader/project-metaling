//! Warp portals in a map instance (spec `specs/warp-map-change/spec.md` ENG-WARP-01..07).

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharacterSpawn, ClientIntent, MapPosition, ScopedServerEvent,
    ServerEvent, SessionId, SessionIntent,
};
use ro_db::{CellType, MapGrid, MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_engine::{EngineError, MapInstance, MapTransfer};

use crate::common::{account, char_id, spawn_data, tick, A, B, C, MAP};

mod cross_map;
mod same_map;

fn intent(n: u32, intent: ClientIntent) -> SessionIntent {
    SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(account(n))),
        Some(char_id(n)),
        intent,
    )
}

/// A warp at `(x, y)` with half-extents `half`, to `dest` on `map` (bare name).
fn warp_to(
    name: &str,
    (x, y): (u16, u16),
    half: (u16, u16),
    map: &str,
    dest: (u16, u16),
) -> NpcSpawn {
    NpcSpawn {
        name: name.into(),
        unique_name: name.into(),
        x,
        y,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some(half),
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: map.into(),
            x: dest.0,
            y: dest.1,
            trigger_while_hidden: false,
        }),
        source: "test:1".into(),
    }
}

/// A same-map warp.
fn warp(name: &str, at: (u16, u16), half: (u16, u16), dest: (u16, u16)) -> NpcSpawn {
    warp_to(name, at, half, "prontera", dest)
}

fn grid(width: u16, height: u16, walls: &[(u16, u16)]) -> Arc<MapGrid> {
    let mut g = MapGrid::all_walkable(width, height).expect("valid dims");
    for &(x, y) in walls {
        assert!(g.set(x, y, CellType::Wall), "wall in bounds");
    }
    Arc::new(g)
}

fn instance(walkability: Arc<MapGrid>, npcs: Vec<NpcSpawn>) -> MapInstance {
    let mut engine = MapInstance::new(MAP, walkability);
    let count = npcs.len();
    assert_eq!(
        engine.spawn_npcs(&MapNpcs { npcs }),
        count,
        "all NPCs spawn"
    );
    engine
}

/// Spawns `n` at `(x, y)` and runs ActorInit at `now`; returns that tick's events.
fn enter(engine: &mut MapInstance, n: u32, (x, y): (u16, u16), now: u64) -> Vec<ScopedServerEvent> {
    engine
        .spawn_character(spawn_data(n, x, y), now)
        .expect("spawn");
    actor_init(engine, n, now)
}

fn actor_init(engine: &mut MapInstance, n: u32, now: u64) -> Vec<ScopedServerEvent> {
    engine.push_intent(intent(n, ClientIntent::NotifyActorInit));
    tick(engine, now)
}

/// Requests a walk for `n` at `from`, then ticks every 50 ms through `until`.
fn walk(
    engine: &mut MapInstance,
    n: u32,
    dest: (u16, u16),
    from: u64,
    until: u64,
) -> Vec<ScopedServerEvent> {
    engine.push_intent(intent(
        n,
        ClientIntent::RequestMove {
            dest_x: dest.0,
            dest_y: dest.1,
        },
    ));
    let mut events = Vec::new();
    for now in (from..=until).step_by(50) {
        events.extend(tick(engine, now));
    }
    events
}

/// Events delivered to character `n`, in order.
fn to(events: &[ScopedServerEvent], n: u32) -> Vec<&ServerEvent> {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(n),
            BroadcastScope::Characters(cs) => cs.contains(&char_id(n)),
            _ => false,
        })
        .map(|e| &e.event)
        .collect()
}

/// `PlayerMapMove`s delivered to `n`.
fn map_moves(events: &[ScopedServerEvent], n: u32) -> Vec<(String, u16, u16)> {
    to(events, n)
        .into_iter()
        .filter_map(|e| match e {
            ServerEvent::PlayerMapMove {
                dest_map,
                dest_x,
                dest_y,
            } => Some((dest_map.clone(), *dest_x, *dest_y)),
            _ => None,
        })
        .collect()
}

/// Whether any event mentions unit `gid` (spawn, standing, walking or vanish).
fn mentions(events: &[&ServerEvent], gid: u32) -> bool {
    events.iter().any(|e| match e {
        ServerEvent::UnitSpawned { unit }
        | ServerEvent::UnitStanding { unit }
        | ServerEvent::UnitWalking { unit, .. } => unit.gid == gid,
        ServerEvent::UnitVanished { gid: g, .. } => *g == gid,
        _ => false,
    })
}

fn vanished(events: &[&ServerEvent], gid: u32) -> Vec<u8> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::UnitVanished {
                gid: g,
                vanish_type,
            } if *g == gid => Some(*vanish_type),
            _ => None,
        })
        .collect()
}

fn pos(engine: &MapInstance, n: u32) -> (u16, u16) {
    let p = engine
        .character_position(char_id(n))
        .expect("character present");
    (p.x, p.y)
}

/// Enters `n` at `cell`, reports whether that fired a warp, then removes `n` again.
fn arrival_fires(engine: &mut MapInstance, cell: (u16, u16)) -> bool {
    let events = enter(engine, A, cell, 0);
    engine.remove_character(char_id(A)).expect("remove");
    !map_moves(&events, A).is_empty()
}

// ---------- ENG-WARP-04: inert warps ----------

fn with_state(state: NpcState) -> NpcSpawn {
    NpcSpawn {
        state,
        ..warp("w", (50, 50), (1, 1), (80, 80))
    }
}

fn walk_in_from(engine: &mut MapInstance, start: (u16, u16)) -> Vec<ScopedServerEvent> {
    enter(engine, A, start, 0);
    walk(engine, A, (55, 50), 50, 2000)
}

/// Warps once to `dest` and returns the cell picked.
fn random_landing(walkability: Arc<MapGrid>, dest: (u16, u16)) -> (u16, u16) {
    let mut engine = instance(walkability, vec![warp("w", (50, 50), (1, 1), dest)]);
    let events = enter(&mut engine, A, (50, 50), 0);
    let moves = map_moves(&events, A);
    assert_eq!(moves.len(), 1, "{events:?}");
    (moves[0].1, moves[0].2)
}

// ---------- ENG-WARP-03: cross-map transfer out ----------

const MAP2: &str = "geffen.gat";

fn drain(engine: &mut MapInstance) -> Vec<MapTransfer> {
    let mut out = Vec::new();
    engine.drain_transfers(&mut out);
    out
}

// ---------- ENG-WARP-08: enter_by_warp ----------

fn map2(npcs: Vec<NpcSpawn>) -> MapInstance {
    let mut engine = MapInstance::new(MAP2, grid(200, 200, &[]));
    let count = npcs.len();
    assert_eq!(engine.spawn_npcs(&MapNpcs { npcs }), count);
    engine
}
