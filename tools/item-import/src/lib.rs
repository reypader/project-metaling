//! Offline converter from rAthena's renewal item database (`db/re/item_db_*.yml`) to Metaling's
//! `ITEM_DATA` YAML (`crates/ro-db/data/item/`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/item_db.md` (clean-room), not ported from rAthena.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::Context;
use ro_core::ItemId;
use ro_db::{ItemData, ItemDb, ItemType};
use serde::Deserialize;

/// `(rAthena source, output file)`, in rAthena's import order.
pub const SOURCES: [(&str, &str); 3] = [
    ("db/re/item_db_usable.yml", "item_usable.yml"),
    ("db/re/item_db_equip.yml", "item_equip.yml"),
    ("db/re/item_db_etc.yml", "item_etc.yml"),
];

/// rAthena `MAX_SLOTS`.
const MAX_SLOTS: u16 = 4;
/// Default `Trade.Override` GM level.
const DEFAULT_TRADE_OVERRIDE: u16 = 100;
/// Location bits that only shadow gear may use.
const SHADOW_LOCATIONS: u32 = 0x3F_0000;
/// `EQP_SHIELD`.
const EQP_SHIELD: u32 = 0x20;

/// The converted items, one list per entry of `SOURCES`, each sorted by id.
#[derive(Debug, Default)]
pub struct Converted {
    pub files: Vec<Vec<ItemData>>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Entries read, per source.
    pub read: Vec<usize>,
    /// Entries that merged into an earlier entry with the same id.
    pub merged: usize,
    pub warnings: Vec<String>,
}

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
        let Some(item) = build(entry.clone(), &mut report.warnings) else {
            continue;
        };
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
         # Modified: reduced to the fields Metaling uses and normalized by tools/item-import. Do not edit by hand.\n\
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RathenaFile {
    #[serde(default)]
    body: Vec<RawItem>,
}

fn parse_rathena(text: &str) -> anyhow::Result<Vec<RawItem>> {
    let mut options = serde_saphyr::Options::default();
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::LastWins;
    // Trusted offline input: rAthena's equip file is larger than the default node budget.
    options.budget = None;
    let file: RathenaFile = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}

