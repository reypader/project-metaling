use std::collections::HashMap;
use std::path::Path;

use crate::yaml::{load_required, schema_error, Schema};
use crate::{DbLoadError, JobTree, SkillData, SkillDb, TreeEntry};

pub const SKILL_TREE_DATA_TYPE: &str = "SKILL_TREE_DATA";
pub const SKILL_TREE_DATA_VERSION: u32 = 1;
pub const SKILL_TREE_DATA_FILE: &str = "skill_tree.yml";
const TREE_SCHEMA: Schema = Schema {
    db_type: SKILL_TREE_DATA_TYPE,
    version: SKILL_TREE_DATA_VERSION,
};

/// Every job's skill tree, keyed by job id.
#[derive(Debug, Clone, Default)]
pub struct SkillTreeDb {
    trees: HashMap<u16, Vec<TreeEntry>>,
}

impl SkillTreeDb {
    /// Parses `skill_tree.yml`. `path` is only a label for errors.
    pub fn parse(path: &str, text: &str) -> Result<Vec<JobTree>, DbLoadError> {
        TREE_SCHEMA.parse(path, text)
    }

    /// Serializes trees with the current header (the importer adds the leading comments).
    pub fn to_yaml(trees: &[JobTree]) -> Result<String, DbLoadError> {
        TREE_SCHEMA.to_yaml(trees)
    }

    /// Builds the registry. Job ids are unique; each tree is sorted by skill id with no
    /// repeats; every skill and requirement names a skill in `skills`, `MaxLevel` is in
    /// `1..=` the skill's, and a requirement level is in `1..=` the required skill's.
    pub fn from_rows(
        path: &str,
        rows: Vec<JobTree>,
        skills: &SkillDb,
    ) -> Result<Self, DbLoadError> {
        let mut trees = HashMap::with_capacity(rows.len());
        for row in rows {
            let label = format!("job {} ({})", row.job, row.name);
            for (index, entry) in row.tree.iter().enumerate() {
                if index > 0 && row.tree[index - 1].skill >= entry.skill {
                    return Err(schema_error(
                        path,
                        format!(
                            "{label}: skills not in ascending id order at {}",
                            entry.skill
                        ),
                    ));
                }
                let max = known(path, &label, skills, entry.skill)?.max_level;
                if !(1..=max).contains(&entry.max_level) {
                    return Err(schema_error(
                        path,
                        format!("{label}: skill {} MaxLevel must be 1..={max}", entry.skill),
                    ));
                }
                for need in &entry.requires {
                    let max = known(path, &label, skills, need.skill)?.max_level;
                    if !(1..=max).contains(&need.level) {
                        return Err(schema_error(
                            path,
                            format!(
                                "{label}: skill {} requires skill {} at level {} (1..={max})",
                                entry.skill, need.skill, need.level
                            ),
                        ));
                    }
                }
            }
            if trees.insert(row.job, row.tree).is_some() {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: u32::from(row.job),
                });
            }
        }
        Ok(Self { trees })
    }

    /// Loads `SKILL_TREE_DATA_FILE` from `dir`, checked against `skills`. A missing file is an
    /// error.
    pub fn load_dir(dir: &Path, skills: &SkillDb) -> Result<Self, DbLoadError> {
        let path = dir.join(SKILL_TREE_DATA_FILE);
        let rows = load_required(&TREE_SCHEMA, &path)?;
        Self::from_rows(&path.display().to_string(), rows, skills)
    }

    /// `job`'s tree, sorted by skill id; empty for a job with no tree.
    pub fn tree(&self, job: u16) -> &[TreeEntry] {
        self.trees.get(&job).map_or(&[], Vec::as_slice)
    }

    /// `skill`'s entry in `job`'s tree.
    pub fn entry(&self, job: u16, skill: u16) -> Option<&TreeEntry> {
        let tree = self.tree(job);
        tree.binary_search_by_key(&skill, |entry| entry.skill)
            .ok()
            .map(|index| &tree[index])
    }

    /// The most `job` can learn of `skill` (rAthena `skill_tree_get_max`): the tree entry's
    /// max, else the skill's own, else 0 for an unknown skill.
    pub fn max_level(&self, job: u16, skill: u16, skills: &SkillDb) -> u8 {
        match self.entry(job, skill) {
            Some(entry) => entry.max_level,
            None => skills.get(skill).map_or(0, |data| data.max_level),
        }
    }

    /// Number of jobs with a tree.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }
}

fn known<'a>(
    path: &str,
    label: &str,
    skills: &'a SkillDb,
    id: u16,
) -> Result<&'a SkillData, DbLoadError> {
    skills
        .get(id)
        .ok_or_else(|| schema_error(path, format!("{label}: unknown skill {id}")))
}
