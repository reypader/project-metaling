//! Offline converter from rAthena's renewal skill files (`db/re/skill_db.yml`,
//! `db/re/skill_tree.yml`) to Metaling's `SKILL_DATA` and `SKILL_TREE_DATA` YAML
//! (`crates/ro-db/data/skill/`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/skills.md` (clean-room), not ported from rAthena. The
//! importer does the work rAthena does at load time, so `ro-db` stays a plain reader:
//! - per-level lists are expanded ("List form expansion") and cut to the skill's max level;
//! - tree inheritance is resolved ("Load-time inheritance");
//! - job and skill names become ids.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::Path;

use anyhow::Context;
use ro_db::{
    JobTree, SkillData, SkillDb, SkillFlags, SkillRequirement, SkillTreeDb, TreeEntry,
    MAX_SKILL_LEVEL,
};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub const SKILL_DB_SOURCE: &str = "db/re/skill_db.yml";
pub const SKILL_TREE_SOURCE: &str = "db/re/skill_tree.yml";

/// Per-level slots rAthena keeps for every skill (`MAX_SKILL_LEVEL`).
const SLOTS: usize = MAX_SKILL_LEVEL as usize;

/// `skill_tree.yml` job names that differ from the `job_import::JOB_IDS` names.
const JOB_NAME_ALIASES: [(&str, &str); 1] = [("SUPERNOVICE", "SUPER_NOVICE")];

/// `(max base level, max job level)` per job id: tree `BaseLevel`/`JobLevel` are capped to
/// them.
pub type MaxLevels = HashMap<u16, (u16, u16)>;

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
         # Modified: trimmed to learn-only fields, per-level arrays expanded, tree inheritance \
         resolved, names resolved to ids, by tools/skill-import. Do not edit by hand.\n\
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
    // Trusted offline input: skill_db.yml is larger than the default budget.
    options.budget = None;
    let file: RathenaFile<T> = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}

// ---------- skill_db.yml ----------

/// One `skill_db.yml` entry. Keys this importer doesn't use are ignored.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawSkill {
    id: u16,
    name: String,
    max_level: u8,
    target_type: Option<String>,
    range: Option<PerLevel<RawRange>>,
    requires: Option<RawRequires>,
    #[serde(default)]
    flags: BTreeMap<String, bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRequires {
    sp_cost: Option<PerLevel<RawSpCost>>,
}

/// rAthena's per-level keys: one value for every level, or a `{Level, value}` list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PerLevel<T> {
    Scalar(i32),
    List(Vec<T>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRange {
    level: u16,
    size: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawSpCost {
    level: u16,
    amount: i32,
}

fn convert_skill(raw: RawSkill, warnings: &mut Vec<String>) -> Option<SkillData> {
    let label = format!("skill {} ({})", raw.id, raw.name);
    if !(1..=MAX_SKILL_LEVEL).contains(&raw.max_level) {
        warnings.push(format!(
            "{label}: MaxLevel {} out of range, skipped",
            raw.max_level
        ));
        return None;
    }
    let inf = match raw.target_type.as_deref() {
        None | Some("Passive") => 0,
        Some("Attack") => 1,
        Some("Ground") => 2,
        Some("Self") => 4,
        Some("Support") => 16,
        Some("Trap") => 32,
        Some(other) => {
            warnings.push(format!(
                "{label}: unknown TargetType '{other}', using Passive"
            ));
            0
        }
    };
    let levels = usize::from(raw.max_level);
    let sp = raw.requires.and_then(|r| r.sp_cost).map(|sp| match sp {
        PerLevel::Scalar(value) => Ok([value; SLOTS]),
        PerLevel::List(list) => {
            expand(&list.iter().map(|e| (e.level, e.amount)).collect::<Vec<_>>())
        }
    });
    let range = raw.range.map(|range| match range {
        PerLevel::Scalar(value) => Ok([value; SLOTS]),
        PerLevel::List(list) => expand(&list.iter().map(|e| (e.level, e.size)).collect::<Vec<_>>()),
    });
    let (sp_cost, range) = match (sp.transpose(), range.transpose()) {
        (Ok(sp), Ok(range)) => (sp, range),
        (Err(e), _) | (_, Err(e)) => {
            warnings.push(format!("{label}: {e}, skipped"));
            return None;
        }
    };
    let sp_cost = narrow(sp_cost, levels, &label, "SpCost", warnings);
    let range = narrow(range, levels, &label, "Range", warnings);
    let mut flags = SkillFlags::default();
    for (name, &on) in &raw.flags {
        let slot = match name.as_str() {
            "IsQuest" => &mut flags.quest,
            "IsWedding" => &mut flags.wedding,
            "IsSpirit" => &mut flags.spirit,
            "IsNpc" => &mut flags.npc,
            "IsGuild" => &mut flags.guild,
            "AlterRangeVulture" => &mut flags.alter_range_vulture,
            "AlterRangeSnakeEye" => &mut flags.alter_range_snake_eye,
            "AlterRangeRadius" => &mut flags.alter_range_radius,
            "AlterRangeResearchTrap" => &mut flags.alter_range_research_trap,
            "AlterRangeShadowJump" => &mut flags.alter_range_shadow_jump,
            _ => continue,
        };
        *slot = on;
    }
    Some(SkillData {
        id: raw.id,
        name: raw.name,
        max_level: raw.max_level,
        inf,
        sp_cost,
        range,
        flags,
    })
}

/// The first `levels` slots in the target type; empty when they are all 0.
fn narrow<T: TryFrom<i32> + Copy + Default + PartialEq>(
    slots: Option<[i32; SLOTS]>,
    levels: usize,
    label: &str,
    key: &str,
    warnings: &mut Vec<String>,
) -> Vec<T> {
    let Some(slots) = slots else {
        return Vec::new();
    };
    let values: Vec<T> = slots[..levels]
        .iter()
        .map(|&value| {
            T::try_from(value).unwrap_or_else(|_| {
                warnings.push(format!("{label}: {key} {value} out of range, using 0"));
                T::default()
            })
        })
        .collect();
    if values.iter().all(|value| *value == T::default()) {
        Vec::new()
    } else {
        values
    }
}

/// Expansion error: a listed level outside `1..=MAX_SKILL_LEVEL` (rAthena drops the skill).
#[derive(Debug)]
pub struct BadLevel(pub u16);

impl fmt::Display for BadLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "per-level entry at Level {} (1..={MAX_SKILL_LEVEL})",
            self.0
        )
    }
}

