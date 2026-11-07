//! Offline converter from rAthena's renewal job files (`db/re/job_stats.yml`, `job_aspd.yml`,
//! `job_exp.yml`, `job_basepoints.yml`, `statpoint.yml`) to Metaling's `JOB_DATA` and
//! `STATPOINT_DATA` YAML (`crates/ro-db/data/job/`). Dev-only; nothing in `crates/` or `bins/`
//! uses it.
//!
//! Written from `reference/catalog/db/job_stats.md`, `db/exp_tables.md` and
//! `mechanics/stats.md` (clean-room), not ported from rAthena.

mod acc;
mod job_ids;
mod output_dto;
mod raw_dto;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::Context;
use ro_db::{ExpGroup, ExpKind, JobData, JobDb, StatPointRow};

use crate::acc::Acc;
use crate::raw_dto::{RathenaFile, RawEntry, RawStatPoint};

pub use job_ids::JOB_IDS;
pub use output_dto::{Converted, Report};

/// The job files, in rAthena's renewal load order. Later files override earlier keys.
pub const JOB_SOURCES: [&str; 4] = [
    "db/re/job_stats.yml",
    "db/re/job_aspd.yml",
    "db/re/job_exp.yml",
    "db/re/job_basepoints.yml",
];
pub const STATPOINT_SOURCE: &str = "db/re/statpoint.yml";

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
    let mut tables: BTreeMap<(ExpKind, Vec<u64>), Vec<u16>> = BTreeMap::new();
    let jobs = jobs
        .into_iter()
        .map(|(id, acc)| {
            let (job, base_exp, job_exp) = acc.finish(id, &mut report);
            for (kind, table) in [(ExpKind::Base, base_exp), (ExpKind::Job, job_exp)] {
                if !table.is_empty() {
                    tables.entry((kind, table)).or_default().push(id);
                }
            }
            job
        })
        .collect();
    // Job ids arrive in order, so each group's ids are sorted.
    let mut exp: Vec<ExpGroup> = tables
        .into_iter()
        .map(|((kind, exp), jobs)| ExpGroup { kind, jobs, exp })
        .collect();
    exp.sort_by_key(|group| (group.kind, group.jobs.first().copied()));
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
        exp,
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
             adjustment), all-zero AP tables omitted; exp tables moved to job_exp.yml; outfits \
             dropped"
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

/// `job_exp.yml`: license/attribution comment, then the YAML.
pub fn render_exp_tables(groups: &[ExpGroup], rathena_commit: &str) -> anyhow::Result<String> {
    let yaml = JobDb::exp_tables_to_yaml(groups)?;
    Ok(format!(
        "{}{yaml}",
        attribution(
            rathena_commit,
            JOB_SOURCES[2],
            "merged per job id, rows above the job's max level dropped, missing rows set to 0, \
             job names replaced by ids, jobs with identical tables grouped"
        )
    ))
}

/// Writes `job_db.yml`, `statpoint.yml` and `job_exp.yml` into `out_dir` (created if missing).
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
        (
            ro_db::JOB_EXP_DATA_FILE,
            render_exp_tables(&converted.exp, rathena_commit)?,
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

fn parse_body<T: serde::de::DeserializeOwned>(text: &str) -> anyhow::Result<Vec<T>> {
    let mut options = serde_saphyr::Options::default();
    // Trusted offline input: the exp and basepoints files are larger than the default budget.
    options.budget = None;
    let file: RathenaFile<T> = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}
