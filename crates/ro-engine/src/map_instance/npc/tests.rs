use std::sync::Arc;

use ro_db::{MapGrid, WarpDest};

use super::*;
use crate::map_instance::START_NPC_ID;

/// ENG-NPC-05: a warp keeps its trigger area and destination for map change.
#[test]
fn eng_npc_05_warp_data_kept() {
    let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("valid dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    let dest = WarpDest {
        map: "geffen".into(),
        x: 119,
        y: 59,
        trigger_while_hidden: true,
    };
    let warp = NpcSpawn {
        name: "prt001".into(),
        unique_name: "prt001".into(),
        x: 10,
        y: 12,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some((2, 3)),
        duplicate_of: None,
        kind: NpcKind::Warp(dest.clone()),
        source: "t:1".into(),
    };
    assert_eq!(engine.spawn_npcs(&MapNpcs { npcs: vec![warp] }), 1);
    let e = engine
        .world
        .entity_by_gid(START_NPC_ID)
        .expect("warp entity");
    assert_eq!(
        engine.world.warps.get(e),
        Some(&WarpTrigger {
            half_extents: (2, 3),
            dest_map: Arc::from("geffen.gat"),
            dest: (dest.x, dest.y),
            trigger_while_hidden: true,
            same_map: false,
        })
    );
    assert_eq!(engine.world.kinds[e.index()], crate::world::EntityKind::Npc);
}

/// Disabled warps keep their state for the trigger check; a warp without a span gets no
/// trigger at all.
#[test]
fn eng_npc_05_disabled_and_spanless_warps() {
    let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("valid dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    let warp = |name: &str, state, trigger| NpcSpawn {
        name: name.into(),
        unique_name: name.into(),
        x: 5,
        y: 5,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state,
        trigger,
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: "geffen".into(),
            x: 1,
            y: 1,
            trigger_while_hidden: false,
        }),
        source: "t:1".into(),
    };
    let npcs = vec![
        warp("off", NpcState::Disabled, Some((1, 1))),
        warp("nospan", NpcState::Normal, None),
    ];
    assert_eq!(engine.spawn_npcs(&MapNpcs { npcs }), 2);
    let off = engine
        .world
        .entity_by_gid(START_NPC_ID)
        .expect("disabled warp");
    assert_eq!(
        engine.world.npcs.get(off).map(|n| n.state),
        Some(NpcState::Disabled)
    );
    assert!(engine.world.warps.contains(off));
    assert!(!engine.world.in_grid[off.index()], "disabled is never sent");
    let nospan = engine
        .world
        .entity_by_gid(START_NPC_ID + 1)
        .expect("spanless warp");
    assert!(!engine.world.warps.contains(nospan));
}
