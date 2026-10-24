//! Warp portals in a map instance (spec `specs/warp-map-change/spec.md` ENG-WARP-01..07).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition,
    ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{CellType, MapGrid, MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_engine::{EngineError, MapInstance, MapTransfer};

const MAP: &str = "prontera.gat";
const A: u32 = 1;
const B: u32 = 2;
const C: u32 = 3;

fn account(n: u32) -> u32 {
    2_000_000 + n
}

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn intent(n: u32, intent: ClientIntent) -> SessionIntent {
    SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(account(n))),
        Some(char_id(n)),
        intent,
    )
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
        status: Default::default(),
        group_id: 0,
        inventory: Default::default(),
    }
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

fn tick(engine: &mut MapInstance, now: u64) -> Vec<ScopedServerEvent> {
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick never fails");
    out
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

// ---------- ENG-WARP-01: trigger index ----------

#[test]
fn eng_warp_01_area_marks_walkable_cells_only() {
    let wall = (51, 50);
    let mut engine = instance(
        grid(100, 100, &[wall]),
        vec![warp("w", (50, 50), (2, 1), (90, 90))],
    );
    for y in 46..=54 {
        for x in 45..=55 {
            let inside = (48..=52).contains(&x) && (49..=51).contains(&y);
            let expect = inside && (x, y) != wall;
            assert_eq!(
                arrival_fires(&mut engine, (x, y)),
                expect,
                "cell ({x}, {y})"
            );
        }
    }
}

#[test]
fn eng_warp_01_area_is_clipped_to_the_map() {
    let mut engine = instance(
        grid(20, 20, &[]),
        vec![
            warp("low", (1, 1), (3, 3), (10, 10)),
            warp("high", (19, 19), (2, 2), (10, 10)),
        ],
    );
    assert!(arrival_fires(&mut engine, (0, 0)));
    assert!(arrival_fires(&mut engine, (4, 4)));
    assert!(!arrival_fires(&mut engine, (5, 4)));
    assert!(arrival_fires(&mut engine, (19, 19)));
    assert!(arrival_fires(&mut engine, (17, 17)));
    assert!(!arrival_fires(&mut engine, (16, 19)));
}

#[test]
fn eng_warp_01_warp_without_trigger_marks_nothing() {
    let mut spanless = warp("w", (50, 50), (1, 1), (90, 90));
    spanless.trigger = None;
    let mut engine = instance(grid(100, 100, &[]), vec![spanless]);
    assert!(!arrival_fires(&mut engine, (50, 50)));
}

#[test]
fn eng_warp_01_overlap_earlier_spawn_wins() {
    let first = warp("first", (50, 50), (1, 1), (10, 10));
    let second = warp("second", (51, 50), (1, 1), (90, 90));
    let mut engine = instance(grid(100, 100, &[]), vec![first.clone(), second.clone()]);
    let events = enter(&mut engine, A, (51, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 10, 10)]);

    let mut engine = instance(grid(100, 100, &[]), vec![second, first]);
    let events = enter(&mut engine, A, (51, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 90, 90)]);
}

// ---------- ENG-WARP-02: same-map warp, mid-path ----------

#[test]
fn eng_warp_02_same_map_warp_mid_path() {
    // Area x 59..=61, y 49..=51; the walk east along y=50 (16 cells, under the click limit)
    // enters it at x=59.
    let mut engine = instance(
        grid(200, 200, &[]),
        vec![warp("w", (60, 50), (1, 1), (100, 100))],
    );
    enter(&mut engine, B, (55, 52), 0); // in view of the source only
    enter(&mut engine, C, (100, 105), 0); // in view of the destination only
    enter(&mut engine, A, (50, 50), 0);
    let gid_a = account(A);

    // Nine 150 ms steps reach x=59 at t=1350.
    let mut events = walk(&mut engine, A, (66, 50), 0, 1300);
    assert!(map_moves(&events, A).is_empty());
    assert_eq!(pos(&engine, A), (58, 50));
    let fired = tick(&mut engine, 1350);
    assert_eq!(map_moves(&fired, A), [(MAP.to_string(), 100, 100)]);
    events.extend(fired);
    for now in (1400..=2000).step_by(50) {
        events.extend(tick(&mut engine, now));
    }
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 100, 100)]);
    assert_eq!(vanished(&to(&events, B), gid_a), [0], "observer B");
    assert!(
        vanished(&to(&events, A), gid_a).is_empty(),
        "self gets no vanish"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.event, ServerEvent::UnitStopped { .. })),
        "no stop on a warp"
    );
    assert!(
        !mentions(&to(&events, C), gid_a),
        "destination sees nothing yet"
    );
    assert_eq!(pos(&engine, A), (100, 100));
    assert!(!engine.is_walking(char_id(A)));

    // Out of the grid: moves are ignored until ActorInit.
    let events = walk(&mut engine, A, (105, 100), 2050, 2500);
    assert!(events.is_empty(), "move while loading: {events:?}");
    assert_eq!(pos(&engine, A), (100, 100));

    let events = actor_init(&mut engine, A, 2550);
    let c_sees: Vec<_> = to(&events, C)
        .into_iter()
        .filter(|e| matches!(e, ServerEvent::UnitSpawned { unit } if unit.gid == gid_a))
        .collect();
    assert_eq!(c_sees.len(), 1, "destination observer gets the spawn");
    assert!(
        to(&events, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::UnitStanding { unit } if unit.gid == account(C))),
        "self sees units at the destination"
    );
    assert!(
        !mentions(&to(&events, B), gid_a),
        "source observer is out of view"
    );

    let events = walk(&mut engine, A, (103, 100), 2600, 2600);
    assert!(
        to(&events, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::PlayerMoved { .. })),
        "moves work again"
    );

    assert!(actor_init(&mut engine, A, 2650)
        .iter()
        .all(|e| !matches!(e.event, ServerEvent::UnitSpawned { .. })));
}

