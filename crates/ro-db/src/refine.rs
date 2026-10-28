//! Refine bonus table (`REFINE_DATA`, from `tools/refine-import`): per group and item level, the
//! cumulative `Bonus` / `RandomBonus` (1/100 units) at each refine level 1..=20.

use std::path::Path;

use serde::{Deserialize, Serialize, Serializer};

use crate::yaml::Schema;
use crate::DbLoadError;

pub const REFINE_DATA_TYPE: &str = "REFINE_DATA";
pub const REFINE_DATA_VERSION: u32 = 1;
pub const REFINE_DATA_FILE: &str = "refine.yml";
/// Highest refine level (renewal `MAX_REFINE`); higher refines read as this.
pub const MAX_REFINE: u8 = 20;

const SCHEMA: Schema = Schema {
    db_type: REFINE_DATA_TYPE,
    version: REFINE_DATA_VERSION,
};

/// Which table an item uses: weapons by weapon level, armor by armor level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RefineGroup {
    Armor,
    Weapon,
}

/// One `(group, level)` row as stored in `refine.yml`. Entry `i` is the total at refine `i + 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct RefineRow {
    pub group: RefineGroup,
    pub level: u8,
    #[serde(serialize_with = "flow_list")]
    pub bonus: Vec<u32>,
    /// Empty when every entry is 0.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub random_bonus: Vec<u32>,
}

/// The bonus at one refine level, in 1/100 units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefineBonus {
    pub bonus: u32,
    pub random_bonus: u32,
}

/// The refine bonus registry.
#[derive(Debug, Clone, Default)]
pub struct RefineDb {
    rows: Vec<RefineRow>,
}

impl RefineDb {
    /// Parses `refine.yml`. `path` is only a label for errors.
    pub fn parse(path: &str, text: &str) -> Result<Vec<RefineRow>, DbLoadError> {
        SCHEMA.parse(path, text)
    }

    /// Serializes rows with the current header (the importer adds the leading comments).
    pub fn to_yaml(rows: &[RefineRow]) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(rows)
    }

    /// Builds the registry. Each `(group, level)` appears once, `Bonus` has `MAX_REFINE`
    /// entries and `RandomBonus` has none or `MAX_REFINE`.
    pub fn from_rows(path: &str, rows: Vec<RefineRow>) -> Result<Self, DbLoadError> {
        let max = usize::from(MAX_REFINE);
        for (i, row) in rows.iter().enumerate() {
            let label = format!("{:?} level {}", row.group, row.level);
            if rows[..i]
                .iter()
                .any(|r| r.group == row.group && r.level == row.level)
            {
                return Err(schema_error(path, format!("{label} appears twice")));
            }
            if row.bonus.len() != max
                || !(row.random_bonus.is_empty() || row.random_bonus.len() == max)
            {
                return Err(schema_error(
                    path,
                    format!("{label}: Bonus and RandomBonus need {max} entries"),
                ));
            }
        }
        Ok(Self { rows })
    }

    /// Loads `REFINE_DATA_FILE` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let path = dir.join(REFINE_DATA_FILE);
        let rows = SCHEMA.load(&path)?.ok_or_else(|| DbLoadError::Io {
            path: path.display().to_string(),
            message: "file not found".into(),
        })?;
        Self::from_rows(&path.display().to_string(), rows)
    }

    /// The bonus of an item of `group` and item `level` at `refine` (clamped to `MAX_REFINE`).
    /// Zero at refine 0 or when the table has no row for that level.
    pub fn bonus(&self, group: RefineGroup, level: u8, refine: u8) -> RefineBonus {
        let Some(index) = usize::from(refine.min(MAX_REFINE)).checked_sub(1) else {
            return RefineBonus::default();
        };
        let Some(row) = self
            .rows
            .iter()
            .find(|row| row.group == group && row.level == level)
        else {
            return RefineBonus::default();
        };
        RefineBonus {
            bonus: row.bonus.get(index).copied().unwrap_or(0),
            random_bonus: row.random_bonus.get(index).copied().unwrap_or(0),
        }
    }
}

fn schema_error(path: &str, message: String) -> DbLoadError {
    DbLoadError::Schema {
        path: path.to_string(),
        message,
    }
}

fn flow_list<S: Serializer>(values: &[u32], serializer: S) -> Result<S::Ok, S::Error> {
    serde_saphyr::FlowSeq(values).serialize(serializer)
}
