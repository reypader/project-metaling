use std::path::Path;

use crate::yaml::Schema;
use crate::{DbLoadError, NpcSpawn};

/// `Header.Type` of an NPC spawn file.
pub const NPC_SPAWN_DB_TYPE: &str = "NPC_SPAWN_DB";
/// `Header.Version` this crate reads and writes.
pub const NPC_SPAWN_DB_VERSION: u32 = 1;

/// All NPCs of one map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapNpcs {
    pub npcs: Vec<NpcSpawn>,
}

const SCHEMA: Schema = Schema {
    db_type: NPC_SPAWN_DB_TYPE,
    version: NPC_SPAWN_DB_VERSION,
};

impl MapNpcs {
    /// Parses an `NPC_SPAWN_DB` file. `path` is only a label for errors.
    pub fn from_yaml_str(path: &str, text: &str) -> Result<Self, DbLoadError> {
        SCHEMA.parse(path, text).map(|npcs| Self { npcs })
    }

    /// Reads and parses one map's file. A missing file is `Ok(None)` (the map has no NPCs).
    pub fn load_yaml(path: &Path) -> Result<Option<Self>, DbLoadError> {
        Ok(SCHEMA.load(path)?.map(|npcs| Self { npcs }))
    }

    /// Serializes with the current header (no leading comments; the importer adds those).
    pub fn to_yaml_string(&self) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(&self.npcs)
    }
}
