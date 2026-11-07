//! What an import produces: the converted skills and trees, and a report.

use std::collections::BTreeSet;

use ro_db::{JobTree, SkillData};

#[derive(Debug, Default)]
pub struct Converted {
    /// Every skill a tree lists or requires, sorted by id.
    pub skills: Vec<SkillData>,
    /// Sorted by job id.
    pub trees: Vec<JobTree>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    pub skills_read: usize,
    pub jobs_read: usize,
    /// Tree job names with no id in `job_import::JOB_IDS` (mounted and dummy variants).
    pub skipped_jobs: BTreeSet<String>,
    pub warnings: Vec<String>,
}