/// rAthena's per-level list expansion (catalog `db/skills.md` "List form expansion"). Each
/// `(level, value)` sets that slot; `m` is the highest listed level. Slots above `m` continue
/// the list: the first period `step` in `1..=m/2` for which every `v[k] - v[k-step]` over
/// `step+1..=m` is the same difference `d` gives `v[k] = v[k-step] + d` (a value that drops
/// below 1 after a value of 0 or more becomes 1, and so does every later one); with no such
/// period the value at `m` repeats. Unlisted slots up to `m` stay 0.
pub fn expand(listed: &[(u16, i32)]) -> Result<[i32; SLOTS], BadLevel> {
    // 1-based: v[0] is unused.
    let mut v = [0i32; SLOTS + 1];
    let mut m = 0;
    for &(level, value) in listed {
        let slot = usize::from(level);
        if !(1..=SLOTS).contains(&slot) {
            return Err(BadLevel(level));
        }
        v[slot] = value;
        m = m.max(slot);
    }
    if m == 0 {
        return Ok([0; SLOTS]);
    }
    let period = (1..=m / 2).find_map(|step| {
        let d = v[m] - v[m - step];
        ((step + 1)..=m)
            .all(|k| v[k] - v[k - step] == d)
            .then_some((step, d))
    });
    match period {
        Some((mut step, mut d)) => {
            for k in (m + 1)..=SLOTS {
                v[k] = v[k - step] + d;
                if v[k] < 1 && v[k - 1] >= 0 {
                    v[k] = 1;
                    step = 1;
                    d = 0;
                }
            }
        }
        None => {
            for k in (m + 1)..=SLOTS {
                v[k] = v[m];
            }
        }
    }
    let mut out = [0; SLOTS];
    out.copy_from_slice(&v[1..]);
    Ok(out)
}

// ---------- skill_tree.yml ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawJob {
    job: String,
    #[serde(default)]
    inherit: Ordered,
    #[serde(default)]
    tree: Vec<RawTreeEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawTreeEntry {
    name: String,
    max_level: Option<u8>,
    exclude: Option<bool>,
    base_level: Option<u16>,
    job_level: Option<u16>,
    requires: Option<Vec<RawRequirement>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRequirement {
    name: String,
    level: u8,
}

/// `Inherit`, in file order: a later parent wins a skill both parents define.
#[derive(Debug, Default)]
struct Ordered(Vec<(String, bool)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of job names to booleans")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, bool>()? {
                    out.push((key, value));
                }
                Ok(Ordered(out))
            }
        }
        deserializer.deserialize_map(OrderedVisitor)
    }
}

/// One job's own tree, with repeated entries for the same job merged.
#[derive(Debug, Default)]
struct JobAcc {
    name: String,
    /// Parent names (upper case) in first-listed order; a later listing changes the flag.
    inherit: Vec<(String, bool)>,
    own: Vec<EntryAcc>,
}

#[derive(Debug, Default, Clone)]
struct EntryAcc {
    name: String,
    max_level: Option<u8>,
    exclude: bool,
    base_level: u16,
    job_level: u16,
    /// Level 0 removes a requirement; kept here and dropped when the tree is resolved.
    requires: Vec<(String, u8)>,
}

struct Jobs {
    list: Vec<JobAcc>,
    by_name: HashMap<String, usize>,
}

