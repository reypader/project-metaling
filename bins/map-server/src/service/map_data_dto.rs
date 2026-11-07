//! Static data a map world starts with, and where the registries are read from.

use std::{path::Path, sync::Arc};

use ro_db::{MapGrid, MapMobSpawns, MapNpcs};
use ro_engine::Registries;

/// Static data a map world starts with.
#[derive(Clone)]
pub struct MapData {
    pub grid: Arc<MapGrid>,
    pub npcs: Arc<MapNpcs>,
    pub mobs: Arc<MapMobSpawns>,
    pub registries: Registries,
}

/// Builds a map's static data from its name (`prontera.gat`).
pub type MapDataLoader = Arc<dyn Fn(&str) -> MapData + Send + Sync>;

/// Where `load_registries` reads from (`MapConfig::registry_paths`). Every source is optional
/// here; `main` refuses to start without the job data.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryPaths<'a> {
    pub items: Option<&'a Path>,
    pub mobs: Option<&'a Path>,
    pub jobs: Option<&'a Path>,
    pub refine: Option<&'a Path>,
    pub skills: Option<&'a Path>,
    pub battle: Option<&'a Path>,
}
