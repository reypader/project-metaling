//! `ro-core` vocabulary types: spawn snapshots and server events.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_core::{
    AccountId, CharId, CharacterLooks, CharacterSpawn, CharacterSummary, MapPosition, ServerEvent,
    Sex, UnitAppearance, UnitLooks, UnitObjectType, DEFAULT_WALK_SPEED,
};

fn summary() -> CharacterSummary {
    CharacterSummary {
        char_id: CharId(150001),
        account_id: AccountId(2000001),
        slot: 0,
        name: "Alice".to_string(),
        sex: Sex::Female,
        job_class: 4001,
        base_level: 42,
        job_level: 10,
        hair_style: 3,
        hair_color: 5,
        last_map: "prontera.gat".to_string(),
        last_pos: MapPosition {
            x: 100,
            y: 100,
            dir: 4,
        },
        delete_date: None,
        looks: CharacterLooks::default(),
    }
}

#[test]
fn character_spawn_from_summary_copies_appearance_and_defaults_speed() {
    let spawn = CharacterSpawn::from_summary(&summary());
    assert_eq!(spawn.account_id, AccountId(2000001));
    assert_eq!(spawn.char_id, CharId(150001));
    assert_eq!(spawn.name, "Alice");
    assert_eq!(spawn.sex, Sex::Female);
    assert_eq!(spawn.job_class, 4001);
    assert_eq!(spawn.base_level, 42);
    assert_eq!(spawn.hair_style, 3);
    assert_eq!(spawn.hair_color, 5);
    assert_eq!(
        spawn.pos,
        MapPosition {
            x: 100,
            y: 100,
            dir: 4
        }
    );
    assert_eq!(spawn.speed, DEFAULT_WALK_SPEED);
    assert_eq!(DEFAULT_WALK_SPEED, 150);
}

#[test]
fn unit_appearance_and_new_server_events_construct() {
    let unit = UnitAppearance {
        object_type: UnitObjectType::Pc,
        gid: 2000001,
        char_id: Some(CharId(150001)),
        name: "Alice".into(),
        job: 0,
        sex: Sex::Male,
        hair_style: 1,
        hair_color: 2,
        head_dir: 0,
        pos: MapPosition { x: 1, y: 1, dir: 4 },
        speed: 150,
        dead_sit: 0,
        clevel: 1,
        effect_state: 0,
        looks: UnitLooks::default(),
    };
    let events = [
        ServerEvent::PlayerMoved {
            start: (1, 1),
            dest: (2, 2),
            move_start_tick: 5,
        },
        ServerEvent::UnitWalking {
            unit: unit.clone(),
            start: (1, 1),
            dest: (2, 2),
            move_start_tick: 5,
        },
        ServerEvent::UnitStanding { unit: unit.clone() },
        ServerEvent::UnitSpawned { unit },
        ServerEvent::UnitVanished {
            gid: 2000001,
            vanish_type: 0,
        },
        ServerEvent::UnitStopped {
            gid: 2000001,
            x: 1,
            y: 1,
        },
        ServerEvent::DirectionChanged {
            gid: 2000001,
            head_dir: 2,
            body_dir: 4,
        },
        ServerEvent::AckEntityName {
            gid: 2000001,
            name: "Alice".into(),
            party_name: String::new(),
            guild_name: String::new(),
            position_name: String::new(),
            title_id: 0,
        },
        ServerEvent::PlayerMapMove {
            dest_map: "prontera.gat".into(),
            dest_x: 1,
            dest_y: 2,
        },
    ];
    assert_eq!(events.len(), 9);
}
