//! Permanent mob spawns in a map instance (spec `specs/mob-spawning/spec.md` ENG-MOB-01..05).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition,
    ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex, UnitAppearance, UnitObjectType,
};
use ro_db::{
    CellType, MapGrid, MapMobSpawns, MapNpcs, MobId, MobSpawn, NpcKind, NpcSpawn, NpcState,
};
use ro_engine::{MapInstance, MobView, START_NPC_ID};

const A: u32 = 1;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn intent(n: u32, intent: ClientIntent) -> SessionIntent {
    SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(2_000_000 + n)),
        Some(char_id(n)),
        intent,
    )
}

/// A spawn line. `(x, y)` with span `(0, 0)` and `x, y > 0` is a fixed 1x1 cell.
fn line(name: &str, x: u16, y: u16, span: (u16, u16), amount: u16) -> MobSpawn {
    MobSpawn {
        name: name.into(),
        mob_id: MobId(1002),
        x,
        y,
        span,
        amount,
        delay1: 5000,
        delay2: 0,
        level: None,
        boss: false,
        event: None,
        source: format!("test:{name}"),
    }
}

fn grid(w: u16, h: u16) -> MapGrid {
    MapGrid::all_walkable(w, h).expect("valid dims")
}

fn instance_on(grid: MapGrid, lines: Vec<MobSpawn>) -> (MapInstance, usize) {
    let mut engine = MapInstance::new("prt_fild08.gat", Arc::new(grid));
    let created = engine.spawn_mobs(&MapMobSpawns { spawns: lines });
    (engine, created)
}

fn instance(lines: Vec<MobSpawn>) -> (MapInstance, usize) {
    instance_on(grid(200, 200), lines)
}

/// Mobs sorted by gid.
fn mobs(engine: &MapInstance) -> Vec<MobView<'_>> {
    let mut v: Vec<_> = engine.mobs().collect();
    v.sort_by_key(|m| m.gid);
    v
}

fn tick(engine: &mut MapInstance, now: u64) -> Vec<ScopedServerEvent> {
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick never fails");
    out
}

/// Spawns player A and runs ActorInit; returns the events of that tick.
fn enter(engine: &mut MapInstance, x: u16, y: u16) -> Vec<ScopedServerEvent> {
    let spawn = CharacterSpawn {
        account_id: AccountId(2_000_000 + A),
        char_id: char_id(A),
        name: "Alice".into(),
        sex: Sex::Female,
        job_class: 0,
        base_level: 10,
        hair_style: 2,
        hair_color: 3,
        pos: MapPosition { x, y, dir: 0 },
        speed: 150,
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    engine.push_intent(intent(A, ClientIntent::NotifyActorInit));
    tick(engine, 0)
}

/// Units shown to A (standing entries), in order.
fn standing_to_a(events: &[ScopedServerEvent]) -> Vec<UnitAppearance> {
    events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(A)))
        .filter_map(|e| match &e.event {
            ServerEvent::UnitStanding { unit } => Some(unit.clone()),
            _ => None,
        })
        .collect()
}

