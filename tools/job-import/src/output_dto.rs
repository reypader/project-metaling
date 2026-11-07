//! What an import produces: the converted jobs, tables and a report.

use std::collections::BTreeSet;

use ro_db::{ExpGroup, JobData, StatPointRow};

#[derive(Debug, Default)]
pub struct Converted {
    /// Sorted by id.
    pub jobs: Vec<JobData>,
    pub points: Vec<StatPointRow>,
    /// Jobs sharing one exp table, Base groups first, each ordered by its first job id.
    pub exp: Vec<ExpGroup>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Entries read, per job source.
    pub read: Vec<usize>,
    /// Job names with no id in `JOB_IDS` (mounted and dummy variants); skipped.
    pub unknown_jobs: BTreeSet<String>,
    /// `(job name, HP slots, SP slots)` filled by the fallback formula.
    pub fallback: Vec<(String, usize, usize)>,
    pub warnings: Vec<String>,
}
