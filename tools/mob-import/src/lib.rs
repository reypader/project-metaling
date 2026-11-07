//! Offline converter from rAthena's renewal mob database (`db/re/mob_db.yml`) to Metaling's
//! `MOB_DATA` YAML (`crates/ro-db/data/mob/mob_db.yml`). Drop item names are resolved through the
//! already-generated item registry. Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/mob_db.md` (clean-room), not ported from rAthena.

mod build;
mod output_dto;
mod raw_dto;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use ro_db::{ItemDb, MobData, MobDb, MOB_DATA_FILE};

use crate::build::build;
use crate::raw_dto::{RathenaFile, RawMob};

pub use output_dto::{Converted, Report};

/// The rAthena source, relative to its root.
pub const SOURCE: &str = "db/re/mob_db.yml";

/// Reads `SOURCE` under `rathena` and converts it.
pub fn import(rathena: &Path, items: &ItemDb) -> anyhow::Result<Converted> {
    let path = rathena.join(SOURCE);
    let text = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
    convert(&text, items).with_context(|| SOURCE.to_string())
}

/// Converts one rAthena mob_db file.
pub fn convert(text: &str, items: &ItemDb) -> anyhow::Result<Converted> {
    let mut options = serde_saphyr::Options::default();
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::LastWins;
    // Trusted offline input, larger than the default node budget.
    options.budget = None;
    let file: RathenaFile = serde_saphyr::from_str_with_options(text, options)?;

    let mut report = Report {
        read: file.body.len(),
        ..Report::default()
    };
    let mut raw: Vec<RawMob> = Vec::new();
    let mut index: HashMap<u32, usize> = HashMap::new();
    for entry in file.body {
        match index.get(&entry.id) {
            Some(&i) => {
                raw[i].merge(entry);
                report.merged += 1;
            }
            None => {
                index.insert(entry.id, raw.len());
                raw.push(entry);
            }
        }
    }

    let mut mobs = Vec::with_capacity(raw.len());
    let mut aegis = HashSet::new();
    for entry in raw {
        let Some(mob) = build(entry, items, &mut report.warnings) else {
            continue;
        };
        if !aegis.insert(mob.aegis_name.to_lowercase()) {
            report.warnings.push(format!(
                "mob {}: duplicate AegisName '{}', skipped",
                mob.id.0, mob.aegis_name
            ));
            continue;
        }
        mobs.push(mob);
    }
    mobs.sort_by_key(|mob| mob.id);
    Ok(Converted { mobs, report })
}

/// The output file: license/attribution comment, then the YAML.
pub fn render_file(mobs: &[MobData], rathena_commit: &str) -> anyhow::Result<String> {
    let yaml = MobDb::file_to_yaml(mobs)?;
    Ok(format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {rathena_commit}, {SOURCE}.\n\
         # Modified: reduced to the fields Metaling uses, drops resolved to item ids, Ai/Modes/Class merged into one mode list, motions clamped as rAthena does at load, by tools/mob-import. Do not edit by hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n{yaml}"
    ))
}

/// Writes `MOB_DATA_FILE` into `out_dir` (created if missing).
pub fn write_output(
    out_dir: &Path,
    converted: &Converted,
    rathena_commit: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    let path = out_dir.join(MOB_DATA_FILE);
    let text = render_file(&converted.mobs, rathena_commit)?;
    std::fs::write(&path, text).with_context(|| path.display().to_string())
}
