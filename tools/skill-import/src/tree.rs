//! Merging `skill_tree.yml` jobs and resolving each job's inherited tree.

use std::collections::{BTreeSet, HashMap};

use ro_db::{SkillData, SkillRequirement, TreeEntry};

use crate::raw_tree_dto::RawJob;
use crate::tree_acc_dto::{EntryAcc, JobAcc, Jobs};

/// Merges entries naming the same job, and entries naming the same skill within a job: a
/// later key overrides an earlier one, and `Requires` merges by skill name (rule 5).
pub(crate) fn merge_jobs(raw: Vec<RawJob>) -> Jobs {
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
pub(crate) fn resolve_tree(
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