#[test]
fn eng_warp_02_second_actor_init_only_resends_inventory() {
    let mut engine = instance(
        grid(100, 100, &[]),
        vec![warp("w", (50, 50), (1, 1), (80, 80))],
    );
    let events = enter(&mut engine, A, (50, 50), 0);
    assert_eq!(map_moves(&events, A).len(), 1);
    assert!(!actor_init(&mut engine, A, 100).is_empty());
    // A repeat only resends the inventory lists (specs/items ENG-INV-04): no warp, no spawn.
    let again = actor_init(&mut engine, A, 200);
    assert!(map_moves(&again, A).is_empty());
    assert_eq!(again.len(), 5, "{again:#?}");
    assert!(again.iter().all(|e| matches!(
        e.event,
        ServerEvent::InventoryStart
            | ServerEvent::InventoryEnd
            | ServerEvent::EquipSwitchList
            | ServerEvent::ParameterChanged { .. }
    )));
}

// ---------- ENG-WARP-04: inert warps ----------

fn with_state(state: NpcState) -> NpcSpawn {
    NpcSpawn {
        state,
        ..warp("w", (50, 50), (1, 1), (80, 80))
    }
}

#[test]
fn eng_warp_04_disabled_and_cloaked_never_fire() {
    for state in [NpcState::Disabled, NpcState::Cloaked] {
        let mut engine = instance(grid(100, 100, &[]), vec![with_state(state)]);
        enter(&mut engine, B, (52, 52), 0);
        let mut events = enter(&mut engine, A, (50, 50), 0);
        events.extend(walk(&mut engine, A, (60, 50), 50, 2000));
        assert!(map_moves(&events, A).is_empty(), "{state:?}");
        assert!(
            vanished(&to(&events, B), account(A)).is_empty(),
            "{state:?}"
        );
    }
}

#[test]
fn eng_warp_04_hidden_warp_fires() {
    let mut engine = instance(grid(100, 100, &[]), vec![with_state(NpcState::Hidden)]);
    let events = walk_in_from(&mut engine, (45, 50));
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
}

fn walk_in_from(engine: &mut MapInstance, start: (u16, u16)) -> Vec<ScopedServerEvent> {
    enter(engine, A, start, 0);
    walk(engine, A, (55, 50), 50, 2000)
}

// ---------- ENG-WARP-05: arrival-cell chain ----------

#[test]
fn eng_warp_05_login_on_warp_cell_spawns_then_vanishes() {
    let mut engine = instance(
        grid(100, 100, &[]),
        vec![warp("w", (50, 50), (1, 1), (80, 80))],
    );
    enter(&mut engine, B, (52, 52), 0);
    let events = enter(&mut engine, A, (50, 50), 0);
    let b_sees = to(&events, B);
    let spawned = b_sees
        .iter()
        .position(|e| matches!(e, ServerEvent::UnitSpawned { unit } if unit.gid == account(A)));
    let vanished_at = b_sees.iter().position(
        |e| matches!(e, ServerEvent::UnitVanished { gid, vanish_type: 0 } if *gid == account(A)),
    );
    assert!(spawned.is_some() && vanished_at.is_some(), "{b_sees:?}");
    assert!(spawned < vanished_at, "spawn first, then vanish");
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
    // The entry events for B came before the warp.
    let a_sees = to(&events, A);
    let saw_b = a_sees
        .iter()
        .position(|e| matches!(e, ServerEvent::UnitStanding { unit } if unit.gid == account(B)));
    let moved = a_sees
        .iter()
        .position(|e| matches!(e, ServerEvent::PlayerMapMove { .. }));
    assert!(saw_b < moved);
}

#[test]
fn eng_warp_05_chain_after_same_map_warp() {
    let mut engine = instance(
        grid(150, 150, &[]),
        vec![
            warp("one", (50, 50), (1, 1), (80, 80)),
            warp("two", (80, 80), (1, 1), (120, 120)),
        ],
    );
    let events = enter(&mut engine, A, (50, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
    let events = actor_init(&mut engine, A, 100);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 120, 120)]);
    let events = actor_init(&mut engine, A, 200);
    assert!(
        map_moves(&events, A).is_empty(),
        "arrival outside every area"
    );
    assert_eq!(pos(&engine, A), (120, 120));
}

