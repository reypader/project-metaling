//! Skill registry (`SKILL_DATA`) and per-job skill trees (`SKILL_TREE_DATA`), from
//! `tools/skill-import` (specs `specs/skills/spec.md` §0-§1, `specs/effects/spec.md` §0.1, §1).
//! Per skill: the target type, the flags that decide learnability and range bonuses, and the
//! per-level casting data (costs, range, cast and delay times, element, requirements). The
//! trees come with rAthena's inheritance already resolved, so each job lists every skill it can
//! learn.

use std::collections::HashMap;
use std::path::Path;

use crate::yaml::{load_required, schema_error, Schema};
use crate::{DbLoadError, SkillData};

pub const SKILL_DATA_TYPE: &str = "SKILL_DATA";
/// 2 added the casting fields (`specs/effects`).
pub const SKILL_DATA_VERSION: u32 = 2;
pub const SKILL_DATA_FILE: &str = "skill_db.yml";
/// rAthena `MAX_SKILL_LEVEL`.
pub const MAX_SKILL_LEVEL: u8 = 13;

const SKILL_SCHEMA: Schema = Schema {
    db_type: SKILL_DATA_TYPE,
    version: SKILL_DATA_VERSION,
};

/// The skill registry, sorted by id.
#[derive(Debug, Clone, Default)]
pub struct SkillDb {
    skills: Vec<SkillData>,
    by_name: HashMap<String, usize>,
}

impl SkillDb {
    /// Parses `skill_db.yml`. `path` is only a label for errors.
    pub fn parse(path: &str, text: &str) -> Result<Vec<SkillData>, DbLoadError> {
        SKILL_SCHEMA.parse(path, text)
    }

    /// Serializes skills with the current header (the importer adds the leading comments).
    pub fn to_yaml(skills: &[SkillData]) -> Result<String, DbLoadError> {
        SKILL_SCHEMA.to_yaml(skills)
    }

    /// Builds the registry. Ids and names (case-insensitive) are unique, `max_level` is in
    /// `1..=MAX_SKILL_LEVEL`, and the per-level lists are empty or have `max_level` entries.
    pub fn from_rows(path: &str, mut skills: Vec<SkillData>) -> Result<Self, DbLoadError> {
        skills.sort_by_key(|skill| skill.id);
        let mut by_name = HashMap::with_capacity(skills.len());
        for (index, skill) in skills.iter().enumerate() {
            let label = format!("skill {} ({})", skill.id, skill.name);
            if index > 0 && skills[index - 1].id == skill.id {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: u32::from(skill.id),
                });
            }
            if by_name.insert(skill.name.to_lowercase(), index).is_some() {
                return Err(schema_error(path, format!("{label}: duplicate Name")));
            }
            if !(1..=MAX_SKILL_LEVEL).contains(&skill.max_level) {
                return Err(schema_error(
                    path,
                    format!("{label}: MaxLevel must be 1..={MAX_SKILL_LEVEL}"),
                ));
            }
            let levels = usize::from(skill.max_level);
            if let Some(key) = skill
                .list_lengths()
                .into_iter()
                .find_map(|(key, len)| (len != 0 && len != levels).then_some(key))
            {
                return Err(schema_error(
                    path,
                    format!("{label}: {key} needs {levels} entries or none"),
                ));
            }
            if let Some(cost) = skill
                .item_cost
                .iter()
                .find(|cost| cost.level.is_some_and(|l| l == 0 || l > skill.max_level))
            {
                return Err(schema_error(
                    path,
                    format!(
                        "{label}: ItemCost {} level out of 1..={}",
                        cost.item, skill.max_level
                    ),
                ));
            }
        }
        Ok(Self { skills, by_name })
    }

    /// Loads `SKILL_DATA_FILE` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let path = dir.join(SKILL_DATA_FILE);
        let rows = load_required(&SKILL_SCHEMA, &path)?;
        Self::from_rows(&path.display().to_string(), rows)
    }

    pub fn get(&self, id: u16) -> Option<&SkillData> {
        self.skills
            .binary_search_by_key(&id, |skill| skill.id)
            .ok()
            .map(|index| &self.skills[index])
    }

    /// Case-insensitive aegis name lookup (`sm_bash`).
    pub fn by_name(&self, name: &str) -> Option<&SkillData> {
        self.by_name
            .get(&name.to_lowercase())
            .map(|&index| &self.skills[index])
    }

    /// Skills in id order.
    pub fn iter(&self) -> impl Iterator<Item = &SkillData> {
        self.skills.iter()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}
