//! Map-entry settings and the load confirm (spec `specs/map-entry-settings/spec.md`
//! ENG-SET-01..08).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, AchievementSummary, BroadcastScope, CharId, CharacterSpawn, CharacterStatus,
    ClientIntent, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
    SkillInfo, StatusWindow,
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
        status: Default::default(),
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

/// `SP_WEIGHT` / `SP_MAXWEIGHT`: every actor init sends them, so they aren't settings.
const WEIGHT_VARS: [u16; 2] = [24, 25];

/// Settings and status events delivered to `n` only, in order.
fn settings(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| e.event.clone())
        .filter(|e| {
            matches!(
                e,
                ServerEvent::SkillList { .. }
                    | ServerEvent::ShortcutKeyList { .. }
                    | ServerEvent::LongLongParameterChanged { .. }
                    | ServerEvent::StatusWindow { .. }
                    | ServerEvent::CoupleStatus { .. }
                    | ServerEvent::AttackRange { .. }
                    | ServerEvent::StatusCost { .. }
                    | ServerEvent::PartyConfig { .. }
                    | ServerEvent::EquipWindowConfig { .. }
                    | ServerEvent::ConfigChanged { .. }
                    | ServerEvent::ReputationList
            ) || matches!(e, ServerEvent::ParameterChanged { var_id, .. } if !WEIGHT_VARS.contains(var_id))
        })
        .collect()
}

fn skill_list() -> ServerEvent {
    ServerEvent::SkillList {
        skills: vec![SkillInfo {
            id: 1,
            inf: 0,
            level: 0,
            sp: 0,
            range: 0,
            name: Arc::from("NV_BASIC"),
            upgradable: true,
        }],
    }
}

/// The login-only part: skill list, hotkeys and the status block (spec §3), for `window`'s
/// stats and the given exp and skill points.
fn login_part(
    window: StatusWindow,
    base_exp: i64,
    job_exp: i64,
    skill_point: i32,
) -> Vec<ServerEvent> {
    let mut part = vec![
        skill_list(),
        ServerEvent::ShortcutKeyList { tab: 0 },
        ServerEvent::ShortcutKeyList { tab: 1 },
    ];
    for (var_id, value) in [
        (1, base_exp),
        (22, 99_999_999),
        (2, job_exp),
        (23, 999_999_999),
    ] {
        part.push(ServerEvent::LongLongParameterChanged { var_id, value });
    }
    part.push(ServerEvent::ParameterChanged {
        var_id: 12,
        value: skill_point,
    });
    part.push(ServerEvent::StatusWindow { status: window });
    for (var_id, stat) in (13..=18).zip(window.stats) {
        part.push(ServerEvent::CoupleStatus {
            var_id,
            base: i32::from(stat),
            bonus: 0,
        });
    }
    part.push(ServerEvent::AttackRange { range: 1 });
    part.push(ServerEvent::ParameterChanged {
        var_id: 53,
        value: 0,
    });
    for var_id in 219..=224 {
        part.push(ServerEvent::CoupleStatus {
            var_id,
            base: 0,
            bonus: 0,
        });
    }
    for var_id in 225..=233 {
        part.push(ServerEvent::ParameterChanged { var_id, value: 0 });
    }
    for var_id in 247..=252 {
        part.push(ServerEvent::StatusCost { var_id, value: 0 });
    }
    part
}

