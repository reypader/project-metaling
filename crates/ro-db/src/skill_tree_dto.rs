//! One job's resolved skill tree as stored in `skill_tree.yml`.

use serde::{Deserialize, Serialize};

use crate::yaml::{flow_list, is_zero};

/// A prerequisite: `skill` learned at `level` or higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillRequirement {
    pub skill: u16,
    pub level: u8,
}

/// One skill in a job's tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct TreeEntry {
    pub skill: u16,
    /// The most this job can learn (at most the skill's `max_level`).
    pub max_level: u8,
    /// Base level needed to see the skill (0 = none).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub base_level: u16,
    /// Job level needed to see the skill (0 = none).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub job_level: u16,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub requires: Vec<SkillRequirement>,
}

/// One job's resolved tree as stored in `skill_tree.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobTree {
    /// rAthena `e_job` id.
    pub job: u16,
    /// The job's rAthena name, for readers of the file.
    pub name: String,
    /// Sorted by skill id.
    #[serde(default)]
    pub tree: Vec<TreeEntry>,
}
