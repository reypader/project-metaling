//! Offline converter from rAthena's renewal skill files (`db/re/skill_db.yml`,
//! `db/re/skill_tree.yml`) to Metaling's `SKILL_DATA` and `SKILL_TREE_DATA` YAML
//! (`crates/ro-db/data/skill/`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/skills.md` (clean-room), not ported from rAthena. The
//! importer does the work rAthena does at load time, so `ro-db` stays a plain reader:
//! - per-level lists are expanded ("List form expansion") and cut to the skill's max level;
//! - tree inheritance is resolved ("Load-time inheritance");
//! - job and skill names become ids.

mod convert_skill;
mod level_expand;
mod output_dto;
mod raw_skill_dto;
mod raw_tree_dto;
mod tree;
mod tree_acc_dto;

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use anyhow::Context;
use ro_db::{JobTree, SkillData, SkillDb, SkillTreeDb, MAX_SKILL_LEVEL};

use crate::convert_skill::convert_skill;
use crate::raw_skill_dto::{RathenaFile, RawSkill};
use crate::raw_tree_dto::RawJob;
use crate::tree::{merge_jobs, resolve_tree};

pub use level_expand::{expand, BadLevel, MaxLevels};
pub use output_dto::{Converted, Report};

pub const SKILL_DB_SOURCE: &str = "db/re/skill_db.yml";
pub const SKILL_TREE_SOURCE: &str = "db/re/skill_tree.yml";

/// Per-level slots rAthena keeps for every skill (`MAX_SKILL_LEVEL`).
const SLOTS: usize = MAX_SKILL_LEVEL as usize;

/// `skill_tree.yml` job names that differ from the `job_import::JOB_IDS` names.
const JOB_NAME_ALIASES: [(&str, &str); 1] = [("SUPERNOVICE", "SUPER_NOVICE")];

/// Reads the skill files (and, for the level caps, the job files) under `rathena`.
pub fn import(rathena: &Path) -> anyhow::Result<Converted> {
    let read = |source: &str| {
        let path = rathena.join(source);
        std::fs::read_to_string(&path).with_context(|| path.display().to_string())
    };
    let jobs = job_import::import(rathena).context("job files (for the level caps)")?;
    let max_levels = jobs
        .jobs
        .iter()
        .map(|job| (job.id, (job.max_base_level, job.max_job_level)))
        .collect();
    convert(
        &read(SKILL_DB_SOURCE)?,
        &read(SKILL_TREE_SOURCE)?,
        &max_levels,
    )
}

/// Converts the `skill_db.yml` and `skill_tree.yml` texts.
pub fn convert(
    skill_db: &str,
    skill_tree: &str,
    max_levels: &MaxLevels,
) -> anyhow::Result<Converted> {
    let mut report = Report::default();
    let raw_skills: Vec<RawSkill> = parse_body(skill_db).context(SKILL_DB_SOURCE)?;
    report.skills_read = raw_skills.len();
    let mut skills: HashMap<String, SkillData> = HashMap::new();
    for raw in raw_skills {
        let Some(skill) = convert_skill(raw, &mut report.warnings) else {
            continue;
        };
        if let Some(old) = skills.insert(skill.name.to_uppercase(), skill) {
            report.warnings.push(format!(
                "skill {} ({}) defined twice; the later one wins",
                old.id, old.name
            ));
        }
    }

    let raw_jobs: Vec<RawJob> = parse_body(skill_tree).context(SKILL_TREE_SOURCE)?;
    report.jobs_read = raw_jobs.len();
    let jobs = merge_jobs(raw_jobs);
    let mut job_ids: HashMap<String, u16> = job_import::JOB_IDS
        .iter()
        .map(|(name, id)| (name.to_string(), *id))
        .collect();
    for (alias, name) in JOB_NAME_ALIASES {
        if let Some(&id) = job_ids.get(name) {
            job_ids.insert(alias.to_string(), id);
        }
    }
    let mut trees = Vec::new();
    for job in &jobs.list {
        let Some(&id) = job_ids.get(&job.name.to_uppercase()) else {
            report.skipped_jobs.insert(job.name.clone());
            continue;
        };
        let caps = max_levels.get(&id).copied();
        let tree = resolve_tree(job, &jobs, caps, &skills, &mut report.warnings);
        trees.push(JobTree {
            job: id,
            name: job.name.clone(),
            tree,
        });
    }
    trees.sort_by_key(|tree| tree.job);

    let used: BTreeSet<u16> = trees
        .iter()
        .flat_map(|tree| &tree.tree)
        .flat_map(|entry| {
            std::iter::once(entry.skill).chain(entry.requires.iter().map(|need| need.skill))
        })
        .collect();
    let mut skills: Vec<SkillData> = skills
        .into_values()
        .filter(|skill| used.contains(&skill.id))
        .collect();
    skills.sort_by_key(|skill| skill.id);
    Ok(Converted {
        skills,
        trees,
        report,
    })
}

/// `skill_db.yml`: license/attribution comment, then the YAML.
pub fn render_skills(skills: &[SkillData], rathena_commit: &str) -> anyhow::Result<String> {
    Ok(format!(
        "{}{}",
        attribution(rathena_commit),
        SkillDb::to_yaml(skills)?
    ))
}

/// `skill_tree.yml`: license/attribution comment, then the YAML.
pub fn render_trees(trees: &[JobTree], rathena_commit: &str) -> anyhow::Result<String> {
    Ok(format!(
        "{}{}",
        attribution(rathena_commit),
        SkillTreeDb::to_yaml(trees)?
    ))
}

/// Writes `skill_db.yml` and `skill_tree.yml` into `out_dir` (created if missing).
pub fn write_output(
    out_dir: &Path,
    converted: &Converted,
    rathena_commit: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    for (file, text) in [
        (
            ro_db::SKILL_DATA_FILE,
            render_skills(&converted.skills, rathena_commit)?,
        ),
        (
            ro_db::SKILL_TREE_DATA_FILE,
            render_trees(&converted.trees, rathena_commit)?,
        ),
    ] {
        let path = out_dir.join(file);
        std::fs::write(&path, text).with_context(|| path.display().to_string())?;
    }
    Ok(())
}

fn attribution(commit: &str) -> String {
    format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {commit}, \
         {SKILL_DB_SOURCE}, {SKILL_TREE_SOURCE}.\n\
         # Modified: trimmed to the fields Metaling uses, per-level arrays expanded, tree \
         inheritance resolved, names resolved to ids, by tools/skill-import. Do not edit by \
         hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n"
    )
}

fn parse_body<T: serde::de::DeserializeOwned>(text: &str) -> anyhow::Result<Vec<T>> {
    let mut options = serde_saphyr::Options::default();
    // Trusted offline input: skill_db.yml is larger than the default budget.
    options.budget = None;
    let file: RathenaFile<T> = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}
