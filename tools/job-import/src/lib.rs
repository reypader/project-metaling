//! Offline converter from rAthena's renewal job files (`db/re/job_stats.yml`, `job_aspd.yml`,
//! `job_exp.yml`, `job_basepoints.yml`, `statpoint.yml`) to Metaling's `JOB_DATA` and
//! `STATPOINT_DATA` YAML (`crates/ro-db/data/job/`). Dev-only; nothing in `crates/` or `bins/`
//! uses it.
//!
//! Written from `reference/catalog/db/job_stats.md`, `db/exp_tables.md` and
//! `mechanics/stats.md` (clean-room), not ported from rAthena.

mod job_ids;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Context;
use ro_db::{JobBonusEntry, JobData, JobDb, StatPointRow, WeaponType};
use serde::Deserialize;

pub use job_ids::JOB_IDS;

/// The job files, in rAthena's renewal load order. Later files override earlier keys.
pub const JOB_SOURCES: [&str; 4] = [
    "db/re/job_stats.yml",
    "db/re/job_aspd.yml",
    "db/re/job_exp.yml",
    "db/re/job_basepoints.yml",
];
pub const STATPOINT_SOURCE: &str = "db/re/statpoint.yml";

/// rAthena `MAX_LEVEL`: the max base and job level of a job with no `job_exp` entry.
const MAX_LEVEL: u16 = 275;
/// Defaults for a job no entry sets (catalog db/job_stats.md).
const DEFAULT_MAX_WEIGHT: u32 = 20000;
const DEFAULT_HP_INCREASE: u32 = 500;
const DEFAULT_SP_INCREASE: u32 = 100;
/// The constant term of the fallback HP and SP formulas.
const FALLBACK_HP_BASE: u32 = 35;
const FALLBACK_SP_BASE: u32 = 10;
/// The fallback base AP is this constant from `FALLBACK_AP_LEVEL` on, for every job (catalog
/// `mechanics/trait_stats.md` §2).
const FALLBACK_AP_BASE: u32 = 200;
const FALLBACK_AP_LEVEL: u16 = 200;
/// The Super Novice line (`mapid & MAPID_SECONDMASK == MAPID_SUPER_NOVICE`: Super Novice, Super
/// Baby, their expanded forms and Hyper Novice). rAthena's fallback base HP adds
/// `SUPER_NOVICE_HP_STEP` from base level 99 and again from 150 for them (catalog
/// `mechanics/stats.md` "Hyper Novice").
const SUPER_NOVICE_LINE: [u16; 5] = [23, 4045, 4190, 4191, 4307];
const SUPER_NOVICE_HP_STEP: u32 = 2000;
const SUPER_NOVICE_HP_LEVELS: [u16; 2] = [99, 150];

