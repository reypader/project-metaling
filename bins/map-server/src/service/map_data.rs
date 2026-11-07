//! Loading a map's static data: walkability, NPCs, mob spawns, registries and battle config.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use anyhow::{anyhow, Context};
use ro_core::{bare_map_name, gat_map_name};
use ro_db::{
    ItemDb, JobDb, MapGrid, MapMobSpawns, MapNpcs, MobDb, NpcKind, RefineDb, SkillDb, SkillTreeDb,
};
use ro_engine::{Registries, SkillScripts};
use ro_mechanics::BattleConfig;
use tracing::{error, info, warn};

use super::{is_valid_map_name, MapData, MapDataLoader, RegistryPaths};

/// Side of the all-walkable grid used when a map's `.gat` is unavailable (BIN-CFG-01).
pub const FALLBACK_MAP_SIZE: u16 = 512;

/// Loads `<dir>/<map_name>` as a `.gat`. A missing directory, missing file, or bad file logs a
/// warning and falls back to an all-walkable `FALLBACK_MAP_SIZE` square grid.
pub fn load_walkability(map_data_dir: Option<&Path>, map_name: &str) -> Arc<MapGrid> {
    if let Some(dir) = map_data_dir {
        match MapGrid::load_gat(&dir.join(map_name)) {
            Ok(grid) => return Arc::new(grid),
            Err(err) => warn!("Map '{}': {}; using fallback grid", map_name, err),
        }
    } else {
        warn!(
            "Map '{}': MAP_DATA_DIR not set; using fallback grid",
            map_name
        );
    }
    fallback_grid()
}

/// One fallback grid shared by every map without a `.gat`; `MapInstance::set_cell` copies
/// it before changing a cell, so maps never see each other's changes.
fn fallback_grid() -> Arc<MapGrid> {
    static FALLBACK: OnceLock<Arc<MapGrid>> = OnceLock::new();
    let grid = FALLBACK.get_or_init(|| {
        let grid =
            MapGrid::all_walkable(FALLBACK_MAP_SIZE, FALLBACK_MAP_SIZE).unwrap_or_else(|err| {
                // Unreachable: the fallback dimensions are non-zero constants.
                error!("fallback grid: {}", err);
                MapGrid::blocked()
            });
        Arc::new(grid)
    });
    Arc::clone(grid)
}

/// Loads `<dir>/<map>.yml` (map name without `.gat`). No directory or no file means no NPCs;
/// a bad file logs a warning and also means no NPCs (BIN-NPC-01).
pub fn load_npcs(npc_data_dir: Option<&Path>, map_name: &str) -> Arc<MapNpcs> {
    let Some(dir) = npc_data_dir else {
        return Arc::new(MapNpcs::default());
    };
    let stem = bare_map_name(map_name);
    match MapNpcs::load_yaml(&dir.join(format!("{stem}.yml"))) {
        Ok(Some(npcs)) => Arc::new(npcs),
        Ok(None) => Arc::new(MapNpcs::default()),
        Err(err) => {
            warn!("Map '{}': {}; spawning no NPCs", map_name, err);
            Arc::new(MapNpcs::default())
        }
    }
}

/// Loads `<dir>/<map>.yml` mob spawn lines (map name without `.gat`). No directory or no file
/// means no mobs; a bad file logs a warning and also means no mobs (BIN-MOB-01).
pub fn load_mob_spawns(mob_spawn_data_dir: Option<&Path>, map_name: &str) -> Arc<MapMobSpawns> {
    let Some(dir) = mob_spawn_data_dir else {
        return Arc::new(MapMobSpawns::default());
    };
    let stem = bare_map_name(map_name);
    match MapMobSpawns::load_yaml(&dir.join(format!("{stem}.yml"))) {
        Ok(Some(mobs)) => Arc::new(mobs),
        Ok(None) => Arc::new(MapMobSpawns::default()),
        Err(err) => {
            warn!("Map '{}': {}; spawning no mobs", map_name, err);
            Arc::new(MapMobSpawns::default())
        }
    }
}