/// One rAthena entry. Every key is optional so a repeated id can merge only what it lists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawItem {
    id: u32,
    aegis_name: Option<String>,
    name: Option<String>,
    #[serde(rename = "Type")]
    item_type: Option<String>,
    weight: Option<u32>,
    slots: Option<u16>,
    view: Option<u32>,
    alias_name: Option<String>,
    locations: Option<BTreeMap<String, bool>>,
    flags: Option<RawFlags>,
    stack: Option<RawStack>,
    trade: Option<RawTrade>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawFlags {
    unique_id: Option<bool>,
    bind_on_equip: Option<bool>,
    drop_effect: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStack {
    amount: Option<u16>,
    inventory: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawTrade {
    #[serde(rename = "Override")]
    override_level: Option<u16>,
    no_drop: Option<bool>,
}

/// `later` replaces each field it has; `None` keeps the earlier value.
fn take<T>(earlier: &mut Option<T>, later: Option<T>) {
    if later.is_some() {
        *earlier = later;
    }
}

impl RawItem {
    fn merge(&mut self, later: RawItem) {
        take(&mut self.aegis_name, later.aegis_name);
        take(&mut self.name, later.name);
        take(&mut self.item_type, later.item_type);
        take(&mut self.weight, later.weight);
        take(&mut self.slots, later.slots);
        take(&mut self.view, later.view);
        take(&mut self.alias_name, later.alias_name);
        if let Some(locations) = later.locations {
            self.locations
                .get_or_insert_with(BTreeMap::new)
                .extend(locations);
        }
        if let Some(flags) = later.flags {
            let mine = self.flags.get_or_insert_with(RawFlags::default);
            take(&mut mine.unique_id, flags.unique_id);
            take(&mut mine.bind_on_equip, flags.bind_on_equip);
            take(&mut mine.drop_effect, flags.drop_effect);
        }
        if let Some(stack) = later.stack {
            let mine = self.stack.get_or_insert_with(RawStack::default);
            take(&mut mine.amount, stack.amount);
            take(&mut mine.inventory, stack.inventory);
        }
        if let Some(trade) = later.trade {
            let mine = self.trade.get_or_insert_with(RawTrade::default);
            take(&mut mine.override_level, trade.override_level);
            take(&mut mine.no_drop, trade.no_drop);
        }
    }
}

fn item_type(name: &str) -> Option<ItemType> {
    Some(match name.to_ascii_lowercase().as_str() {
        "healing" => ItemType::Healing,
        "usable" | "delayconsume" => ItemType::Usable,
        "etc" => ItemType::Etc,
        "armor" => ItemType::Armor,
        "weapon" => ItemType::Weapon,
        "card" => ItemType::Card,
        "petegg" => ItemType::PetEgg,
        "petarmor" => ItemType::PetArmor,
        "ammo" => ItemType::Ammo,
        "shadowgear" => ItemType::ShadowGear,
        "cash" => ItemType::Cash,
        _ => return None,
    })
}

/// `EQP_*` bits by YAML `Locations` key (lowercase).
fn location_bits(name: &str) -> Option<u32> {
    Some(match name.to_ascii_lowercase().as_str() {
        "head_low" => 0x1,
        "right_hand" => 0x2,
        "garment" => 0x4,
        "right_accessory" => 0x8,
        "armor" => 0x10,
        "left_hand" => 0x20,
        "shoes" => 0x40,
        "left_accessory" => 0x80,
        "head_top" => 0x100,
        "head_mid" => 0x200,
        "costume_head_top" => 0x400,
        "costume_head_mid" => 0x800,
        "costume_head_low" => 0x1000,
        "costume_garment" => 0x2000,
        "ammo" => 0x8000,
        "shadow_armor" => 0x1_0000,
        "shadow_weapon" => 0x2_0000,
        "shadow_shield" => 0x4_0000,
        "shadow_shoes" => 0x8_0000,
        "shadow_right_accessory" => 0x10_0000,
        "shadow_left_accessory" => 0x20_0000,
        "both_hand" => 0x22,
        "both_accessory" => 0x88,
        _ => return None,
    })
}

/// `e_item_drop_effect` by name (lowercase).
fn drop_effect(name: &str) -> Option<u8> {
    Some(match name.to_ascii_lowercase().as_str() {
        "none" => 0,
        "client" => 1,
        "blue_pillar" => 2,
        "yellow_pillar" => 3,
        "purple_pillar" => 4,
        "green_pillar" => 5,
        "red_pillar" => 6,
        _ => return None,
    })
}

/// Applies rAthena's load rules to one merged entry. `None` (with a warning) when it can't be
/// an item.
fn build(raw: RawItem, warnings: &mut Vec<String>) -> Option<ItemData> {
    let id = raw.id;
    let (Some(aegis_name), Some(name)) = (raw.aegis_name, raw.name) else {
        warnings.push(format!("item {id}: no AegisName or Name, skipped"));
        return None;
    };
    if id == 0 {
        warnings.push(format!("item '{aegis_name}': id 0, skipped"));
        return None;
    }

    let mut item_type = match raw.item_type.as_deref() {
        None => ItemType::Etc,
        Some(t) => item_type(t).unwrap_or_else(|| {
            warnings.push(format!("item {id}: invalid Type '{t}', using Etc"));
            ItemType::Etc
        }),
    };

    let mut locations = 0u32;
    for (key, on) in raw.locations.unwrap_or_default() {
        match location_bits(&key) {
            Some(bits) if on => locations |= bits,
            Some(bits) => locations &= !bits,
            None => warnings.push(format!("item {id}: unknown location '{key}'")),
        }
    }
    let is_equipment = matches!(
        item_type,
        ItemType::Weapon | ItemType::Armor | ItemType::Ammo | ItemType::ShadowGear
    );
    if is_equipment && locations == 0 {
        item_type = ItemType::Etc;
    }
    if locations & SHADOW_LOCATIONS != 0 && item_type != ItemType::ShadowGear {
        item_type = ItemType::Etc;
    }

    let mut view = raw.view.unwrap_or(0);
    if item_type == ItemType::Armor && locations & EQP_SHIELD != 0 && view == 0 {
        view = 1;
    }

    let flags = raw.flags.unwrap_or_default();
    let stackable = item_type.is_stackable();
    let mut unique_id = flags.unique_id.unwrap_or(false);
    if unique_id && !stackable {
        warnings.push(format!(
            "item {id}: UniqueId on a non-stackable item, ignored"
        ));
        unique_id = false;
    }
    let drop_effect = match flags.drop_effect.as_deref() {
        None => 0,
        Some(e) => drop_effect(e).unwrap_or_else(|| {
            warnings.push(format!("item {id}: unknown DropEffect '{e}'"));
            0
        }),
    };

    let (stack_amount, stack_inventory) = match raw.stack {
        Some(stack) if stackable => (stack.amount.unwrap_or(0), stack.inventory.unwrap_or(false)),
        Some(_) => {
            warnings.push(format!("item {id}: Stack on a non-stackable item, ignored"));
            (0, false)
        }
        None => (0, false),
    };
    let trade = raw.trade.unwrap_or_default();

    Some(ItemData {
        id: ItemId(id),
        aegis_name,
        name,
        item_type,
        weight: raw.weight.unwrap_or(0),
        locations,
        view,
        view_id: None,
        slots: raw.slots.unwrap_or(0).min(MAX_SLOTS),
        stack_amount,
        stack_inventory,
        unique_id,
        bind_on_equip: flags.bind_on_equip.unwrap_or(false),
        drop_effect,
        no_drop: trade.no_drop.unwrap_or(false),
        no_drop_override: trade.override_level.unwrap_or(DEFAULT_TRADE_OVERRIDE),
    })
}
