//! Job registry (`JOB_DATA`, from `tools/job-import`): per-job max weight, max levels, job-level
//! stat bonuses, ASPD bases and base HP/SP/AP tables, plus the cumulative status- and
//! trait-point table (`STATPOINT_DATA`) and the base- and job-exp tables (`JOB_EXP_DATA`).

use std::collections::HashMap;
use std::path::Path;

use crate::job::{clamped, DEFAULT_ASPD_BASE, WEAPON_TYPES};
use crate::yaml::{load_required, schema_error, Schema};
use crate::{DbLoadError, ExpGroup, ExpKind, Job, JobBonus, JobData, StatPointRow};

pub const JOB_DATA_TYPE: &str = "JOB_DATA";
pub const JOB_DATA_VERSION: u32 = 2;
pub const JOB_DATA_FILE: &str = "job_db.yml";
pub const STATPOINT_DATA_TYPE: &str = "STATPOINT_DATA";
pub const STATPOINT_DATA_VERSION: u32 = 2;
pub const STATPOINT_DATA_FILE: &str = "statpoint.yml";
pub const JOB_EXP_DATA_TYPE: &str = "JOB_EXP_DATA";
pub const JOB_EXP_DATA_VERSION: u32 = 1;
pub const JOB_EXP_DATA_FILE: &str = "job_exp.yml";

/// rAthena's next base exp at (or above) the max base level, or for a job with no table.
pub const NEXT_BASE_EXP_AT_MAX: u64 = 99_999_999;
/// The same for job exp.
pub const NEXT_JOB_EXP_AT_MAX: u64 = 999_999_999;

const JOB_SCHEMA: Schema = Schema {
    db_type: JOB_DATA_TYPE,
    version: JOB_DATA_VERSION,
};
const STATPOINT_SCHEMA: Schema = Schema {
    db_type: STATPOINT_DATA_TYPE,
    version: STATPOINT_DATA_VERSION,
};
const EXP_SCHEMA: Schema = Schema {
    db_type: JOB_EXP_DATA_TYPE,
    version: JOB_EXP_DATA_VERSION,
};

/// The global job registry and status-point table.
#[derive(Debug, Clone, Default)]
pub struct JobDb {
    /// Sorted by id.
    jobs: Vec<Job>,
    by_id: HashMap<u16, usize>,
    by_name: HashMap<String, usize>,
    /// Index `base_level - 1`.
    status_points: Vec<u32>,
    /// Index `base_level - 1`.
    trait_points: Vec<u32>,
    /// The shared exp tables that jobs point into.
    exp_tables: Vec<Vec<u64>>,
}

impl JobDb {
    /// Parses `job_db.yml`. `path` is only a label for errors.
    pub fn parse_jobs(path: &str, text: &str) -> Result<Vec<JobData>, DbLoadError> {
        JOB_SCHEMA.parse(path, text)
    }

    /// Parses `statpoint.yml`. `path` is only a label for errors.
    pub fn parse_status_points(path: &str, text: &str) -> Result<Vec<StatPointRow>, DbLoadError> {
        STATPOINT_SCHEMA.parse(path, text)
    }

    /// Serializes jobs with the current header (the importer adds the leading comments).
    pub fn jobs_to_yaml(jobs: &[JobData]) -> Result<String, DbLoadError> {
        JOB_SCHEMA.to_yaml(jobs)
    }

    pub fn status_points_to_yaml(rows: &[StatPointRow]) -> Result<String, DbLoadError> {
        STATPOINT_SCHEMA.to_yaml(rows)
    }