/// Loads the item, mob, job, refine and skill registries and the battle config (specs
/// `specs/items/spec.md` §0, `specs/stats/spec.md` §1). An unset source warns and gives an
/// empty registry (the battle config: rAthena's shipped defaults); a set one that is missing or
/// bad is an error naming the path (startup fails).
pub fn load_registries(paths: RegistryPaths<'_>) -> anyhow::Result<Registries> {
    let items = match paths.items {
        Some(dir) => ItemDb::load_dir(dir)?,
        None => {
            warn!("ITEM_DATA_DIR not set; the item registry is empty");
            ItemDb::default()
        }
    };
    let mobs = match paths.mobs {
        Some(dir) => MobDb::load_dir(dir)?,
        None => {
            warn!("MOB_DATA_DIR not set; the mob registry is empty");
            MobDb::default()
        }
    };
    let jobs = match paths.jobs {
        Some(dir) => JobDb::load_dir(dir)?,
        None => {
            warn!("JOB_DATA_DIR not set; the job registry is empty");
            JobDb::default()
        }
    };
    let refine = match paths.refine {
        Some(dir) => RefineDb::load_dir(dir)?,
        None => {
            warn!("REFINE_DATA_DIR not set; refines give no bonus");
            RefineDb::default()
        }
    };
    let (skills, skill_tree) = match paths.skills {
        Some(dir) => {
            let skills = SkillDb::load_dir(dir)?;
            let tree = SkillTreeDb::load_dir(dir, &skills)?;
            (skills, tree)
        }
        None => {
            warn!("SKILL_DATA_DIR not set; the skill registry is empty");
            (SkillDb::default(), SkillTreeDb::default())
        }
    };
    let battle = load_battle_config(paths.battle)?;
    info!(
        "Registries: {} items, {} mobs, {} jobs, {} skills",
        items.len(),
        mobs.len(),
        jobs.len(),
        skills.len()
    );
    Ok(Registries {
        items: Arc::new(items),
        mobs: Arc::new(mobs),
        jobs: Arc::new(jobs),
        refine: Arc::new(refine),
        skills: Arc::new(skills),
        skill_tree: Arc::new(skill_tree),
        battle: Arc::new(battle),
        scripts: SkillScripts::builtin()?,
    })
}

/// Reads our battle config YAML (`config/battle.yml`). Every key is optional; an unknown key or
/// an out-of-range value is an error naming the file. Unset: rAthena's shipped defaults.
pub fn load_battle_config(path: Option<&Path>) -> anyhow::Result<BattleConfig> {
    let Some(path) = path else {
        info!("BATTLE_CONFIG not set; using rAthena's shipped defaults");
        return Ok(BattleConfig::default());
    };
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading BATTLE_CONFIG {}", path.display()))?;
    let config: BattleConfig = serde_saphyr::from_str(&text)
        .with_context(|| format!("parsing BATTLE_CONFIG {}", path.display()))?;
    config
        .validate()
        .map_err(|reason| anyhow!("BATTLE_CONFIG {}: {reason}", path.display()))?;
    Ok(config)
}

/// Loader reading `.gat` files from `map_data_dir`, NPC lists from `npc_data_dir` and mob
/// spawn lists from `mob_spawn_data_dir`, and handing every map the shared `registries`. Warps
/// that can't be taken lose their trigger before the map spawns its NPCs.
pub fn map_data_loader(
    map_data_dir: Option<PathBuf>,
    npc_data_dir: Option<PathBuf>,
    mob_spawn_data_dir: Option<PathBuf>,
    registries: Registries,
) -> MapDataLoader {
    Arc::new(move |map_name: &str| {
        let mut npcs = Arc::unwrap_or_clone(load_npcs(npc_data_dir.as_deref(), map_name));
        disable_unreachable_warps(&mut npcs, map_data_dir.as_deref(), map_name);
        MapData {
            grid: load_walkability(map_data_dir.as_deref(), map_name),
            npcs: Arc::new(npcs),
            mobs: load_mob_spawns(mob_spawn_data_dir.as_deref(), map_name),
            registries: registries.clone(),
        }
    })
}

/// Clears the trigger of each warp whose destination is invalid or an instance (`@`) map, or,
/// with `map_data_dir` set, has no `.gat` file there (BIN-WARP-04). Warns once per warp.
/// Without `map_data_dir` every map uses the fallback grid, so such warps stay.
fn disable_unreachable_warps(npcs: &mut MapNpcs, map_data_dir: Option<&Path>, map_name: &str) {
    for npc in &mut npcs.npcs {
        let NpcKind::Warp(dest) = &npc.kind else {
            continue;
        };
        if npc.trigger.is_none() {
            continue;
        }
        let dest_map = gat_map_name(&dest.map);
        let reason = if !is_valid_map_name(&dest_map) {
            "invalid or instance destination"
        } else if dest_map.len() > MAX_WIRE_MAP_NAME {
            "destination name too long for 0x0091"
        } else if map_data_dir.is_some_and(|dir| !dir.join(&dest_map).is_file()) {
            "destination .gat not found"
        } else {
            continue;
        };
        warn!(
            "Map '{}': warp '{}' ({}) to '{}' is inert: {}",
            map_name, npc.name, npc.source, dest.map, reason
        );
        npc.trigger = None;
    }
}

/// Longest map name `0x0091` carries: a 16-byte field with a NUL.
const MAX_WIRE_MAP_NAME: usize = 15;
