//! Map-data loaders: an injected grid, NPCs, mobs and the shipped registries.

use std::collections::HashMap;
use std::sync::Arc;

use map_server::{MapData, MapDataLoader};
use ro_db::{CellType, MapGrid, MapMobSpawns, MapNpcs, MobId, MobSpawn};

/// Loader handing every map the same 200x200 grid with `walls`, no NPCs and no mobs.
pub fn grid_loader(walls: &[(u16, u16)]) -> MapDataLoader {
    data_loader(walls, MapNpcs::default(), MapMobSpawns::default())
}

/// Loader handing every map the same 200x200 grid with `walls` and the same `npcs` and `mobs`.
pub fn data_loader(walls: &[(u16, u16)], npcs: MapNpcs, mobs: MapMobSpawns) -> MapDataLoader {
    let mut grid = MapGrid::all_walkable(200, 200).expect("valid dims");
    for &(x, y) in walls {
        assert!(grid.set(x, y, CellType::Wall));
    }
    let data = MapData {
        grid: Arc::new(grid),
        npcs: Arc::new(npcs),
        mobs: Arc::new(mobs),
        registries: test_registries(),
    };
    Arc::new(move |_map: &str| data.clone())
}

/// Loader handing every map a 200x200 all-walkable grid, with the NPCs listed for it (none for
/// maps not listed).
pub fn maps_loader(maps: Vec<(&'static str, MapNpcs)>) -> MapDataLoader {
    let grid = Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims"));
    let maps: HashMap<&'static str, Arc<MapNpcs>> = maps
        .into_iter()
        .map(|(map, npcs)| (map, Arc::new(npcs)))
        .collect();
    let registries = test_registries();
    Arc::new(move |map: &str| MapData {
        grid: Arc::clone(&grid),
        npcs: maps.get(map).cloned().unwrap_or_default(),
        mobs: Arc::default(),
        registries: registries.clone(),
    })
}

/// The shipped item, mob, job, refine and skill registries (read from `crates/ro-db/data`, like the
/// server does) and the default battle config.
pub fn test_registries() -> ro_engine::Registries {
    static REGISTRIES: std::sync::OnceLock<ro_engine::Registries> = std::sync::OnceLock::new();
    REGISTRIES
        .get_or_init(|| {
            let data =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ro-db/data");
            let (items, mobs, jobs, refine, skills) = (
                data.join("item"),
                data.join("mob"),
                data.join("job"),
                data.join("refine"),
                data.join("skill"),
            );
            map_server::load_registries(map_server::RegistryPaths {
                items: Some(&items),
                mobs: Some(&mobs),
                jobs: Some(&jobs),
                refine: Some(&refine),
                skills: Some(&skills),
                battle: None,
            })
            .expect("shipped registries load")
        })
        .clone()
}

/// A `.gat` file of `width` x `height` walkable cells.
pub fn gat_file(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"GRAT".to_vec();
    b.extend_from_slice(&[1, 2]);
    b.extend_from_slice(&width.to_le_bytes());
    b.extend_from_slice(&height.to_le_bytes());
    for _ in 0..width * height {
        b.extend_from_slice(&[0u8; 20]);
    }
    b
}
/// One Poring (shipped 1002: 55 HP, 150/40 exp) next to the seeded character at (100, 100).
pub fn poring_loader() -> map_server::MapDataLoader {
    let spawn = MobSpawn {
        name: "Poring".into(),
        mob_id: MobId(1002),
        x: 101,
        y: 100,
        span: (0, 0),
        amount: 1,
        delay1: 5000,
        delay2: 0,
        level: None,
        boss: false,
        event: None,
        source: "test:poring".into(),
    };
    data_loader(
        &[],
        MapNpcs::default(),
        MapMobSpawns {
            spawns: vec![spawn],
        },
    )
}
