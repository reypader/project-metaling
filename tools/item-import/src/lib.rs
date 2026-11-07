//! Offline converter from rAthena's renewal item database (`db/re/item_db_*.yml`) to Metaling's
//! `ITEM_DATA` YAML (`crates/ro-db/data/item/`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/item_db.md` (clean-room), not ported from rAthena.

mod build;
mod names;
mod output_dto;
mod raw_dto;
mod script;

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use ro_core::ItemId;
use ro_db::{ItemData, ItemDb, ItemUse};

use crate::build::{build, is_consumable};
use crate::raw_dto::{RathenaFile, RawItem};

pub use output_dto::{Converted, Report};

/// `(rAthena source, output file)`, in rAthena's import order.
pub const SOURCES: [(&str, &str); 3] = [
    ("db/re/item_db_usable.yml", "item_usable.yml"),
    ("db/re/item_db_equip.yml", "item_equip.yml"),
    ("db/re/item_db_etc.yml", "item_etc.yml"),
];

/// Reads the three source files under `rathena` and converts them.
pub fn import(rathena: &Path) -> anyhow::Result<Converted> {
    let mut texts = Vec::with_capacity(SOURCES.len());
    for (source, _) in SOURCES {
        let path = rathena.join(source);
        let text = std::fs::read_to_string(&path).with_context(|| format!("{}", path.display()))?;
        texts.push((source, text));
    }
    let refs: Vec<(&str, &str)> = texts.iter().map(|(l, t)| (*l, t.as_str())).collect();
    convert(&refs)
}

/// Converts `(label, text)` sources. Output list `i` holds the items whose id first appeared in
/// source `i`.
pub fn convert(sources: &[(&str, &str)]) -> anyhow::Result<Converted> {
    let mut report = Report::default();
    // id -> (source index, merged raw entry), in first-seen order.
    let mut raw: Vec<(usize, RawItem)> = Vec::new();
    let mut index: HashMap<u32, usize> = HashMap::new();
    for (file_index, (label, text)) in sources.iter().enumerate() {
        let body = parse_rathena(text).with_context(|| label.to_string())?;
        report.read.push(body.len());
        for entry in body {
            match index.get(&entry.id) {
                Some(&i) => {
                    raw[i].1.merge(entry);
                    report.merged += 1;
                }
                None => {
                    index.insert(entry.id, raw.len());
                    raw.push((file_index, entry));
                }
            }
        }
    }

    // First pass: build entries without aliases; skip incomplete ones and duplicate aegis names.
    let mut built: Vec<(usize, ItemData, Option<String>)> = Vec::with_capacity(raw.len());
    let mut aegis_ids: HashMap<String, ItemId> = HashMap::new();
    for (file_index, entry) in raw {
        let id = entry.id;
        let has_script = entry.script.is_some();
        let Some(item) = build(entry.clone(), &mut report.warnings) else {
            continue;
        };
        if has_script && is_consumable(item.item_type) {
            match item.on_use {
                Some(ItemUse::ItemHeal { .. }) => report.lifted_item_heal += 1,
                Some(ItemUse::PercentHeal { .. }) => report.lifted_percent_heal += 1,
                None => report.not_lifted += 1,
            }
        }
        let key = item.aegis_name.to_lowercase();
        if aegis_ids.contains_key(&key) {
            report.warnings.push(format!(
                "item {id}: duplicate AegisName '{}', skipped",
                item.aegis_name
            ));
            continue;
        }
        aegis_ids.insert(key, item.id);
        built.push((file_index, item, entry.alias_name));
    }

    let mut files: Vec<Vec<ItemData>> = vec![Vec::new(); sources.len()];
    for (file_index, mut item, alias) in built {
        if let Some(alias) = alias {
            match aegis_ids.get(&alias.to_lowercase()) {
                Some(&target) => item.view_id = Some(target),
                None => report
                    .warnings
                    .push(format!("item {}: unknown AliasName '{alias}'", item.id.0)),
            }
        }
        if let Some(list) = files.get_mut(file_index) {
            list.push(item);
        }
    }
    for list in &mut files {
        list.sort_by_key(|item| item.id);
    }
    Ok(Converted { files, report })
}

/// One output file: license/attribution comment, then the YAML.
pub fn render_file(
    items: &[ItemData],
    rathena_commit: &str,
    source: &str,
) -> anyhow::Result<String> {
    let yaml = ItemDb::file_to_yaml(items)?;
    Ok(format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {rathena_commit}, {source}.\n\
         # Modified: reduced to the fields Metaling uses (ITEM_DATA v3, with equipment fields and lifted heal `Use` effects) and normalized by tools/item-import. Do not edit by hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n{yaml}"
    ))
}

/// Writes the three files into `out_dir` (created if missing). Returns the number of items.
pub fn write_output(
    out_dir: &Path,
    converted: &Converted,
    rathena_commit: &str,
) -> anyhow::Result<usize> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    let mut total = 0;
    for ((source, file), items) in SOURCES.iter().zip(&converted.files) {
        let text = render_file(items, rathena_commit, source)?;
        let path = out_dir.join(file);
        std::fs::write(&path, text).with_context(|| path.display().to_string())?;
        total += items.len();
    }
    Ok(total)
}

fn parse_rathena(text: &str) -> anyhow::Result<Vec<RawItem>> {
    let mut options = serde_saphyr::Options::default();
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::LastWins;
    // Trusted offline input: rAthena's equip file is larger than the default node budget.
    options.budget = None;
    let file: RathenaFile = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}