#[derive(Debug, Default)]
pub struct Converted {
    /// Sorted by id.
    pub jobs: Vec<JobData>,
    pub points: Vec<StatPointRow>,
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

/// Reads the job and status point files under `rathena` and converts them.
pub fn import(rathena: &Path) -> anyhow::Result<Converted> {
    let read = |source: &str| {
        let path = rathena.join(source);
        std::fs::read_to_string(&path).with_context(|| path.display().to_string())
    };
    let mut texts = Vec::with_capacity(JOB_SOURCES.len());
    for source in JOB_SOURCES {
        texts.push((source, read(source)?));
    }
    let refs: Vec<(&str, &str)> = texts.iter().map(|(l, t)| (*l, t.as_str())).collect();
    convert(&refs, &read(STATPOINT_SOURCE)?)
}

/// Converts `(label, text)` job sources (in load order) and the `statpoint.yml` text.
pub fn convert(job_sources: &[(&str, &str)], statpoint: &str) -> anyhow::Result<Converted> {
    let ids: HashMap<String, u16> = JOB_IDS
        .iter()
        .map(|(name, id)| (name.to_string(), *id))
        .collect();
    let mut report = Report::default();
    let mut jobs: BTreeMap<u16, Acc> = BTreeMap::new();
    for (label, text) in job_sources {
        let body: Vec<RawEntry> = parse_body(text).with_context(|| label.to_string())?;
        report.read.push(body.len());
        for entry in body {
            for (name, on) in &entry.jobs {
                if !on {
                    continue;
                }
                let Some(&id) = ids.get(&name.to_uppercase()) else {
                    report.unknown_jobs.insert(name.clone());
                    continue;
                };
                jobs.entry(id).or_insert_with(|| Acc::new(name)).merge(
                    &entry,
                    label,
                    &mut report.warnings,
                );
            }
        }
    }
    let jobs = jobs
        .into_iter()
        .map(|(id, acc)| acc.finish(id, &mut report))
        .collect();
    let points = parse_body::<RawStatPoint>(statpoint)
        .context(STATPOINT_SOURCE)?
        .into_iter()
        .map(|row| StatPointRow {
            level: row.level,
            points: row.points,
            trait_points: row.trait_points,
        })
        .collect();
    Ok(Converted {
        jobs,
        points,
        report,
    })
}

/// `job_db.yml`: license/attribution comment, then the YAML.
pub fn render_jobs(jobs: &[JobData], rathena_commit: &str) -> anyhow::Result<String> {
    let yaml = JobDb::jobs_to_yaml(jobs)?;
    Ok(format!(
        "{}{yaml}",
        attribution(
            rathena_commit,
            &JOB_SOURCES.join(", "),
            "merged per job id (later files override earlier keys), reduced to max weight, max \
             levels, job-level stat bonuses, ASPD bases and base HP/SP/AP; missing HP/SP/AP \
             table slots filled with rAthena's fallback formulas (with its Super Novice line HP \
             adjustment), all-zero AP tables omitted; exp tables and outfits dropped"
        )
    ))
}

/// `statpoint.yml`: license/attribution comment, then the YAML.
pub fn render_status_points(rows: &[StatPointRow], rathena_commit: &str) -> anyhow::Result<String> {
    let yaml = JobDb::status_points_to_yaml(rows)?;
    Ok(format!(
        "{}{yaml}",
        attribution(
            rathena_commit,
            STATPOINT_SOURCE,
            "reformatted, zero trait points omitted"
        )
    ))
}

/// Writes `job_db.yml` and `statpoint.yml` into `out_dir` (created if missing).
pub fn write_output(
    out_dir: &Path,
    converted: &Converted,
    rathena_commit: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    for (file, text) in [
        (
            ro_db::JOB_DATA_FILE,
            render_jobs(&converted.jobs, rathena_commit)?,
        ),
        (
            ro_db::STATPOINT_DATA_FILE,
            render_status_points(&converted.points, rathena_commit)?,
        ),
    ] {
        let path = out_dir.join(file);
        std::fs::write(&path, text).with_context(|| path.display().to_string())?;
    }
    Ok(())
}

fn attribution(commit: &str, sources: &str, changes: &str) -> String {
    format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {commit}, {sources}.\n\
         # Modified: {changes}, by tools/job-import. Do not edit by hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n"
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RathenaFile<T> {
    #[serde(default = "Vec::new")]
    body: Vec<T>,
}

fn parse_body<T: serde::de::DeserializeOwned>(text: &str) -> anyhow::Result<Vec<T>> {
    let mut options = serde_saphyr::Options::default();
    // Trusted offline input: the exp and basepoints files are larger than the default budget.
    options.budget = None;
    let file: RathenaFile<T> = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}

/// One entry of any job file. Keys this importer doesn't use (exp tables, outfits) are ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawEntry {
    #[serde(default)]
    jobs: BTreeMap<String, bool>,
    max_weight: Option<u32>,
    hp_factor: Option<u32>,
    hp_increase: Option<u32>,
    sp_factor: Option<u32>,
    sp_increase: Option<u32>,
    ap_factor: Option<u32>,
    ap_increase: Option<u32>,
    bonus_stats: Option<Vec<JobBonusEntry>>,
    #[serde(rename = "BaseASPD")]
    base_aspd: Option<BTreeMap<String, u16>>,
    max_base_level: Option<u16>,
    max_job_level: Option<u16>,
    base_hp: Option<Vec<RawHp>>,
    base_sp: Option<Vec<RawSp>>,
    base_ap: Option<Vec<RawAp>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawHp {
    level: u16,
    hp: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawSp {
    level: u16,
    sp: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawAp {
    level: u16,
    ap: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStatPoint {
    level: u16,
    points: u32,
    #[serde(default)]
    trait_points: u32,
}

/// One job's keys merged over every entry naming it.
#[derive(Debug, Default)]
struct Acc {
    name: String,
    max_weight: Option<u32>,
    hp_factor: Option<u32>,
    hp_increase: Option<u32>,
    sp_factor: Option<u32>,
    sp_increase: Option<u32>,
    ap_factor: Option<u32>,
    ap_increase: Option<u32>,
    bonus: BTreeMap<u16, JobBonusEntry>,
    aspd: BTreeMap<WeaponType, u16>,
    shield_aspd: Option<u16>,
    max_base_level: Option<u16>,
    max_job_level: Option<u16>,
    hp: BTreeMap<u16, u32>,
    sp: BTreeMap<u16, u32>,
    ap: BTreeMap<u16, u32>,
}

impl Acc {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    fn merge(&mut self, entry: &RawEntry, label: &str, warnings: &mut Vec<String>) {
        take(&mut self.max_weight, entry.max_weight);
        take(&mut self.hp_factor, entry.hp_factor);
        take(&mut self.hp_increase, entry.hp_increase);
        take(&mut self.sp_factor, entry.sp_factor);
        take(&mut self.sp_increase, entry.sp_increase);
        take(&mut self.ap_factor, entry.ap_factor);
        take(&mut self.ap_increase, entry.ap_increase);
        take(&mut self.max_base_level, entry.max_base_level);
        take(&mut self.max_job_level, entry.max_job_level);
        for gain in entry.bonus_stats.iter().flatten() {
            if self.bonus.insert(gain.level, *gain).is_some() {
                warnings.push(format!(
                    "{}: BonusStats level {} set twice ({label}); the later one wins",
                    self.name, gain.level
                ));
            }
        }
        for (weapon, &value) in entry.base_aspd.iter().flatten() {
            if weapon == "Shield" {
                self.shield_aspd = Some(value);
            } else if let Some(kind) = weapon_type(weapon) {
                self.aspd.insert(kind, value);
            } else {
                warnings.push(format!(
                    "{}: unknown BaseASPD weapon '{weapon}' ({label}), skipped",
                    self.name
                ));
            }
        }
        for row in entry.base_hp.iter().flatten() {
            self.hp.insert(row.level, row.hp);
        }
        for row in entry.base_sp.iter().flatten() {
            self.sp.insert(row.level, row.sp);
        }
        for row in entry.base_ap.iter().flatten() {
            self.ap.insert(row.level, row.ap);
        }
    }

    fn finish(self, id: u16, report: &mut Report) -> JobData {
        let (max_base_level, max_job_level) = match (self.max_base_level, self.max_job_level) {
            (Some(base), Some(job)) => (base, job),
            (base, job) => {
                report.warnings.push(format!(
                    "{} ({id}): no MaxBaseLevel/MaxJobLevel in job_exp, using {MAX_LEVEL}",
                    self.name
                ));
                (base.unwrap_or(MAX_LEVEL), job.unwrap_or(MAX_LEVEL))
            }
        };
        let hp_factor = self.hp_factor.unwrap_or(0);
        let hp_increase = self.hp_increase.unwrap_or(DEFAULT_HP_INCREASE);
        let sp_factor = self.sp_factor.unwrap_or(0);
        let sp_increase = self.sp_increase.unwrap_or(DEFAULT_SP_INCREASE);
        let super_novice = SUPER_NOVICE_LINE.contains(&id);
        let (base_hp, hp_filled) = table(&self.hp, max_base_level, |level| {
            let steps = if super_novice {
                SUPER_NOVICE_HP_LEVELS
                    .iter()
                    .filter(|&&l| level >= l)
                    .count() as u32
            } else {
                0
            };
            fallback(level, FALLBACK_HP_BASE, hp_factor, hp_increase) + steps * SUPER_NOVICE_HP_STEP
        });
        let (base_sp, sp_filled) = table(&self.sp, max_base_level, |level| {
            fallback(level, FALLBACK_SP_BASE, sp_factor, sp_increase)
        });
        let ap_factor = self.ap_factor.unwrap_or(0);
        let ap_increase = self.ap_increase.unwrap_or(0);
        let (mut base_ap, _) = table(&self.ap, max_base_level, |level| {
            let base = if level >= FALLBACK_AP_LEVEL {
                FALLBACK_AP_BASE
            } else {
                0
            };
            fallback(level, base, ap_factor, ap_increase)
        });
        if base_ap.iter().all(|&ap| ap == 0) {
            base_ap.clear();
        }
        if hp_filled > 0 || sp_filled > 0 {
            report
                .fallback
                .push((self.name.clone(), hp_filled, sp_filled));
        }
        let bonus_stats = self
            .bonus
            .into_values()
            .filter(|entry| entry.level <= max_job_level)
            .collect();
        JobData {
            id,
            name: self.name,
            max_weight: self.max_weight.unwrap_or(DEFAULT_MAX_WEIGHT),
            max_base_level,
            max_job_level,
            bonus_stats,
            base_aspd: self.aspd,
            shield_aspd: self.shield_aspd,
            base_hp,
            base_sp,
            base_ap,
        }
    }
}

/// A later key overrides an earlier one; an absent key keeps it.
fn take<T: Copy>(earlier: &mut Option<T>, later: Option<T>) {
    if later.is_some() {
        *earlier = later;
    }
}

/// The dense table for levels `1..=max`: the file's value where it has a non-zero one, the
/// fallback elsewhere. Returns the table and how many slots were filled.
fn table(given: &BTreeMap<u16, u32>, max: u16, fallback: impl Fn(u16) -> u32) -> (Vec<u32>, usize) {
    let mut filled = 0;
    let values = (1..=max)
        .map(|level| match given.get(&level) {
            Some(&value) if value > 0 => value,
            _ => {
                filled += 1;
                fallback(level)
            }
        })
        .collect();
    (values, filled)
}

/// rAthena's base HP/SP/AP when a table slot is missing (catalog mechanics/stats.md,
/// mechanics/trait_stats.md §2):
/// `base + floor(level * increase / 100) + sum over i = 2..=level of round(factor / 100 * i)`.
fn fallback(level: u16, base: u32, factor: u32, increase: u32) -> u32 {
    let per_level = f64::from(factor) / 100.0;
    let mut total = f64::from(base) + (f64::from(level) * f64::from(increase) / 100.0).floor();
    for i in 2..=level {
        total += (per_level * f64::from(i) + 0.5).floor();
    }
    // In range by construction: at most a few hundred thousand for level 275.
    total as u32
}

/// rAthena's `W_*` suffixes as used in `job_aspd.yml`.
fn weapon_type(name: &str) -> Option<WeaponType> {
    Some(match name {
        "Fist" => WeaponType::Fist,
        "Dagger" => WeaponType::Dagger,
        "1hSword" => WeaponType::OneHandSword,
        "2hSword" => WeaponType::TwoHandSword,
        "1hSpear" => WeaponType::OneHandSpear,
        "2hSpear" => WeaponType::TwoHandSpear,
        "1hAxe" => WeaponType::OneHandAxe,
        "2hAxe" => WeaponType::TwoHandAxe,
        "Mace" => WeaponType::Mace,
        "2hMace" => WeaponType::TwoHandMace,
        "Staff" => WeaponType::Staff,
        "Bow" => WeaponType::Bow,
        "Knuckle" => WeaponType::Knuckle,
        "Musical" => WeaponType::Musical,
        "Whip" => WeaponType::Whip,
        "Book" => WeaponType::Book,
        "Katar" => WeaponType::Katar,
        "Revolver" => WeaponType::Revolver,
        "Rifle" => WeaponType::Rifle,
        "Gatling" => WeaponType::Gatling,
        "Shotgun" => WeaponType::Shotgun,
        "Grenade" => WeaponType::Grenade,
        "Huuma" => WeaponType::Huuma,
        "2hStaff" => WeaponType::TwoHandStaff,
        _ => return None,
    })
}
