//! Contract-level tests for the engine-foundation touchpoints (spec §1).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_core::{
    AccountId, CellType, CharId, CharacterSpawn, CharacterSummary, ClientIntent, EngineError,
    MapEngine, MapPosition, ScopedServerEvent, ServerEvent, SessionIntent, Sex, UnitAppearance,
    DEFAULT_WALK_SPEED,
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
    }
}

#[test]
fn db_02_cell_type_from_raw_maps_rathena_values() {
    assert_eq!(CellType::from_raw(0), CellType::Walkable);
    assert_eq!(CellType::from_raw(1), CellType::Wall);
    assert_eq!(CellType::from_raw(2), CellType::Walkable2);
    assert_eq!(CellType::from_raw(3), CellType::Water);
    assert_eq!(CellType::from_raw(4), CellType::Walkable4);
    assert_eq!(CellType::from_raw(5), CellType::Cliff);
    assert_eq!(CellType::from_raw(6), CellType::Walkable6);
    assert_eq!(CellType::from_raw(7), CellType::Unknown(7));
    assert_eq!(CellType::from_raw(255), CellType::Unknown(255));
}

#[test]
fn db_02_cell_type_walkability() {
    for raw in [0u8, 2, 3, 4, 6] {
        assert!(
            CellType::from_raw(raw).is_walkable(),
            "raw {raw} must be walkable"
        );
    }
    for raw in [1u8, 5, 7, 8, 200, 255] {
        assert!(
            !CellType::from_raw(raw).is_walkable(),
            "raw {raw} must not be walkable"
        );
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
fn engine_error_new_variants_display() {
    assert!(EngineError::MapNotFound("x.gat".into())
        .to_string()
        .contains("x.gat"));
    assert!(EngineError::CharacterAlreadyEntered(CharId(7))
        .to_string()
        .contains('7'));
    assert!(!EngineError::WorldCapacityExceeded.to_string().is_empty());
}

#[test]
fn unit_appearance_and_new_server_events_construct() {
    let unit = UnitAppearance {
        gid: 2000001,
        char_id: Some(CharId(150001)),
        name: "Alice".to_string(),
        job: 0,
        sex: Sex::Male,
        hair_style: 1,
        hair_color: 2,
        head_dir: 0,
        pos: MapPosition { x: 1, y: 1, dir: 4 },
        speed: 150,
        dead_sit: 0,
        clevel: 1,
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

/// Minimal engine proving the trait is object-safe and usable behind `Box<dyn MapEngine>`.
struct NullEngine;

impl MapEngine for NullEngine {
    fn map_name(&self) -> &str {
        "null.gat"
    }
    fn spawn_character(
        &mut self,
        spawn: CharacterSpawn,
        _now_ms: u64,
    ) -> Result<Vec<ScopedServerEvent>, EngineError> {
        Err(EngineError::CharacterAlreadyEntered(spawn.char_id))
    }
    fn push_intent(&mut self, _intent: SessionIntent) {}
    fn step_tick(
        &mut self,
        _now_ms: u64,
        _out: &mut Vec<ScopedServerEvent>,
    ) -> Result<(), EngineError> {
        Ok(())
    }
    fn query_aoi_characters(&self, _center: MapPosition, out: &mut Vec<CharId>) {
        out.clear();
    }
    fn character_position(&self, _char_id: CharId) -> Option<MapPosition> {
        None
    }
    fn remove_character(&mut self, char_id: CharId) -> Result<Vec<ScopedServerEvent>, EngineError> {
        Err(EngineError::UnknownCharacter(char_id))
    }
}

#[test]
fn map_engine_is_object_safe() {
    let mut engine: Box<dyn MapEngine> = Box::new(NullEngine);
    assert_eq!(engine.map_name(), "null.gat");
    engine.push_intent(SessionIntent::new(
        ro_core::SessionId(1),
        None,
        None,
        ClientIntent::NotifyActorInit,
    ));
    let mut out = Vec::new();
    assert_eq!(engine.step_tick(0, &mut out), Ok(()));
    assert!(out.is_empty());
    assert_eq!(
        engine.remove_character(CharId(1)),
        Err(EngineError::UnknownCharacter(CharId(1)))
    );
}
