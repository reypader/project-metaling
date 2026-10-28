//! Offline converter from rAthena's renewal refine table (`db/re/refine.yml`) to Metaling's
//! `REFINE_DATA` YAML (`crates/ro-db/data/refine/`). Dev-only; nothing in `crates/` or `bins/`
//! uses it.
//!
//! Written from `reference/catalog/db/refine.md` (clean-room), not ported from rAthena.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use ro_db::{RefineDb, RefineGroup, RefineRow, MAX_REFINE};
use serde::Deserialize;

pub const SOURCE: &str = "db/re/refine.yml";

#[derive(Debug, Default)]
pub struct Converted {
    /// Armor rows first, then Weapon, each by level.
    pub rows: Vec<RefineRow>,
    pub warnings: Vec<String>,
}

/// Reads `db/re/refine.yml` under `rathena` and converts it.
pub fn import(rathena: &Path) -> anyhow::Result<Converted> {
    let path = rathena.join(SOURCE);
    let text = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
    convert(&text).context(SOURCE)
}

/// Converts the text of `refine.yml`. Only `Bonus` and `RandomBonus` are kept; chances, costs
/// and the shadow groups (which have no bonus) are dropped. Repeated keys override earlier
/// ones, as rAthena merges them.
pub fn convert(text: &str) -> anyhow::Result<Converted> {
    let mut options = serde_saphyr::Options::default();
    // Trusted offline input: the refine file is larger than the default node budget.
    options.budget = None;
    let file: RawFile = serde_saphyr::from_str_with_options(text, options)?;
    let mut warnings = Vec::new();
    let max = usize::from(MAX_REFINE);
    let mut tables: BTreeMap<(u8, u8), (Vec<u32>, Vec<u32>)> = BTreeMap::new();
    for group in file.body {
        let group_key = match group.group.as_str() {
            "Armor" => 0,
            "Weapon" => 1,
            other => {
                warnings.push(format!("group {other}: no stat bonus in Metaling, skipped"));
                continue;
            }
        };
        for level in group.levels {
            let (bonus, random) = tables
                .entry((group_key, level.level))
                .or_insert_with(|| (vec![0; max], vec![0; max]));
            for refine in level.refine_levels {
                let Some(i) = usize::from(refine.level)
                    .checked_sub(1)
                    .filter(|&i| i < max)
                else {
                    warnings.push(format!(
                        "{} level {} refine {}: outside 1..={max}, skipped",
                        group.group, level.level, refine.level
                    ));
                    continue;
                };
                if let Some(value) = refine.bonus {
                    bonus[i] = value;
                }
                if let Some(value) = refine.random_bonus {
                    random[i] = value;
                }
            }
        }
    }
    let rows = tables
        .into_iter()
        .map(|((group, level), (bonus, random))| RefineRow {
            group: if group == 0 {
                RefineGroup::Armor
            } else {
                RefineGroup::Weapon
            },
            level,
            bonus,
            random_bonus: if random.iter().all(|&v| v == 0) {
                Vec::new()
            } else {
                random
            },
        })
        .collect();
    Ok(Converted { rows, warnings })
}

/// `refine.yml`: license/attribution comment, then the YAML.
pub fn render(rows: &[RefineRow], rathena_commit: &str) -> anyhow::Result<String> {
    let yaml = RefineDb::to_yaml(rows)?;
    Ok(format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {rathena_commit}, {SOURCE}.\n\
         # Modified: reduced to Bonus and RandomBonus per group, item level and refine level (chances, \
         costs and shadow groups dropped), by tools/refine-import. Do not edit by hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n{yaml}"
    ))
}

/// Writes `refine.yml` into `out_dir` (created if missing).
pub fn write_output(
    out_dir: &Path,
    rows: &[RefineRow],
    rathena_commit: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    let path = out_dir.join(ro_db::REFINE_DATA_FILE);
    std::fs::write(&path, render(rows, rathena_commit)?).with_context(|| path.display().to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawFile {
    #[serde(default)]
    body: Vec<RawGroup>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawGroup {
    group: String,
    #[serde(default)]
    levels: Vec<RawLevel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawLevel {
    level: u8,
    #[serde(default)]
    refine_levels: Vec<RawRefine>,
}

/// One refine level; `Chances` and the broadcast/blessing keys are ignored.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRefine {
    level: u8,
    bonus: Option<u32>,
    random_bonus: Option<u32>,
}
