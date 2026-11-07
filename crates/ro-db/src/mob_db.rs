//! The mob registry (`MOB_DATA` YAML), produced offline by `tools/mob-import` from rAthena's mob
//! database. Drops are resolved to item ids. Combat fields (stats, motions, size, race, element,
//! modes) are for the damage formulas and mob status.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::yaml::Schema;
use crate::{DbLoadError, MobData, MobId, MAX_ELEMENT_LEVEL};

/// `Header.Type` of the mob file.
pub const MOB_DATA_TYPE: &str = "MOB_DATA";
/// `Header.Version` this crate reads and writes.
pub const MOB_DATA_VERSION: u32 = 2;
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
    if mob.attack_motion == 0 {
        return Err("AttackMotion must be at least 1".into());
    }
    if mob.attack_delay < mob.attack_motion {
        return Err(format!(
            "AttackDelay {} is below AttackMotion {}",
            mob.attack_delay, mob.attack_motion
        ));
    }
    if !(1..=MAX_ELEMENT_LEVEL).contains(&mob.element_level) {
        return Err(format!(
            "ElementLevel {} outside 1..={MAX_ELEMENT_LEVEL}",
            mob.element_level
        ));
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
