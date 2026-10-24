//! The mob registry (`MOB_DATA` YAML), produced offline by `tools/mob-import` from rAthena's mob
//! database. Drops are resolved to item ids. Level, HP and walk speed are imported but not wired
//! yet.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ro_core::ItemId;
use serde::{Deserialize, Serialize};

use crate::yaml::Schema;
use crate::{DbLoadError, MobId};

/// `Header.Type` of the mob file.
pub const MOB_DATA_TYPE: &str = "MOB_DATA";
/// `Header.Version` this crate reads and writes.
pub const MOB_DATA_VERSION: u32 = 1;
/// The file of a mob data directory.
pub const MOB_DATA_FILE: &str = "mob_db.yml";
/// rAthena `MAX_MOB_DROP`.
pub const MAX_MOB_DROPS: usize = 10;
/// rAthena `MAX_MVP_DROP`.
pub const MAX_MVP_DROPS: usize = 3;
/// Drop rates are per 10000.
pub const MAX_DROP_RATE: u16 = 10_000;

const SCHEMA: Schema = Schema {
    db_type: MOB_DATA_TYPE,
    version: MOB_DATA_VERSION,
};

/// rAthena mob `Class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobClass {
    Normal,
    Boss,
    Guardian,
    Battlefield,
    Event,
}

/// One drop entry: `item` drops when `roll % 10000 < rate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct MobDrop {
    pub item: ItemId,
    /// `1..=10000`.
    pub rate: u16,
}

/// One mob database entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct MobData {
    pub id: MobId,
    pub aegis_name: String,
    pub name: String,
    pub level: u16,
    pub hp: u32,
    /// ms per cell.
    pub walk_speed: u16,
    pub class: MobClass,
    pub mvp: bool,
    /// At most `MAX_MOB_DROPS`, in db order.
    pub drops: Vec<MobDrop>,
    /// At most `MAX_MVP_DROPS`. Unused until MVP rewards.
    pub mvp_drops: Vec<MobDrop>,
}

impl MobData {
    /// rAthena `bosstype`: 2 for an MVP, 1 for `Class: Boss`, else 0.
    pub fn boss_type(&self) -> u8 {
        if self.mvp {
            2
        } else if self.class == MobClass::Boss {
            1
        } else {
            0
        }
    }
}

/// The global mob registry.
#[derive(Debug, Clone, Default)]
pub struct MobDb {
    /// Sorted by id.
    mobs: Vec<MobData>,
    by_id: HashMap<MobId, usize>,
}

impl MobDb {
    /// Parses one `MOB_DATA` file. `path` is only a label for errors.
    pub fn parse_file(path: &str, text: &str) -> Result<Vec<MobData>, DbLoadError> {
        SCHEMA.parse(path, text)
    }

    /// Serializes with the current header (no leading comments; the importer adds those).
    pub fn file_to_yaml(mobs: &[MobData]) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(mobs)
    }

    /// Builds the registry. Ids must be valid mob ids and unique, aegis names unique
    /// (case-insensitive), drop lists within their limits and rates in `1..=10000`.
    pub fn from_mobs(path: &str, mut mobs: Vec<MobData>) -> Result<Self, DbLoadError> {
        mobs.sort_by_key(|mob| mob.id);
        let mut by_id = HashMap::with_capacity(mobs.len());
        let mut aegis = HashSet::with_capacity(mobs.len());
        for (i, mob) in mobs.iter().enumerate() {
            if by_id.insert(mob.id, i).is_some() {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: mob.id.0,
                });
            }
            check_mob(mob).map_err(|message| DbLoadError::Schema {
                path: path.to_string(),
                message: format!("mob {}: {message}", mob.id.0),
            })?;
            if !aegis.insert(mob.aegis_name.to_lowercase()) {
                return Err(DbLoadError::Schema {
                    path: path.to_string(),
                    message: format!("duplicate AegisName '{}'", mob.aegis_name),
                });
            }
        }
        Ok(Self { mobs, by_id })
    }

    /// Loads `MOB_DATA_FILE` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let path = dir.join(MOB_DATA_FILE);
        let label = path.display().to_string();
        let Some(mobs) = SCHEMA.load(&path)? else {
            return Err(DbLoadError::Io {
                path: label,
                message: "file not found".into(),
            });
        };
        Self::from_mobs(&label, mobs)
    }

    pub fn get(&self, id: MobId) -> Option<&MobData> {
        self.by_id.get(&id).map(|&i| &self.mobs[i])
    }

    /// Mobs in id order.
    pub fn iter(&self) -> impl Iterator<Item = &MobData> {
        self.mobs.iter()
    }

    pub fn len(&self) -> usize {
        self.mobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mobs.is_empty()
    }
}

fn check_mob(mob: &MobData) -> Result<(), String> {
    if !mob.id.is_valid() {
        return Err("id outside the mob id ranges".into());
    }
    if mob.drops.len() > MAX_MOB_DROPS {
        return Err(format!("{} drops (max {MAX_MOB_DROPS})", mob.drops.len()));
    }
    if mob.mvp_drops.len() > MAX_MVP_DROPS {
        return Err(format!(
            "{} MVP drops (max {MAX_MVP_DROPS})",
            mob.mvp_drops.len()
        ));
    }
    if let Some(bad) = mob
        .drops
        .iter()
        .chain(&mob.mvp_drops)
        .find(|d| d.rate == 0 || d.rate > MAX_DROP_RATE)
    {
        return Err(format!("drop rate {} outside 1..=10000", bad.rate));
    }
    Ok(())
}