    /// Builds the registry. Ids and names (case-insensitive) must be unique, the HP/SP tables
    /// must have `max_base_level` entries (the AP table may be empty), bonus levels must be
    /// within `1..=max_job_level`, and the status-point rows must run 1, 2, 3, ... `path` is
    /// only a label for errors.
    pub fn from_parts(
        path: &str,
        mut jobs: Vec<JobData>,
        points: &[StatPointRow],
    ) -> Result<Self, DbLoadError> {
        jobs.sort_by_key(|job| job.id);
        let mut db = Self {
            jobs: Vec::with_capacity(jobs.len()),
            by_id: HashMap::with_capacity(jobs.len()),
            by_name: HashMap::with_capacity(jobs.len()),
            status_points: Vec::with_capacity(points.len()),
            trait_points: Vec::with_capacity(points.len()),
            exp_tables: Vec::new(),
        };
        for data in jobs {
            let index = db.jobs.len();
            if db.by_id.insert(data.id, index).is_some() {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: u32::from(data.id),
                });
            }
            if db.by_name.insert(data.name.to_lowercase(), index).is_some() {
                return Err(schema_error(
                    path,
                    format!("duplicate job Name '{}'", data.name),
                ));
            }
            db.jobs.push(build_job(path, data)?);
        }
        for (i, row) in points.iter().enumerate() {
            if usize::from(row.level) != i + 1 {
                return Err(schema_error(
                    path,
                    format!(
                        "status point row {}: expected Level {}, got {}",
                        i + 1,
                        i + 1,
                        row.level
                    ),
                ));
            }
            db.status_points.push(row.points);
            db.trait_points.push(row.trait_points);
        }
        if db.status_points.is_empty() {
            return Err(schema_error(path, "empty status point table".into()));
        }
        Ok(db)
    }

    /// Parses `job_exp.yml`. `path` is only a label for errors.
    pub fn parse_exp_tables(path: &str, text: &str) -> Result<Vec<ExpGroup>, DbLoadError> {
        EXP_SCHEMA.parse(path, text)
    }

    pub fn exp_tables_to_yaml(groups: &[ExpGroup]) -> Result<String, DbLoadError> {
        EXP_SCHEMA.to_yaml(groups)
    }

    /// Attaches the exp tables. Every job id must be known and in at most one group per kind.
    pub fn with_exp_tables(
        mut self,
        path: &str,
        groups: Vec<ExpGroup>,
    ) -> Result<Self, DbLoadError> {
        for group in groups {
            let table = self.exp_tables.len();
            for &id in &group.jobs {
                let Some(&index) = self.by_id.get(&id) else {
                    return Err(schema_error(
                        path,
                        format!("exp table names unknown job {id}"),
                    ));
                };
                let job = &mut self.jobs[index];
                let slot = match group.kind {
                    ExpKind::Base => &mut job.base_exp,
                    ExpKind::Job => &mut job.job_exp,
                };
                if slot.replace(table).is_some() {
                    return Err(schema_error(
                        path,
                        format!("job {id} is in two {:?} exp tables", group.kind),
                    ));
                }
            }
            self.exp_tables.push(group.exp);
        }
        Ok(self)
    }

    /// Loads `JOB_DATA_FILE`, `STATPOINT_DATA_FILE` and `JOB_EXP_DATA_FILE` from `dir`. A
    /// missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let jobs = load_required(&JOB_SCHEMA, &dir.join(JOB_DATA_FILE))?;
        let points: Vec<StatPointRow> =
            load_required(&STATPOINT_SCHEMA, &dir.join(STATPOINT_DATA_FILE))?;
        let exp_path = dir.join(JOB_EXP_DATA_FILE);
        let exp = load_required(&EXP_SCHEMA, &exp_path)?;
        Self::from_parts(&dir.display().to_string(), jobs, &points)?
            .with_exp_tables(&exp_path.display().to_string(), exp)
    }

    /// The base exp needed to go from `level` to `level + 1` (rAthena `pc_nextbaseexp`): 0 at
    /// level 0, `NEXT_BASE_EXP_AT_MAX` at or above the job's max level or with no table, else
    /// the table row (0 if missing).
    pub fn next_base_exp(&self, job: u16, level: u16) -> u64 {
        self.next_exp(job, level, ExpKind::Base)
    }

    /// The job-exp counterpart of `next_base_exp`, with `NEXT_JOB_EXP_AT_MAX`.
    pub fn next_job_exp(&self, job: u16, level: u16) -> u64 {
        self.next_exp(job, level, ExpKind::Job)
    }

    fn next_exp(&self, job: u16, level: u16, kind: ExpKind) -> u64 {
        let at_max = match kind {
            ExpKind::Base => NEXT_BASE_EXP_AT_MAX,
            ExpKind::Job => NEXT_JOB_EXP_AT_MAX,
        };
        if level == 0 {
            return 0;
        }
        let Some(job) = self.get(job) else {
            return at_max;
        };
        let (max, table) = match kind {
            ExpKind::Base => (job.max_base_level, job.base_exp),
            ExpKind::Job => (job.max_job_level, job.job_exp),
        };
        let Some(table) = table.and_then(|i| self.exp_tables.get(i)) else {
            return at_max;
        };
        if level >= max {
            return at_max;
        }
        table.get(usize::from(level - 1)).copied().unwrap_or(0)
    }

    pub fn get(&self, id: u16) -> Option<&Job> {
        self.by_id.get(&id).map(|&i| &self.jobs[i])
    }

    /// Case-insensitive name lookup (`lord_knight`).
    pub fn by_name(&self, name: &str) -> Option<&Job> {
        self.by_name
            .get(&name.to_lowercase())
            .map(|&i| &self.jobs[i])
    }

    /// Cumulative status points at `base_level` (clamped to the table).
    pub fn status_points(&self, base_level: u16) -> u32 {
        clamped(&self.status_points, base_level)
            .copied()
            .unwrap_or(0)
    }

    /// Cumulative trait points at `base_level` (clamped to the table; 0 below 201).
    pub fn trait_points(&self, base_level: u16) -> u32 {
        clamped(&self.trait_points, base_level)
            .copied()
            .unwrap_or(0)
    }

    /// Jobs in id order.
    pub fn iter(&self) -> impl Iterator<Item = &Job> {
        self.jobs.iter()
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

fn build_job(path: &str, data: JobData) -> Result<Job, DbLoadError> {
    let label = format!("job {} ({})", data.id, data.name);
    let max_base = usize::from(data.max_base_level);
    let max_job = usize::from(data.max_job_level);
    if max_base == 0 || max_job == 0 {
        return Err(schema_error(
            path,
            format!("{label}: max levels must be at least 1"),
        ));
    }
    for (table, values) in [
        ("BaseHp", &data.base_hp),
        ("BaseSp", &data.base_sp),
        ("BaseAp", &data.base_ap),
    ] {
        if values.len() != max_base && !(table == "BaseAp" && values.is_empty()) {
            return Err(schema_error(
                path,
                format!(
                    "{label}: {table} has {} entries, MaxBaseLevel is {max_base}",
                    values.len()
                ),
            ));
        }
    }
    let mut gains = vec![JobBonus::default(); max_job];
    for entry in &data.bonus_stats {
        let level = usize::from(entry.level);
        let Some(slot) = level.checked_sub(1).and_then(|i| gains.get_mut(i)) else {
            return Err(schema_error(
                path,
                format!("{label}: BonusStats level {level} outside 1..={max_job}"),
            ));
        };
        slot.add(&entry.gain);
    }
    let mut total = JobBonus::default();
    let bonus = gains
        .iter()
        .map(|gain| {
            total.add(gain);
            total
        })
        .collect();
    let mut aspd = [DEFAULT_ASPD_BASE; WEAPON_TYPES];
    for (&weapon, &value) in &data.base_aspd {
        if let Some(slot) = aspd.get_mut(weapon as usize) {
            *slot = value;
        }
    }
    Ok(Job {
        id: data.id,
        name: data.name,
        max_weight: data.max_weight,
        max_base_level: data.max_base_level,
        max_job_level: data.max_job_level,
        bonus,
        aspd,
        shield_aspd: data.shield_aspd.unwrap_or(DEFAULT_ASPD_BASE),
        base_hp: data.base_hp,
        base_sp: data.base_sp,
        base_ap: data.base_ap,
        base_exp: None,
        job_exp: None,
    })
}
