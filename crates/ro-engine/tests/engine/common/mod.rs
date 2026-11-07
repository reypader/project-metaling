//! Helpers shared by the test modules.

mod consts;

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, ItemId, LearnedSkill,
    MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex, SkillFlag,
    UnitAppearance,
};
use ro_db::{ItemData, ItemType, JobDb};
use ro_engine::MapInstance;

pub use consts::*;

pub fn account(n: u32) -> u32 {
    2_000_000 + n
}

pub fn ack(var_id: u16, ok: bool, value: u8) -> ServerEvent {
    ServerEvent::StatusChangeAck { var_id, ok, value }
}

pub fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

/// Spawns player A and runs ActorInit; returns the events of that tick.
pub fn enter(engine: &mut MapInstance, x: u16, y: u16) -> Vec<ScopedServerEvent> {
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
        status: Default::default(),
        group_id: 0,
        inventory: Default::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    engine.push_intent(intent(A, ClientIntent::NotifyActorInit));
    tick(engine, 0)
}

pub fn gid(n: u32) -> u32 {
    2_000_000 + n
}

pub fn index_of(events: &[ServerEvent], wanted: &ServerEvent) -> usize {
    events
        .iter()
        .position(|e| e == wanted)
        .unwrap_or_else(|| panic!("{wanted:?} not in {events:?}"))
}

pub fn intent(n: u32, intent: ClientIntent) -> SessionIntent {
    SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(2_000_000 + n)),
        Some(char_id(n)),
        intent,
    )
}

pub fn is_message(event: &ServerEvent) -> bool {
    matches!(event, ServerEvent::SelfChat { .. })
}

pub fn item_data(id: u32, item_type: ItemType, locations: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: format!("Item_{id}"),
        name: format!("Item {id}"),
        item_type,
        weight: 10,
        locations,
        ..ItemData::default()
    }
}

pub fn jobs() -> Arc<JobDb> {
    static JOBS: OnceLock<Arc<JobDb>> = OnceLock::new();
    Arc::clone(JOBS.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        Arc::new(JobDb::load_dir(&dir).expect("shipped job data loads"))
    }))
}

pub fn learned(skills: &[(u16, u8)]) -> Vec<LearnedSkill> {
    skills
        .iter()
        .map(|&(id, level)| LearnedSkill {
            id,
            level,
            flag: SkillFlag::Permanent,
        })
        .collect()
}

pub fn long(var_id: u16, value: i64) -> ServerEvent {
    ServerEvent::LongLongParameterChanged { var_id, value }
}

pub fn look(look_type: u8, value: u32, value2: u32) -> ServerEvent {
    ServerEvent::SpriteChanged {
        gid: gid(A),
        look_type,
        value,
        value2,
    }
}

pub fn name(n: u32) -> String {
    format!("Char{n}")
}

pub fn p(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

pub fn param(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

pub fn param_of(events: &[ServerEvent], var_id: u16) -> Option<i32> {
    events.iter().find_map(|e| match e {
        ServerEvent::ParameterChanged { var_id: v, value } if *v == var_id => Some(*value),
        _ => None,
    })
}

/// What character `n` receives, in order (its own and the area events).
pub fn seen_by(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(n),
            BroadcastScope::Characters(chars) => chars.contains(&char_id(n)),
            _ => false,
        })
        .map(|e| e.event.clone())
        .collect()
}

pub fn spawn_data(n: u32, x: u16, y: u16) -> CharacterSpawn {
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
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    }
}

/// Units shown to A (standing entries), in order.
pub fn standing_to_a(events: &[ScopedServerEvent]) -> Vec<UnitAppearance> {
    events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(A)))
        .filter_map(|e| match &e.event {
            ServerEvent::UnitStanding { unit } => Some(unit.clone()),
            _ => None,
        })
        .collect()
}

pub fn tick(engine: &mut MapInstance, now: u64) -> Vec<ScopedServerEvent> {
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick never fails");
    out
}

/// Walks A to `dest`, ticking every 50 ms until `until`.
pub fn walk(
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
