use std::path::Path;

use crate::yaml::Schema;
use crate::{DbLoadError, MobSpawn};

/// `Header.Type` of a mob spawn file.
pub const MOB_SPAWN_DB_TYPE: &str = "MOB_SPAWN_DB";
/// `Header.Version` this crate reads and writes.
pub const MOB_SPAWN_DB_VERSION: u32 = 1;

/// All permanent spawn lines of one map, in rAthena load order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapMobSpawns {
    pub spawns: Vec<MobSpawn>,
}

const SCHEMA: Schema = Schema {
    db_type: MOB_SPAWN_DB_TYPE,
    version: MOB_SPAWN_DB_VERSION,
};

impl MapMobSpawns {
    /// Parses a `MOB_SPAWN_DB` file. `path` is only a label for errors.
    pub fn from_yaml_str(path: &str, text: &str) -> Result<Self, DbLoadError> {
        SCHEMA.parse(path, text).map(|spawns| Self { spawns })
    }

    /// Reads and parses one map's file. A missing file is `Ok(None)` (the map has no mobs).
    pub fn load_yaml(path: &Path) -> Result<Option<Self>, DbLoadError> {
        Ok(SCHEMA.load(path)?.map(|spawns| Self { spawns }))
    }

    /// Serializes with the current header (no leading comments; the importer adds those).
    pub fn to_yaml_string(&self) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(&self.spawns)
    }
}
