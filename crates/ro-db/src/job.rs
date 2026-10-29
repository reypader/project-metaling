//! Job registry (`JOB_DATA`, from `tools/job-import`): per-job max weight, max levels, job-level
//! stat bonuses, ASPD bases and base HP/SP/AP tables, plus the cumulative status- and
//! trait-point table (`STATPOINT_DATA`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize, Serializer};

use crate::yaml::Schema;
use crate::{DbLoadError, WeaponType};

pub const JOB_DATA_TYPE: &str = "JOB_DATA";
pub const JOB_DATA_VERSION: u32 = 2;
pub const JOB_DATA_FILE: &str = "job_db.yml";
pub const STATPOINT_DATA_TYPE: &str = "STATPOINT_DATA";
pub const STATPOINT_DATA_VERSION: u32 = 2;
pub const STATPOINT_DATA_FILE: &str = "statpoint.yml";

const JOB_SCHEMA: Schema = Schema {
    db_type: JOB_DATA_TYPE,
    version: JOB_DATA_VERSION,
};
const STATPOINT_SCHEMA: Schema = Schema {
    db_type: STATPOINT_DATA_TYPE,
    version: STATPOINT_DATA_VERSION,
};

/// ASPD base of a weapon (or shield) the job table doesn't list.
pub const DEFAULT_ASPD_BASE: u16 = 200;
const WEAPON_TYPES: usize = WeaponType::TwoHandStaff as usize + 1;

/// Stat bonus from job level: the six primary stats and the six trait stats.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobBonus {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub str: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub agi: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vit: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub int: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dex: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub luk: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pow: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sta: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wis: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spl: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub con: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub crt: i16,
}

impl JobBonus {
    fn add(&mut self, other: &JobBonus) {
        self.str += other.str;
        self.agi += other.agi;
        self.vit += other.vit;
        self.int += other.int;
        self.dex += other.dex;
        self.luk += other.luk;
        self.pow += other.pow;
        self.sta += other.sta;
        self.wis += other.wis;
        self.spl += other.spl;
        self.con += other.con;
        self.crt += other.crt;
    }
}

/// One `BonusStats` entry: the stats gained on reaching `level`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobBonusEntry {
    pub level: u16,
    #[serde(flatten)]
    pub gain: JobBonus,
}

/// One job as stored in `job_db.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobData {
    pub id: u16,
    /// rAthena's `JOB_` suffix as written in its job files (`Lord_Knight`).
    pub name: String,
    pub max_weight: u32,
    pub max_base_level: u16,
    pub max_job_level: u16,
    /// Sparse gains by job level (as in rAthena); accumulated at load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bonus_stats: Vec<JobBonusEntry>,
    /// Weapons the job lists; any other weapon is `DEFAULT_ASPD_BASE`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub base_aspd: BTreeMap<WeaponType, u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_aspd: Option<u16>,
    /// Base HP at base level `i + 1`; exactly `max_base_level` entries.
    #[serde(serialize_with = "flow_list")]
    pub base_hp: Vec<u32>,
    #[serde(serialize_with = "flow_list")]
    pub base_sp: Vec<u32>,
    /// Base AP at base level `i + 1`: empty (all zero) or exactly `max_base_level` entries.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub base_ap: Vec<u32>,
}

/// One `statpoint.yml` row: cumulative status and trait points at base level `level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct StatPointRow {
    pub level: u16,
    pub points: u32,
    /// Absent below base level 201.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub trait_points: u32,
}

/// A loaded job, with its job bonus accumulated per job level.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: u16,
    pub name: String,
    pub max_weight: u32,
    pub max_base_level: u16,
    pub max_job_level: u16,
    /// Index `job_level - 1`; `max_job_level` rows.
    bonus: Vec<JobBonus>,
    aspd: [u16; WEAPON_TYPES],
    shield_aspd: u16,
    base_hp: Vec<u32>,
    base_sp: Vec<u32>,
    /// Empty when the job has no AP at any level.
    base_ap: Vec<u32>,
}

impl Job {
    /// Total job bonus at `job_level` (clamped to `1..=max_job_level`).
    pub fn bonus(&self, job_level: u16) -> JobBonus {
        clamped(&self.bonus, job_level).copied().unwrap_or_default()
    }

    /// Base HP from the table at `base_level` (clamped to `1..=max_base_level`).
    pub fn base_hp(&self, base_level: u16) -> u32 {
        clamped(&self.base_hp, base_level).copied().unwrap_or(0)
    }

    pub fn base_sp(&self, base_level: u16) -> u32 {
        clamped(&self.base_sp, base_level).copied().unwrap_or(0)
    }

    /// Base AP (the MaxAP before bonuses) at `base_level`; 0 if the job has no AP table.
    pub fn base_ap(&self, base_level: u16) -> u32 {
        clamped(&self.base_ap, base_level).copied().unwrap_or(0)
    }

    pub fn aspd_base(&self, weapon: WeaponType) -> u16 {
        self.aspd
            .get(weapon as usize)
            .copied()
            .unwrap_or(DEFAULT_ASPD_BASE)
    }

    pub fn shield_aspd(&self) -> u16 {
        self.shield_aspd
    }
}

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

    /// Loads `JOB_DATA_FILE` and `STATPOINT_DATA_FILE` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let jobs = load_required(&JOB_SCHEMA, &dir.join(JOB_DATA_FILE))?;
        let points: Vec<StatPointRow> =
            load_required(&STATPOINT_SCHEMA, &dir.join(STATPOINT_DATA_FILE))?;
        Self::from_parts(&dir.display().to_string(), jobs, &points)
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
    })
}

fn load_required<T: serde::de::DeserializeOwned>(
    schema: &Schema,
    path: &Path,
) -> Result<Vec<T>, DbLoadError> {
    schema.load(path)?.ok_or_else(|| DbLoadError::Io {
        path: path.display().to_string(),
        message: "file not found".into(),
    })
}

/// The entry for a 1-based `level`, clamped to the table.
fn clamped<T>(table: &[T], level: u16) -> Option<&T> {
    let last = table.len().checked_sub(1)?;
    table.get(usize::from(level.max(1) - 1).min(last))
}

fn schema_error(path: &str, message: String) -> DbLoadError {
    DbLoadError::Schema {
        path: path.to_string(),
        message,
    }
}

fn is_zero(value: &i16) -> bool {
    *value == 0
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

/// Writes a long numeric table on one line (`[40, 46, 53]`).
fn flow_list<S: Serializer>(values: &[u32], serializer: S) -> Result<S::Ok, S::Error> {
    serde_saphyr::FlowSeq(values).serialize(serializer)
}
