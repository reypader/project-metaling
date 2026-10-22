//! Static NPCs in a map instance (spec `specs/npc-spawning/spec.md` ENG-NPC-01..04).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition,
    ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex, UnitAppearance, UnitObjectType,
};
use ro_db::{MapGrid, MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_engine::{MapInstance, START_NPC_ID};

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

fn npc(name: &str, x: u16, y: u16, sprite: i16) -> NpcSpawn {
    NpcSpawn {
        name: name.into(),
        unique_name: name.into(),
        x,
        y,
        dir: 4,
        sprite,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: None,
        duplicate_of: None,
        kind: NpcKind::Script,
        source: "test:1".into(),
    }
}

fn warp(name: &str, x: u16, y: u16) -> NpcSpawn {
    NpcSpawn {
        dir: 0,
        trigger: Some((1, 1)),
        kind: NpcKind::Warp(WarpDest {
            map: "geffen".into(),
            x: 5,
            y: 6,
            trigger_while_hidden: false,
        }),
        ..npc(name, x, y, 45)
    }
}

fn instance(npcs: Vec<NpcSpawn>) -> (MapInstance, usize) {
    let grid = Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    let created = engine.spawn_npcs(&MapNpcs { npcs });
    (engine, created)
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

// ---------- ENG-NPC-01 / 02 ----------

#[test]
fn eng_npc_01_spawn_skips_out_of_bounds_and_emits_nothing() {
    let (mut engine, created) = instance(vec![
        npc("Kafra#1", 100, 110, 117),
        npc("Outside", 300, 300, 117),
    ]);
    assert_eq!(created, 1);
    assert!(tick(&mut engine, 0).is_empty(), "spawning sends nothing");
}

#[test]
fn eng_npc_02_visible_npcs_are_shown_on_entry() {
    let mut cloaked = npc("Cloaked", 104, 104, 117);
    cloaked.state = NpcState::Cloaked;
    let mut mob = npc("PoringNpc", 105, 105, 1002);
    mob.sprite_is_mob = true;
    let mut disabled = npc("Disabled", 102, 102, 117);
    disabled.state = NpcState::Disabled;
    let (mut engine, created) = instance(vec![
        npc("Kafra#1", 100, 110, 117),
        npc("Fake", 101, 101, -1),
        disabled,
        npc("Invisible", 103, 103, 32767),
        npc("HiddenWarpSprite", 100, 103, 139),
        warp("prt001", 106, 100),
        cloaked,
        mob,
        npc("FarAway", 150, 150, 117),
    ]);
    assert_eq!(created, 9);

    let shown = standing_to_a(&enter(&mut engine, 100, 100));
    let names: Vec<&str> = shown.iter().map(|u| &*u.name).collect();
    assert_eq!(names.len(), 5, "{names:?}");
    for want in [
        "Kafra#1",
        "HiddenWarpSprite",
        "prt001",
        "Cloaked",
        "PoringNpc",
    ] {
        assert!(names.contains(&want), "{want} missing from {names:?}");
    }

    let kafra = shown.iter().find(|u| &*u.name == "Kafra#1").expect("kafra");
    assert_eq!(kafra.object_type, UnitObjectType::Npc);
    assert_eq!(
        kafra.gid, START_NPC_ID,
        "ids count from START_NPC_ID in spawn order"
    );
    assert_eq!(kafra.char_id, None);
    assert_eq!((kafra.job, kafra.speed, kafra.clevel), (117, 200, 0));
    assert_eq!((kafra.pos.x, kafra.pos.y, kafra.pos.dir), (100, 110, 4));

    let warp = shown.iter().find(|u| &*u.name == "prt001").expect("warp");
    assert_eq!((warp.job, warp.object_type), (45, UnitObjectType::Npc));
    let cloaked = shown
        .iter()
        .find(|u| &*u.name == "Cloaked")
        .expect("cloaked");
    assert_eq!(cloaked.effect_state, 0x4);
    let mob = shown
        .iter()
        .find(|u| &*u.name == "PoringNpc")
        .expect("mob sprite");
    assert_eq!(mob.object_type, UnitObjectType::MobSpriteNpc);
    assert!(
        shown.iter().all(|u| u.gid >= START_NPC_ID),
        "NPC ids never collide with account ids"
    );
}

#[test]
fn eng_npc_02_npcs_are_not_aoi_recipients() {
    let (mut engine, _) = instance(vec![npc("Kafra#1", 100, 101, 117)]);
    enter(&mut engine, 100, 100);
    let mut out = Vec::new();
    engine.query_aoi_characters(
        MapPosition {
            x: 100,
            y: 100,
            dir: 0,
        },
        &mut out,
    );
    assert_eq!(out, [char_id(A)]);
}

// ---------- ENG-NPC-03 ----------

#[test]
fn eng_npc_03_walking_into_and_out_of_view() {
    let (mut engine, _) = instance(vec![npc("Kafra#1", 100, 120, 117)]);
    assert!(
        standing_to_a(&enter(&mut engine, 100, 100)).is_empty(),
        "20 cells away"
    );

    let events = walk(&mut engine, (100, 106), 1_000, 2_500);
    let shown = standing_to_a(&events);
    assert_eq!(shown.len(), 1, "{events:#?}");
    assert_eq!(&*shown[0].name, "Kafra#1");
    assert!(
        events
            .iter()
            .all(|e| matches!(e.scope, BroadcastScope::SelfOnly(c) if c == char_id(A))),
        "nothing addressed to anyone but A: {events:#?}"
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

// ---------- ENG-NPC-04 ----------

#[test]
fn eng_npc_04_name_request() {
    let (mut engine, _) = instance(vec![
        npc("Kafra Employee#prt1", 100, 110, 117),
        npc("Far", 100, 130, 117),
    ]);
    enter(&mut engine, 100, 100);
    engine.push_intent(intent(
        A,
        ClientIntent::RequestEntityName {
            entity_id: START_NPC_ID,
        },
    ));
    engine.push_intent(intent(
        A,
        ClientIntent::RequestEntityName {
            entity_id: START_NPC_ID + 1,
        },
    ));
    let events = tick(&mut engine, 10);
    assert_eq!(
        events,
        [ScopedServerEvent::new(
            BroadcastScope::SelfOnly(char_id(A)),
            ServerEvent::AckNpcName {
                gid: START_NPC_ID,
                name: "Kafra Employee#prt1".into(),
            }
        )],
        "only the NPC in range answers"
    );
}
