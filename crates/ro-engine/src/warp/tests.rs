use ro_db::NpcState;

use super::*;
use crate::ecs::Entity;
use crate::protocol_consts::DEAD;
use crate::MapInstance;

#[test]
fn cell_bits_set_get_and_clip() {
    let mut bits = CellBits::new(10, 7);
    bits.set(9, 6);
    bits.set(10, 0);
    assert!(bits.get(9, 6));
    assert!(!bits.get(10, 0), "out of bounds is never set");
    assert!(!bits.get(8, 6));
    assert!(bits.any_near(8, 5));
    assert!(!bits.any_near(7, 6));
}

use std::sync::Arc;

use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, Sex};
use ro_db::{MapGrid, MapNpcs, NpcKind, NpcSpawn, WarpDest};

fn warp(at: (u16, u16), dest: (u16, u16)) -> NpcSpawn {
    NpcSpawn {
        name: "w".into(),
        unique_name: "w".into(),
        x: at.0,
        y: at.1,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some((1, 1)),
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: "prontera".into(),
            x: dest.0,
            y: dest.1,
            trigger_while_hidden: false,
        }),
        source: "t:1".into(),
    }
}

/// A map with the given warps and one player (not yet in the grid) at `at`.
fn setup(npcs: Vec<NpcSpawn>, at: (u16, u16)) -> (MapInstance, Entity) {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("valid dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.spawn_npcs(&MapNpcs { npcs });
    let spawn = CharacterSpawn {
        account_id: AccountId(2_000_001),
        char_id: CharId(150_001),
        name: "A".into(),
        sex: Sex::Female,
        job_class: 0,
        base_level: 1,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition {
            x: at.0,
            y: at.1,
            dir: 0,
        },
        speed: 150,
        status: Default::default(),
        group_id: 0,
        inventory: Default::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    };
    let e = engine.world.spawn_player(&spawn).expect("spawn");
    engine.world.show(e);
    (engine, e)
}

fn rewarp_count(engine: &MapInstance, e: Entity) -> u8 {
    engine.world.players.get(e).expect("player").rewarp_count
}

/// ENG-WARP-04: a dead player never warps.
#[test]
fn eng_warp_04_dead_player_does_not_warp() {
    let (mut engine, e) = setup(vec![warp((50, 50), (80, 80))], (50, 50));
    engine.world.dead_sit[e.index()] = DEAD;
    engine.touch_warps(e);
    assert!(engine.events.is_empty());
    assert_eq!(engine.cell(e), (50, 50));
    assert!(engine.world.in_grid[e.index()]);
}

/// ENG-WARP-06: the count grows on landings inside an area and resets elsewhere.
#[test]
fn eng_warp_06_rewarp_count_increments_and_resets() {
    let (mut engine, e) = setup(
        vec![warp((50, 50), (80, 80)), warp((80, 80), (20, 20))],
        (50, 50),
    );
    engine.touch_warps(e);
    assert_eq!(engine.cell(e), (80, 80));
    assert_eq!(rewarp_count(&engine, e), 1, "landed in the second area");
    engine.world.show(e);
    engine.touch_warps(e);
    assert_eq!(engine.cell(e), (20, 20));
    assert_eq!(rewarp_count(&engine, e), 0, "landed outside every area");
}