/// Walks A to `dest`, ticking every 50 ms until `until`.
fn walk(
    engine: &mut MapInstance,
    dest: (u16, u16),
    from: u64,
    until: u64,
) -> Vec<ScopedServerEvent> {
    engine.push_intent(intent(
        A,
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

// ---------- ENG-MOB-01 ----------

#[test]
fn eng_mob_01_spawns_amount_per_line_with_placeholder_stats() {
    let mut leveled = line("Lunatic", 60, 60, (0, 0), 2);
    leveled.mob_id = MobId(1063);
    leveled.level = Some(12);
    let (engine, created) = instance(vec![line("Poring", 0, 0, (0, 0), 3), leveled]);
    assert_eq!(created, 5);

    let m = mobs(&engine);
    let gids: Vec<_> = m.iter().map(|m| m.gid).collect();
    assert_eq!(gids, (START_NPC_ID..START_NPC_ID + 5).collect::<Vec<_>>());
    for mob in &m {
        assert_eq!((mob.hp, mob.max_hp), (1, 1));
        assert_eq!(mob.speed, 150);
        assert_eq!(mob.pos.dir, 0);
        assert!(mob.visible, "every mob is in the grid");
    }
    let names: Vec<_> = m.iter().map(|m| (m.name, m.mob_id.0, m.level)).collect();
    assert_eq!(
        names,
        [
            ("Poring", 1002, 1),
            ("Poring", 1002, 1),
            ("Poring", 1002, 1),
            ("Lunatic", 1063, 12),
            ("Lunatic", 1063, 12),
        ]
    );
    assert_eq!(m[0].spawn.source, "test:Poring", "mob records its line");
    assert_eq!(m[4].spawn.source, "test:Lunatic");
}

#[test]
fn eng_mob_01_shares_the_unit_id_counter_with_npcs() {
    let mut engine = MapInstance::new("prontera.gat", Arc::new(grid(200, 200)));
    let npc = NpcSpawn {
        name: "Kafra".into(),
        unique_name: "Kafra".into(),
        x: 10,
        y: 10,
        dir: 0,
        sprite: 117,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: None,
        duplicate_of: None,
        kind: NpcKind::Script,
        source: "t:1".into(),
    };
    let second = NpcSpawn {
        x: 11,
        ..npc.clone()
    };
    assert_eq!(
        engine.spawn_npcs(&MapNpcs {
            npcs: vec![npc, second]
        }),
        2
    );
    let spawns = MapMobSpawns {
        spawns: vec![line("Poring", 50, 50, (0, 0), 2)],
    };
    assert_eq!(engine.spawn_mobs(&spawns), 2);
    let gids: Vec<_> = mobs(&engine).iter().map(|m| m.gid).collect();
    assert_eq!(gids, [START_NPC_ID + 2, START_NPC_ID + 3]);
}

#[test]
fn eng_mob_01_out_of_grid_line_is_skipped_and_spawning_emits_nothing() {
    let (mut engine, created) = instance(vec![
        line("Outside", 250, 10, (0, 0), 4),
        line("Inside", 20, 20, (0, 0), 1),
    ]);
    assert_eq!(created, 1);
    assert_eq!(mobs(&engine)[0].name, "Inside");
    assert!(tick(&mut engine, 0).is_empty(), "spawning sends nothing");
}

#[test]
fn eng_mob_01_capacity_stops_spawning() {
    let mut engine = MapInstance::with_capacity("prt_fild08.gat", Arc::new(grid(200, 200)), 3);
    let spawns = MapMobSpawns {
        spawns: vec![
            line("Poring", 0, 0, (0, 0), 10),
            line("Later", 30, 30, (0, 0), 1),
        ],
    };
    assert_eq!(engine.spawn_mobs(&spawns), 3);
    assert!(mobs(&engine).iter().all(|m| m.name == "Poring"));
}

// ---------- ENG-MOB-02 ----------

/// 200x200 with every third column a wall.
fn striped_grid() -> MapGrid {
    let mut g = grid(200, 200);
    for x in (0..200).step_by(3) {
        for y in 0..200 {
            assert!(g.set(x, y, CellType::Wall), "in bounds");
        }
    }
    g
}

#[test]
fn eng_mob_02_every_mob_is_on_a_walkable_cell() {
    let g = striped_grid();
    let (engine, created) = instance_on(
        g.clone(),
        vec![
            line("Wide", 0, 0, (0, 0), 100),
            line("Rect", 100, 100, (6, 4), 50),
            line("OnWall", 99, 99, (0, 0), 10),
        ],
    );
    assert_eq!(created, 160);
    for m in mobs(&engine) {
        assert!(g.is_walkable(m.pos.x, m.pos.y), "{m:?}");
    }
}

#[test]
fn eng_mob_02_rectangles_map_wide_and_single_cells() {
    let (engine, _) = instance(vec![
        line("Rect", 100, 100, (5, 3), 80),
        line("Wide", 0, 0, (0, 0), 80),
        line("Cell", 40, 60, (0, 0), 5),
    ]);
    for m in mobs(&engine) {
        let (x, y) = (m.pos.x, m.pos.y);
        match m.name {
            // The center moves by up to xs-1 and the search by xs-1 again.
            "Rect" => assert!(x.abs_diff(100) <= 8 && y.abs_diff(100) <= 4, "{x},{y}"),
            "Wide" => assert!(
                (15..=184).contains(&x) && (15..=184).contains(&y),
                "{x},{y}"
            ),
            "Cell" => assert_eq!((x, y), (40, 60)),
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn eng_mob_02_same_map_places_identically() {
    let lines = || {
        vec![
            line("Rect", 100, 100, (5, 3), 20),
            line("Wide", 0, 0, (0, 0), 20),
        ]
    };
    let (a, _) = instance_on(striped_grid(), lines());
    let (b, _) = instance_on(striped_grid(), lines());
    let cells = |e: &MapInstance| -> Vec<_> { mobs(e).iter().map(|m| (m.gid, m.pos)).collect() };
    assert_eq!(cells(&a), cells(&b));
}

// ---------- ENG-MOB-03 ----------

#[test]
fn eng_mob_03_unplaceable_mobs_are_skipped_and_the_rest_spawn() {
    // All wall except (2, 2): too close to the edge for any search, but a 1x1 line on it takes
    // its own cell.
    let mut g = grid(100, 100);
    for x in 0..100 {
        for y in 0..100 {
            if (x, y) != (2, 2) {
                assert!(g.set(x, y, CellType::Wall), "in bounds");
            }
        }
    }
    let (engine, created) = instance_on(
        g,
        vec![
            line("Nowhere", 50, 50, (5, 5), 3),
            line("Wide", 0, 0, (0, 0), 2),
            line("Corner", 2, 2, (0, 0), 1),
        ],
    );
    assert_eq!(created, 1);
    let m = mobs(&engine);
    assert_eq!((m[0].name, m[0].pos.x, m[0].pos.y), ("Corner", 2, 2));
    assert_eq!(m[0].gid, START_NPC_ID, "failed mobs take no unit id");
}

// ---------- ENG-MOB-04 ----------

#[test]
fn eng_mob_04_seen_on_entry() {
    let mut lv = line("Near", 105, 100, (0, 0), 1);
    lv.level = Some(7);
    let (mut engine, _) = instance(vec![lv, line("Far", 130, 100, (0, 0), 1)]);
    let shown = standing_to_a(&enter(&mut engine, 100, 100));
    assert_eq!(shown.len(), 1, "{shown:#?}");
    let unit = &shown[0];
    assert_eq!(unit.object_type, UnitObjectType::Mob);
    assert_eq!(unit.gid, START_NPC_ID);
    assert_eq!(unit.char_id, None);
    assert_eq!(&*unit.name, "Near");
    assert_eq!((unit.job, unit.clevel, unit.speed), (1002, 7, 150));
    assert_eq!(
        (unit.pos.x, unit.pos.y, unit.pos.dir, unit.dead_sit),
        (105, 100, 0, 0)
    );
}

#[test]
fn eng_mob_04_walking_into_and_out_of_view() {
    let (mut engine, _) = instance(vec![line("Poring", 100, 120, (0, 0), 1)]);
    assert!(
        standing_to_a(&enter(&mut engine, 100, 100)).is_empty(),
        "20 cells away"
    );

    let events = walk(&mut engine, (100, 106), 1_000, 2_500);
    let shown = standing_to_a(&events);
    assert_eq!(shown.len(), 1, "{events:#?}");
    assert_eq!(
        (shown[0].object_type, &*shown[0].name),
        (UnitObjectType::Mob, "Poring")
    );
    assert!(
        events
            .iter()
            .all(|e| matches!(e.scope, BroadcastScope::SelfOnly(c) if c == char_id(A))),
        "mobs receive nothing: {events:#?}"
    );

    let events = walk(&mut engine, (100, 100), 3_000, 4_500);
    let vanished: Vec<_> = events
        .iter()
        .filter_map(|e| match e.event {
            ServerEvent::UnitVanished { gid, vanish_type } => Some((gid, vanish_type)),
            _ => None,
        })
        .collect();
    assert_eq!(vanished, [(START_NPC_ID, 0)]);
}

// ---------- ENG-MOB-05 ----------

#[test]
fn eng_mob_05_name_request() {
    let (mut engine, _) = instance(vec![
        line("Poring", 100, 110, (0, 0), 1),
        line("Far", 100, 130, (0, 0), 1),
    ]);
    enter(&mut engine, 100, 100);
    for entity_id in [START_NPC_ID, START_NPC_ID + 1, START_NPC_ID + 99] {
        engine.push_intent(intent(A, ClientIntent::RequestEntityName { entity_id }));
    }
    let events = tick(&mut engine, 10);
    assert_eq!(
        events,
        [ScopedServerEvent::new(
            BroadcastScope::SelfOnly(char_id(A)),
            ServerEvent::AckNpcName {
                gid: START_NPC_ID,
                name: "Poring".into(),
            }
        )],
        "only the mob in range answers; unknown gids get nothing"
    );
}
