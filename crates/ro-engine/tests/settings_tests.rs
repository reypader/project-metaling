//! Map-entry settings and the load confirm (spec `specs/map-entry-settings/spec.md`
//! ENG-SET-01..04).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition,
    ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{MapGrid, MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_engine::MapInstance;

const MAP: &str = "prontera.gat";
const A: u32 = 1;
const B: u32 = 2;

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

fn spawn_data(n: u32, x: u16, y: u16) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(2_000_000 + n),
        char_id: char_id(n),
        name: format!("Char{n}"),
        sex: Sex::Female,
        job_class: 0,
        base_level: 10,
        hair_style: 2,
        hair_color: 3,
        pos: MapPosition { x, y, dir: 0 },
        speed: 150,
        str: 1,
        group_id: 0,
        inventory: Default::default(),
    }
}

/// A same-map warp at `at` (single cell) to `dest`.
fn same_map_warp(at: (u16, u16), dest: (u16, u16)) -> NpcSpawn {
    NpcSpawn {
        name: "w".into(),
        unique_name: "w".into(),
        x: at.0,
        y: at.1,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some((0, 0)),
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: "prontera".into(),
            x: dest.0,
            y: dest.1,
            trigger_while_hidden: false,
        }),
        source: "test:1".into(),
    }
}

fn instance(npcs: Vec<NpcSpawn>) -> MapInstance {
    let grid = MapGrid::all_walkable(200, 200).expect("valid dims");
    let mut engine = MapInstance::new(MAP, Arc::new(grid));
    engine.spawn_npcs(&MapNpcs { npcs });
    engine
}

fn tick(engine: &mut MapInstance, now: u64) -> Vec<ScopedServerEvent> {
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick never fails");
    out
}

fn push_tick(
    engine: &mut MapInstance,
    n: u32,
    i: ClientIntent,
    now: u64,
) -> Vec<ScopedServerEvent> {
    engine.push_intent(intent(n, i));
    tick(engine, now)
}

/// Settings events delivered to `n` only, in order.
fn settings(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| e.event.clone())
        .filter(|e| {
            matches!(
                e,
                ServerEvent::SkillList
                    | ServerEvent::ShortcutKeyList { .. }
                    | ServerEvent::PartyConfig { .. }
                    | ServerEvent::EquipWindowConfig { .. }
                    | ServerEvent::ConfigChanged { .. }
                    | ServerEvent::ReputationList
            )
        })
        .collect()
}

fn changemap_part() -> Vec<ServerEvent> {
    let mut part = vec![
        ServerEvent::PartyConfig {
            deny_invites: false,
        },
        ServerEvent::EquipWindowConfig { show_equip: false },
    ];
    for config_type in [2, 1, 3, 5] {
        part.push(ServerEvent::ConfigChanged {
            config_type,
            value: 0,
        });
    }
    part.push(ServerEvent::ReputationList);
    part
}

#[test]
fn eng_set_01_login_sends_the_settings_group_once() {
    let mut engine = instance(Vec::new());
    engine
        .spawn_character(spawn_data(B, 103, 100), 0)
        .expect("spawn B");
    push_tick(&mut engine, B, ClientIntent::NotifyActorInit, 0);
    engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn A");
    let events = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 20);

    let mut expected = vec![
        ServerEvent::SkillList,
        ServerEvent::ShortcutKeyList { tab: 0 },
        ServerEvent::ShortcutKeyList { tab: 1 },
    ];
    expected.extend(changemap_part());
    assert_eq!(settings(&events, A), expected);

    // After the unit entries: B's standing entry comes before the first settings event.
    let standing = events
        .iter()
        .position(|e| matches!(&e.event, ServerEvent::UnitStanding { .. }))
        .expect("B's entry");
    let first_setting = events
        .iter()
        .position(|e| e.event == ServerEvent::SkillList)
        .expect("skill list");
    assert!(standing < first_setting, "{events:#?}");
    assert!(
        settings(&events, B).is_empty(),
        "B gets none of A's settings"
    );

    let again = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 40);
    assert!(settings(&again, A).is_empty(), "{again:#?}");
}

#[test]
fn eng_set_02_cross_map_arrival_sends_only_the_changemap_part() {
    let mut engine = instance(Vec::new());
    engine
        .enter_by_warp(spawn_data(A, 0, 0), (100, 100), 0, 0)
        .expect("arrive");
    let events = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 20);
    assert_eq!(settings(&events, A), changemap_part());
}

#[test]
fn eng_set_03_same_map_warp_sends_none() {
    let mut engine = instance(vec![same_map_warp((103, 100), (50, 50))]);
    engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn");
    push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 0);
    engine.push_intent(intent(
        A,
        ClientIntent::RequestMove {
            dest_x: 103,
            dest_y: 100,
        },
    ));
    let mut warped = false;
    for now in (50..=1000).step_by(50) {
        warped |= tick(&mut engine, now)
            .iter()
            .any(|e| matches!(e.event, ServerEvent::PlayerMapMove { .. }));
    }
    assert!(warped, "the walk reached the warp");

    let events = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 1050);
    assert!(
        events
            .iter()
            .any(|e| matches!(e.event, ServerEvent::InventoryStart)),
        "the actor init ran: {events:#?}"
    );
    assert!(settings(&events, A).is_empty(), "{events:#?}");
}

#[test]
fn eng_set_04_load_confirm() {
    let mut engine = instance(Vec::new());
    engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn");
    let early = push_tick(&mut engine, A, ClientIntent::BlockingPlayCancel, 0);
    assert!(early.is_empty(), "ignored before actor init: {early:#?}");

    push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 20);
    let events = push_tick(&mut engine, A, ClientIntent::BlockingPlayCancel, 40);
    assert_eq!(
        events,
        vec![ScopedServerEvent::new(
            BroadcastScope::SelfOnly(char_id(A)),
            ServerEvent::LoadConfirmed
        )]
    );
}

#[test]
fn eng_set_05_login_on_a_warp_cell_sends_settings_before_the_warp() {
    let mut engine = instance(vec![same_map_warp((100, 100), (50, 50))]);
    engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn on the warp cell");
    let events = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 0);
    let last_setting = events
        .iter()
        .rposition(|e| e.event == ServerEvent::ReputationList)
        .expect("settings sent");
    let map_move = events
        .iter()
        .position(|e| matches!(e.event, ServerEvent::PlayerMapMove { .. }))
        .expect("the arrival cell warps");
    assert!(last_setting < map_move, "{events:#?}");

    let after_warp = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 50);
    assert!(settings(&after_warp, A).is_empty(), "{after_warp:#?}");
}