/// Merges entries naming the same job, and entries naming the same skill within a job: a
/// later key overrides an earlier one, and `Requires` merges by skill name (rule 5).
fn merge_jobs(raw: Vec<RawJob>) -> Jobs {
    let mut jobs = Jobs {
        list: Vec::new(),
        by_name: HashMap::new(),
    };
    for entry in raw {
        let key = entry.job.to_uppercase();
        let index = *jobs.by_name.entry(key).or_insert_with(|| {
            jobs.list.push(JobAcc {
                name: entry.job.clone(),
                ..JobAcc::default()
            });
            jobs.list.len() - 1
        });
        let job = &mut jobs.list[index];
        for (parent, on) in entry.inherit.0 {
            let parent = parent.to_uppercase();
            match job.inherit.iter_mut().find(|(name, _)| *name == parent) {
                Some(slot) => slot.1 = on,
                None => job.inherit.push((parent, on)),
            }
        }
        for raw in entry.tree {
            let at = match job
                .own
                .iter()
                .position(|e| e.name.eq_ignore_ascii_case(&raw.name))
            {
                Some(at) => at,
                None => {
                    job.own.push(EntryAcc {
                        name: raw.name.clone(),
                        ..EntryAcc::default()
                    });
                    job.own.len() - 1
                }
            };
            let acc = &mut job.own[at];
            if raw.max_level.is_some() {
                acc.max_level = raw.max_level;
            }
            if let Some(exclude) = raw.exclude {
                acc.exclude = exclude;
            }
            if let Some(level) = raw.base_level {
                acc.base_level = level;
            }
            if let Some(level) = raw.job_level {
                acc.job_level = level;
            }
            for need in raw.requires.into_iter().flatten() {
                match acc
                    .requires
                    .iter_mut()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&need.name))
                {
                    Some(slot) => slot.1 = need.level,
                    None => acc.requires.push((need.name, need.level)),
                }
            }
        }
    }
    jobs
}

/// The job's own entries, then each parent's own entries in `Inherit` order (rules 1-4): an
/// `Exclude`d parent entry and a skill the job defines itself are not inherited, a later
/// parent wins a skill two parents define, `BaseLevel`/`JobLevel` are capped to the job's max
/// levels, and `MaxLevel: 0` removes the skill. Names become ids; sorted by skill id.
fn resolve_tree(
    job: &JobAcc,
    jobs: &Jobs,
    caps: Option<(u16, u16)>,
    skills: &HashMap<String, SkillData>,
    warnings: &mut Vec<String>,
) -> Vec<TreeEntry> {
    let mut merged: Vec<&EntryAcc> = Vec::new();
    let own_names: BTreeSet<String> = job.own.iter().map(|e| e.name.to_uppercase()).collect();
    for entry in &job.own {
        merged.push(entry);
    }
    for (parent, on) in &job.inherit {
        if !on {
            continue;
        }
        let Some(&index) = jobs.by_name.get(parent) else {
            warnings.push(format!("job {}: unknown parent {parent}", job.name));
            continue;
        };
        for entry in &jobs.list[index].own {
            let key = entry.name.to_uppercase();
            if entry.exclude || own_names.contains(&key) {
                continue;
            }
            match merged
                .iter_mut()
                .find(|e| e.name.eq_ignore_ascii_case(&entry.name))
            {
                Some(slot) => *slot = entry,
                None => merged.push(entry),
            }
        }
    }

    let mut tree = Vec::with_capacity(merged.len());
    for entry in merged {
        let Some(max_level) = entry.max_level else {
            warnings.push(format!(
                "job {}: {} has no MaxLevel, skipped",
                job.name, entry.name
            ));
            continue;
        };
        if max_level == 0 {
            continue;
        }
        let Some(skill) = skills.get(&entry.name.to_uppercase()) else {
            warnings.push(format!(
                "job {}: unknown skill {}, skipped",
                job.name, entry.name
            ));
            continue;
        };
        let (base_cap, job_cap) = caps.unwrap_or((u16::MAX, u16::MAX));
        let mut requires = Vec::with_capacity(entry.requires.len());
        for (name, level) in &entry.requires {
            if *level == 0 {
                continue;
            }
            let Some(need) = skills.get(&name.to_uppercase()) else {
                warnings.push(format!(
                    "job {}: {} requires unknown skill {name}, requirement dropped",
                    job.name, entry.name
                ));
                continue;
            };
            if *level > need.max_level {
                warnings.push(format!(
                    "job {}: {} requires {name} {level}, above its max {}; capped",
                    job.name, entry.name, need.max_level
                ));
            }
            requires.push(SkillRequirement {
                skill: need.id,
                level: (*level).min(need.max_level),
            });
        }
        tree.push(TreeEntry {
            skill: skill.id,
            max_level: max_level.min(skill.max_level),
            base_level: entry.base_level.min(base_cap),
            job_level: entry.job_level.min(job_cap),
            requires,
        });
    }
    tree.sort_by_key(|entry| entry.skill);
    tree
}