/// The window of a fresh character: every stat 1 (cost 2), 48 status points.
fn fresh_window() -> StatusWindow {
    StatusWindow {
        status_point: 48,
        stats: [1; 6],
        costs: [2; 6],
        ..StatusWindow::default()
    }
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

    let mut expected = login_part(fresh_window(), 0, 0, 0);
    expected.extend(changemap_part());
    assert_eq!(settings(&events, A), expected);

    // After the unit entries: B's standing entry comes before the first settings event.
    let standing = events
        .iter()
        .position(|e| matches!(&e.event, ServerEvent::UnitStanding { .. }))
        .expect("B's entry");
    let first_setting = events
        .iter()
        .position(|e| e.event == skill_list())
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
    assert_eq!(
        settings(&events, A),
        changemap_part(),
        "no skill list, hotkeys or status block"
    );
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

#[test]
fn eng_set_06_status_block_carries_the_loaded_status() {
    let mut engine = instance(Vec::new());
    let mut spawn = spawn_data(A, 100, 100);
    spawn.status = CharacterStatus {
        str: 9,
        agi: 11,
        vit: 50,
        int: 98,
        dex: 99,
        luk: 1,
        status_point: 7,
        skill_point: 3,
        base_exp: 1234,
        job_exp: 56,
        ..CharacterStatus::default()
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    let events = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 0);

    let window = StatusWindow {
        status_point: 7,
        stats: [9, 11, 50, 98, 99, 1],
        // 2 + (s-1)/10 below the cap of 99, 0 at it.
        costs: [2, 3, 6, 11, 0, 2],
        ..StatusWindow::default()
    };
    let mut expected = login_part(window, 1234, 56, 3);
    expected.extend(changemap_part());
    assert_eq!(settings(&events, A), expected);
}

fn param(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

#[test]
fn eng_set_07_login_burst_before_the_first_actor_init() {
    let mut engine = instance(Vec::new());
    let mut spawn = spawn_data(A, 100, 101);
    spawn.status = CharacterStatus {
        str: 5,
        agi: 6,
        hp: 31,
        max_hp: 45,
        sp: 9,
        max_sp: 12,
        ..CharacterStatus::default()
    };
    let burst = engine.spawn_character(spawn, 0).expect("spawn");
    assert!(
        burst
            .iter()
            .all(|e| e.scope == BroadcastScope::SelfOnly(char_id(A))),
        "{burst:#?}"
    );
    let events: Vec<ServerEvent> = burst.into_iter().map(|e| e.event).collect();

    let mut expected = vec![
        ServerEvent::SelfChat {
            text: b"Welcome to Metaling.".to_vec(),
        },
        ServerEvent::PlayerMapMove {
            dest_map: MAP.to_string(),
            dest_x: 100,
            dest_y: 101,
        },
        param(25, 20_000 + 300 * 5),
    ];
    for (var_id, base) in (13..=18).zip([5, 6, 1, 1, 1, 1]) {
        expected.push(ServerEvent::CoupleStatus {
            var_id,
            base,
            bonus: 0,
        });
    }
    for var_id in [49, 50, 53, 41, 45, 46, 42, 51, 52, 44, 43, 48, 47] {
        expected.push(param(var_id, 0));
    }
    expected.extend([
        ServerEvent::AttackRange { range: 1 },
        param(6, 45),
        param(8, 12),
        param(5, 31),
        param(7, 9),
        param(24, 0),
        ServerEvent::UnreadMail { unread: false },
        ServerEvent::AchievementUpdate {
            summary: AchievementSummary {
                exp_to_next: 18,
                ..AchievementSummary::default()
            },
        },
        ServerEvent::OverweightPercent { percent: 70 },
    ]);
    assert_eq!(events, expected);
    assert!(tick(&mut engine, 10).is_empty(), "returned, not queued");

    // A cross-map arrival gets no login burst: the map-server sends its 0x0091.
    let mut other = instance(Vec::new());
    other
        .enter_by_warp(spawn_data(B, 0, 0), (100, 100), 0, 0)
        .expect("arrives");
    assert!(tick(&mut other, 10).is_empty());
}

/// Self events of an actor init with the position of the `0x01d7` look and the `0x099b`.
fn look_and_property(events: &[ScopedServerEvent], n: u32) -> (Option<usize>, Option<usize>) {
    let mine: Vec<&ServerEvent> = events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| &e.event)
        .collect();
    (
        mine.iter()
            .position(|e| matches!(e, ServerEvent::SpriteChanged { .. })),
        mine.iter()
            .position(|e| matches!(e, ServerEvent::MapProperty { .. })),
    )
}

#[test]
fn eng_set_08_every_entry_sends_the_look_and_the_map_property() {
    let mut engine = instance(vec![same_map_warp((100, 103), (50, 50))]);
    engine
        .spawn_character(spawn_data(B, 103, 100), 0)
        .expect("spawn B");
    push_tick(&mut engine, B, ClientIntent::NotifyActorInit, 0);
    engine
        .spawn_character(spawn_data(A, 100, 100), 0)
        .expect("spawn A");
    let events = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 10);

    let mine: Vec<&ServerEvent> = events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(A)))
        .map(|e| &e.event)
        .collect();
    assert_eq!(
        *mine[0],
        ServerEvent::SpriteChanged {
            gid: 2_000_000 + A,
            look_type: 2,
            value: 0,
            value2: 0,
        },
        "the look comes first"
    );
    let property = mine
        .iter()
        .position(|e| matches!(e, ServerEvent::MapProperty { .. }))
        .expect("0x099b sent");
    assert_eq!(
        *mine[property],
        ServerEvent::MapProperty {
            property: 0,
            flags: 0x0600,
        }
    );
    assert!(
        matches!(
            mine[property - 1],
            ServerEvent::ParameterChanged { var_id: 25, .. }
        ),
        "after the weights: {mine:#?}"
    );
    assert!(
        matches!(mine[property + 1], ServerEvent::UnitStanding { .. }),
        "before the unit entries: {mine:#?}"
    );
    let spawned = events
        .iter()
        .position(|e| matches!(e.event, ServerEvent::UnitSpawned { .. }))
        .expect("B sees A");
    let property_at = events
        .iter()
        .position(|e| matches!(e.event, ServerEvent::MapProperty { .. }))
        .expect("0x099b sent");
    assert!(spawned < property_at, "after the spawn to others");
    assert!(
        !events
            .iter()
            .any(|e| e.scope != BroadcastScope::SelfOnly(char_id(A))
                && matches!(e.event, ServerEvent::SpriteChanged { .. })),
        "observers get no look yet"
    );

    // The second 0x007d after the login reload: inventory only.
    let again = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 20);
    assert_eq!(look_and_property(&again, A), (None, None), "{again:#?}");

    // A same-map warp sends both again on its actor init.
    engine.push_intent(intent(
        A,
        ClientIntent::RequestMove {
            dest_x: 100,
            dest_y: 103,
        },
    ));
    let mut warped = false;
    for now in (50..=1000).step_by(50) {
        warped |= tick(&mut engine, now)
            .iter()
            .any(|e| matches!(e.event, ServerEvent::PlayerMapMove { .. }));
    }
    assert!(warped, "the walk reached the warp");
    let after = push_tick(&mut engine, A, ClientIntent::NotifyActorInit, 1050);
    let (look, property) = look_and_property(&after, A);
    assert_eq!(look, Some(0), "{after:#?}");
    assert!(property.is_some(), "{after:#?}");

    // A cross-map arrival too.
    let mut other = instance(Vec::new());
    other
        .enter_by_warp(spawn_data(A, 0, 0), (100, 100), 0, 0)
        .expect("arrives");
    let arrival = push_tick(&mut other, A, ClientIntent::NotifyActorInit, 0);
    assert_eq!(look_and_property(&arrival, A).0, Some(0));
    assert!(look_and_property(&arrival, A).1.is_some());
}
