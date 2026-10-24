//! Offline converter from rAthena's renewal mob database (`db/re/mob_db.yml`) to Metaling's
//! `MOB_DATA` YAML (`crates/ro-db/data/mob/mob_db.yml`). Drop item names are resolved through the
//! already-generated item registry. Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/mob_db.md` (clean-room), not ported from rAthena.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use ro_db::{
    ItemDb, MobClass, MobData, MobDb, MobDrop, MobId, MAX_DROP_RATE, MAX_MOB_DROPS, MAX_MVP_DROPS,
    MOB_DATA_FILE,
};
use serde::Deserialize;

/// The rAthena source, relative to its root.
pub const SOURCE: &str = "db/re/mob_db.yml";

/// rAthena `DEFAULT_WALK_SPEED`, `MIN_WALK_SPEED`, `MAX_WALK_SPEED`.
const DEFAULT_WALK_SPEED: u16 = 150;
const MIN_WALK_SPEED: u16 = 20;
const MAX_WALK_SPEED: u16 = 1000;

/// The converted mobs, sorted by id.
#[derive(Debug, Default)]
pub struct Converted {
    pub mobs: Vec<MobData>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Entries read.
    pub read: usize,
    /// Entries that merged into an earlier entry with the same id.
    pub merged: usize,
    pub warnings: Vec<String>,
}

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
         # Modified: reduced to the fields Metaling uses, drops resolved to item ids, by tools/mob-import. Do not edit by hand.\n\
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RathenaFile {
    #[serde(default)]
    body: Vec<RawMob>,
}

/// One rAthena entry. Every key is optional so a repeated id can merge only what it lists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawMob {
    id: u32,
    aegis_name: Option<String>,
    name: Option<String>,
    level: Option<u16>,
    hp: Option<u32>,
    walk_speed: Option<u16>,
    class: Option<String>,
    modes: Option<BTreeMap<String, bool>>,
    /// Every occurrence's list, in file order; slots are assigned at build.
    #[serde(default)]
    drops: Vec<RawDrop>,
    #[serde(default)]
    mvp_drops: Vec<RawDrop>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawDrop {
    item: String,
    rate: u32,
    index: Option<usize>,
}

fn take<T>(earlier: &mut Option<T>, later: Option<T>) {
    if later.is_some() {
        *earlier = later;
    }
}

impl RawMob {
    fn merge(&mut self, later: RawMob) {
        take(&mut self.aegis_name, later.aegis_name);
        take(&mut self.name, later.name);
        take(&mut self.level, later.level);
        take(&mut self.hp, later.hp);
        take(&mut self.walk_speed, later.walk_speed);
        take(&mut self.class, later.class);
        if let Some(modes) = later.modes {
            self.modes.get_or_insert_with(BTreeMap::new).extend(modes);
        }
        self.drops.extend(later.drops);
        self.mvp_drops.extend(later.mvp_drops);
    }
}

fn mob_class(name: &str) -> Option<MobClass> {
    Some(match name.to_ascii_lowercase().as_str() {
        "normal" => MobClass::Normal,
        "boss" => MobClass::Boss,
        "guardian" => MobClass::Guardian,
        "battlefield" => MobClass::Battlefield,
        "event" => MobClass::Event,
        _ => return None,
    })
}

/// Places drops into `max` slots: `Index` overwrites that slot, otherwise the first free slot.
/// Unknown items are skipped; rate 0 entries are removed afterwards; rates are capped at 10000.
fn resolve_drops(
    mob_id: u32,
    list: &[RawDrop],
    max: usize,
    items: &ItemDb,
    warnings: &mut Vec<String>,
) -> Vec<MobDrop> {
    let mut slots: Vec<Option<MobDrop>> = vec![None; max];
    for raw in list {
        let Some(item) = items.by_aegis_name(&raw.item) else {
            warnings.push(format!("mob {mob_id}: unknown drop item '{}'", raw.item));
            continue;
        };
        let rate = u16::try_from(raw.rate.min(u32::from(MAX_DROP_RATE))).unwrap_or(MAX_DROP_RATE);
        let drop = MobDrop {
            item: item.id,
            rate,
        };
        let slot = match raw.index {
            Some(i) if i < max => Some(i),
            Some(i) => {
                warnings.push(format!("mob {mob_id}: drop Index {i} out of range"));
                None
            }
            None => slots.iter().position(Option::is_none).or_else(|| {
                warnings.push(format!("mob {mob_id}: more than {max} drops"));
                None
            }),
        };
        if let Some(slot) = slot.and_then(|i| slots.get_mut(i)) {
            *slot = Some(drop);
        }
    }
    slots.into_iter().flatten().filter(|d| d.rate > 0).collect()
}

fn build(raw: RawMob, items: &ItemDb, warnings: &mut Vec<String>) -> Option<MobData> {
    let id = raw.id;
    if !MobId(id).is_valid() {
        warnings.push(format!("mob {id}: id outside the mob id ranges, skipped"));
        return None;
    }
    let (Some(aegis_name), Some(name)) = (raw.aegis_name, raw.name) else {
        warnings.push(format!("mob {id}: no AegisName or Name, skipped"));
        return None;
    };
    let class = match raw.class.as_deref() {
        None => MobClass::Normal,
        Some(c) => mob_class(c).unwrap_or_else(|| {
            warnings.push(format!("mob {id}: unknown Class '{c}', using Normal"));
            MobClass::Normal
        }),
    };
    let mvp = raw
        .modes
        .unwrap_or_default()
        .iter()
        .any(|(mode, on)| *on && mode.eq_ignore_ascii_case("mvp"));
    Some(MobData {
        id: MobId(id),
        aegis_name,
        name,
        level: raw.level.unwrap_or(1),
        hp: raw.hp.unwrap_or(1),
        walk_speed: raw
            .walk_speed
            .unwrap_or(DEFAULT_WALK_SPEED)
            .clamp(MIN_WALK_SPEED, MAX_WALK_SPEED),
        class,
        mvp,
        drops: resolve_drops(id, &raw.drops, MAX_MOB_DROPS, items, warnings),
        mvp_drops: resolve_drops(id, &raw.mvp_drops, MAX_MVP_DROPS, items, warnings),
    })
}