// ---------- ENG-WARP-06: rewarp guard ----------

#[test]
fn eng_warp_06_eleven_chained_warps_then_skip() {
    let mut engine = instance(
        grid(150, 150, &[]),
        vec![
            warp("one", (50, 50), (1, 1), (80, 80)),
            warp("two", (80, 80), (1, 1), (50, 50)),
        ],
    );
    let mut warps = map_moves(&enter(&mut engine, A, (50, 50), 0), A).len();
    for k in 1..20 {
        warps += map_moves(&actor_init(&mut engine, A, k * 100), A).len();
    }
    assert_eq!(warps, 11);
    assert_eq!(pos(&engine, A), (80, 80), "stays at the 11th destination");

    // The skip reset the count: the next step inside the area warps again.
    let events = walk(&mut engine, A, (80, 83), 3000, 4000);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 50, 50)]);
}

// ---------- ENG-WARP-07: destination cell ----------

#[test]
fn eng_warp_07_in_bounds_wall_is_used_as_is() {
    let mut engine = instance(
        grid(100, 100, &[(80, 80)]),
        vec![warp("w", (50, 50), (1, 1), (80, 80))],
    );
    let events = enter(&mut engine, A, (50, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
}

/// Warps once to `dest` and returns the cell picked.
fn random_landing(walkability: Arc<MapGrid>, dest: (u16, u16)) -> (u16, u16) {
    let mut engine = instance(walkability, vec![warp("w", (50, 50), (1, 1), dest)]);
    let events = enter(&mut engine, A, (50, 50), 0);
    let moves = map_moves(&events, A);
    assert_eq!(moves.len(), 1, "{events:?}");
    (moves[0].1, moves[0].2)
}

#[test]
fn eng_warp_07_zero_and_out_of_bounds_pick_a_random_free_cell() {
    for dest in [(0, 0), (500, 10), (10, 100)] {
        let (x, y) = random_landing(grid(100, 100, &[]), dest);
        assert!(
            (1..=98).contains(&x) && (1..=98).contains(&y),
            "{dest:?} -> ({x}, {y})"
        );
        assert!(
            x.abs_diff(50) > 2 || y.abs_diff(50) > 2,
            "not within 1 of a warp cell: ({x}, {y})"
        );
    }
}

#[test]
fn eng_warp_07_random_cell_skips_walls_and_cells_near_warps() {
    // 7x7: the random range is x, y in 1..=5. Only (1, 1) and (5, 5) are walkable there, and
    // (5, 5) touches the warp cell (5, 4).
    let mut walls = Vec::new();
    for y in 1..=5 {
        for x in 1..=5 {
            if ![(1, 1), (5, 5), (5, 4)].contains(&(x, y)) {
                walls.push((x, y));
            }
        }
    }
    let mut engine = instance(grid(7, 7, &walls), vec![warp("w", (5, 4), (0, 0), (0, 0))]);
    let events = enter(&mut engine, A, (5, 4), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 1, 1)]);
}

#[test]
fn eng_warp_07_no_free_cell_means_no_warp_and_no_events() {
    // 5x5: the random range 1..=3 is all inside the warp area.
    let mut engine = instance(grid(5, 5, &[]), vec![warp("w", (2, 2), (1, 1), (0, 0))]);
    enter(&mut engine, B, (0, 0), 0);
    let events = enter(&mut engine, A, (2, 2), 0);
    assert!(map_moves(&events, A).is_empty());
    assert!(vanished(&to(&events, B), account(A)).is_empty());
    assert_eq!(pos(&engine, A), (2, 2));
    // Still in the grid: moves work.
    let events = walk(&mut engine, A, (3, 3), 50, 50);
    assert!(to(&events, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::PlayerMoved { .. })));
}

#[test]
fn eng_warp_07_rng_is_deterministic_per_map_name() {
    let first = random_landing(grid(100, 100, &[]), (0, 0));
    let again = random_landing(grid(100, 100, &[]), (0, 0));
    assert_eq!(first, again);
}

// ---------- ENG-WARP-03: cross-map transfer out ----------

const MAP2: &str = "geffen.gat";

fn drain(engine: &mut MapInstance) -> Vec<MapTransfer> {
    let mut out = Vec::new();
    engine.drain_transfers(&mut out);
    out
}

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

// ---------- ENG-WARP-08: enter_by_warp ----------

fn map2(npcs: Vec<NpcSpawn>) -> MapInstance {
    let mut engine = MapInstance::new(MAP2, grid(200, 200, &[]));
    let count = npcs.len();
    assert_eq!(engine.spawn_npcs(&MapNpcs { npcs }), count);
    engine
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
