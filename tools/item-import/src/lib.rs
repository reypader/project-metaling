//! Offline converter from rAthena's renewal item database (`db/re/item_db_*.yml`) to Metaling's
//! `ITEM_DATA` YAML (`crates/ro-db/data/item/`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/item_db.md` (clean-room), not ported from rAthena.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::Context;
use ro_core::ItemId;
use ro_db::{AmmoType, Gender, ItemData, ItemDb, ItemType, WeaponType, ALL_CLASSES, ALL_JOBS};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

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
/// rAthena `AREA_SIZE`: the largest weapon range.
const MAX_RANGE: u8 = 14;
/// Highest weapon level (renewal has level-5 weapons).
const MAX_WEAPON_LEVEL: u8 = 5;

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
         # Modified: reduced to the fields Metaling uses (ITEM_DATA v2, with equipment fields) and normalized by tools/item-import. Do not edit by hand.\n\
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
    sub_type: Option<String>,
    attack: Option<u32>,
    magic_attack: Option<u32>,
    defense: Option<u32>,
    range: Option<u32>,
    weapon_level: Option<u32>,
    armor_level: Option<u32>,
    equip_level_min: Option<u32>,
    equip_level_max: Option<u32>,
    gender: Option<String>,
    /// Replaced whole by a later entry (rAthena resets the bitsets when `Jobs` is present).
    jobs: Option<Switches>,
    /// Edited by a later entry: each entry's switches apply on top of the earlier value.
    classes: Option<Switches>,
    refineable: Option<bool>,
    flags: Option<RawFlags>,
    stack: Option<RawStack>,
    trade: Option<RawTrade>,
}

/// A YAML map of `name: bool`, in file order.
#[derive(Debug, Clone, Default)]
struct Switches(Vec<(String, bool)>);

impl<'de> Deserialize<'de> for Switches {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SwitchesVisitor;
        impl<'de> Visitor<'de> for SwitchesVisitor {
            type Value = Switches;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map of names to booleans")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Switches, A::Error> {
                let mut list = Vec::new();
                while let Some(entry) = map.next_entry::<String, bool>()? {
                    list.push(entry);
                }
                Ok(Switches(list))
            }
        }
        deserializer.deserialize_map(SwitchesVisitor)
    }
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
        take(&mut self.sub_type, later.sub_type);
        take(&mut self.attack, later.attack);
        take(&mut self.magic_attack, later.magic_attack);
        take(&mut self.defense, later.defense);
        take(&mut self.range, later.range);
        take(&mut self.weapon_level, later.weapon_level);
        take(&mut self.armor_level, later.armor_level);
        take(&mut self.equip_level_min, later.equip_level_min);
        take(&mut self.equip_level_max, later.equip_level_max);
        take(&mut self.gender, later.gender);
        take(&mut self.jobs, later.jobs);
        take(&mut self.refineable, later.refineable);
        if let Some(Switches(later_classes)) = later.classes {
            // An earlier entry without `Classes` stood for every class.
            let mine = self
                .classes
                .get_or_insert_with(|| Switches(vec![("All".into(), true)]));
            mine.0.extend(later_classes);
        }
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

/// Lowercase with underscores removed, so `Super_Novice` and `SuperNovice` match.
fn normalized(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Weapon `SubType` names (`W_*` constants).
fn weapon_type(name: &str) -> Option<WeaponType> {
    use WeaponType::*;
    Some(match normalized(name).as_str() {
        "fist" => Fist,
        "dagger" => Dagger,
        "1hsword" => OneHandSword,
        "2hsword" => TwoHandSword,
        "1hspear" => OneHandSpear,
        "2hspear" => TwoHandSpear,
        "1haxe" => OneHandAxe,
        "2haxe" => TwoHandAxe,
        "mace" => Mace,
        "2hmace" => TwoHandMace,
        "staff" => Staff,
        "bow" => Bow,
        "knuckle" => Knuckle,
        "musical" => Musical,
        "whip" => Whip,
        "book" => Book,
        "katar" => Katar,
        "revolver" => Revolver,
        "rifle" => Rifle,
        "gatling" => Gatling,
        "shotgun" => Shotgun,
        "grenade" => Grenade,
        "huuma" => Huuma,
        "2hstaff" => TwoHandStaff,
        _ => return None,
    })
}

/// Ammo `SubType` names (`AMMO_*` constants).
fn ammo_type(name: &str) -> Option<AmmoType> {
    use AmmoType::*;
    Some(match normalized(name).as_str() {
        "arrow" => Arrow,
        "dagger" => Dagger,
        "bullet" => Bullet,
        "shell" => Shell,
        "grenade" => Grenade,
        "shuriken" => Shuriken,
        "kunai" => Kunai,
        "cannonball" => Cannonball,
        "throwweapon" => ThrowWeapon,
        _ => return None,
    })
}

/// `Jobs` names -> (bitset index, base-class bit): index 0 = first classes and Novice, 1 = the
/// 2-1 branch, 2 = the 2-2 branch (catalog `mechanics/equipment.md` §3a).
fn job_bit(name: &str) -> Option<(usize, u32)> {
    Some(match normalized(name).as_str() {
        "novice" => (0, 0),
        "swordman" => (0, 1),
        "mage" => (0, 2),
        "archer" => (0, 3),
        "acolyte" => (0, 4),
        "merchant" => (0, 5),
        "thief" => (0, 6),
        "taekwon" => (0, 7),
        "gunslinger" => (0, 8),
        "ninja" => (0, 9),
        "summoner" => (0, 10),
        "gangsi" => (0, 11),
        "supernovice" => (1, 0),
        "knight" => (1, 1),
        "wizard" => (1, 2),
        "hunter" => (1, 3),
        "priest" => (1, 4),
        "blacksmith" => (1, 5),
        "assassin" => (1, 6),
        "stargladiator" => (1, 7),
        "rebellion" => (1, 8),
        "kagerouoboro" => (1, 9),
        "spirithandler" => (1, 10),
        "deathknight" => (1, 11),
        "crusader" => (2, 1),
        "sage" => (2, 2),
        "barddancer" => (2, 3),
        "monk" => (2, 4),
        "alchemist" => (2, 5),
        "rogue" => (2, 6),
        "soullinker" => (2, 7),
        "darkcollector" => (2, 11),
        _ => return None,
    })
}

/// `Classes` names -> `ITEMJ_*` bits (renewal).
fn class_bits(name: &str) -> Option<u8> {
    const NORMAL: u8 = 0x01;
    const UPPER: u8 = 0x02;
    const BABY: u8 = 0x04;
    const THIRD: u8 = 0x08;
    const THIRD_UPPER: u8 = 0x10;
    const THIRD_BABY: u8 = 0x20;
    const FOURTH: u8 = 0x40;
    Some(match normalized(name).as_str() {
        "all" => ALL_CLASSES,
        "normal" => NORMAL,
        "upper" => UPPER,
        "baby" => BABY,
        "third" => THIRD,
        "thirdupper" => THIRD_UPPER,
        "thirdbaby" => THIRD_BABY,
        "fourth" => FOURTH,
        "allupper" => UPPER | THIRD_UPPER | FOURTH,
        "allbaby" => BABY | THIRD_BABY,
        "allthird" => THIRD | THIRD_UPPER | THIRD_BABY,
        _ => return None,
    })
}

fn gender(name: &str) -> Option<Gender> {
    Some(match normalized(name).as_str() {
        "female" => Gender::Female,
        "male" => Gender::Male,
        "both" => Gender::Both,
        _ => return None,
    })
}

/// `Jobs`: start from nothing, apply `All`, then each name in file order. An unknown name makes
/// the item usable by every job.
fn jobs(id: u32, switches: Option<Switches>, warnings: &mut Vec<String>) -> [u64; 3] {
    let Some(Switches(list)) = switches else {
        return ALL_JOBS;
    };
    let mut sets = [0u64; 3];
    if list
        .iter()
        .any(|(name, on)| *on && normalized(name) == "all")
    {
        sets = ALL_JOBS;
    }
    for (name, on) in list.iter().filter(|(name, _)| normalized(name) != "all") {
        let Some((index, bit)) = job_bit(name) else {
            warnings.push(format!(
                "item {id}: unknown job '{name}', usable by every job"
            ));
            return ALL_JOBS;
        };
        if *on {
            sets[index] |= 1 << bit;
        } else {
            sets[index] &= !(1 << bit);
        }
    }
    sets
}

/// `Classes`: switches apply in order from 0. An unknown name allows every class.
fn classes(id: u32, switches: Option<Switches>, warnings: &mut Vec<String>) -> u8 {
    let Some(Switches(list)) = switches else {
        return ALL_CLASSES;
    };
    let mut mask = 0u8;
    for (name, on) in list {
        let Some(bits) = class_bits(&name) else {
            warnings.push(format!(
                "item {id}: unknown class '{name}', usable by every class"
            ));
            return ALL_CLASSES;
        };
        if on {
            mask |= bits;
        } else {
            mask &= !bits;
        }
    }
    mask
}

/// `value` when the type may carry the field, else 0 (with a warning if it was set).
fn kept(id: u32, field: &str, value: u32, allowed: bool, warnings: &mut Vec<String>) -> u32 {
    if allowed {
        return value;
    }
    if value != 0 {
        warnings.push(format!("item {id}: {field} on this item type, cleared"));
    }
    0
}

/// Item ids whose gender is forced (catalog `mechanics/equipment.md` §3a "Gender").
const WEDDING_RING_M: u32 = 2634;
const WEDDING_RING_F: u32 = 2635;

fn small<T: TryFrom<u32> + Copy>(value: u32, max: T) -> T {
    T::try_from(value).unwrap_or(max)
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
    if raw.sub_type.is_some()
        && !matches!(
            item_type,
            ItemType::Weapon | ItemType::Ammo | ItemType::Card
        )
    {
        warnings.push(format!(
            "item {id}: SubType on a {item_type:?} item, skipped"
        ));
        return None;
    }

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

    // Equipment fields, cleared by the final type (after the demotions above).
    let is_weapon = item_type == ItemType::Weapon;
    let is_armor = item_type == ItemType::Armor;
    let sub_type = raw.sub_type.as_deref();
    let weapon_type = match sub_type {
        Some(name) if is_weapon => weapon_type(name).unwrap_or_else(|| {
            warnings.push(format!("item {id}: unknown weapon SubType '{name}'"));
            WeaponType::Fist
        }),
        _ => WeaponType::Fist,
    };
    let ammo_type = match sub_type {
        Some(name) if item_type == ItemType::Ammo => {
            let ammo = ammo_type(name);
            if ammo.is_none() {
                warnings.push(format!("item {id}: unknown ammo SubType '{name}'"));
            }
            ammo
        }
        _ => None,
    };
    let attack = kept(
        id,
        "Attack",
        raw.attack.unwrap_or(0),
        is_weapon || item_type == ItemType::Ammo,
        warnings,
    );
    let magic_attack = kept(
        id,
        "MagicAttack",
        raw.magic_attack.unwrap_or(0),
        is_weapon,
        warnings,
    );
    let defense = kept(
        id,
        "Defense",
        raw.defense.unwrap_or(0),
        is_armor || item_type == ItemType::ShadowGear,
        warnings,
    );
    let range = small(
        kept(id, "Range", raw.range.unwrap_or(0), is_weapon, warnings),
        MAX_RANGE,
    )
    .min(MAX_RANGE);
    let weapon_level = match kept(
        id,
        "WeaponLevel",
        raw.weapon_level.unwrap_or(0),
        is_weapon,
        warnings,
    ) {
        _ if !is_weapon => 0,
        0 => 1,
        level => small(level, MAX_WEAPON_LEVEL).min(MAX_WEAPON_LEVEL),
    };
    let armor_level = match kept(
        id,
        "ArmorLevel",
        raw.armor_level.unwrap_or(0),
        is_armor,
        warnings,
    ) {
        _ if !is_armor => 0,
        0 => 1,
        level => small(level, u8::MAX),
    };
    let equip_level_min = small(raw.equip_level_min.unwrap_or(0), u16::MAX);
    let mut equip_level_max = small(raw.equip_level_max.unwrap_or(0), u16::MAX);
    if equip_level_max != 0 && equip_level_max < equip_level_min {
        warnings.push(format!(
            "item {id}: EquipLevelMax below EquipLevelMin, no maximum"
        ));
        equip_level_max = 0;
    }
    let mut gender = match raw.gender.as_deref() {
        None => Gender::Both,
        Some(g) => self::gender(g).unwrap_or_else(|| {
            warnings.push(format!("item {id}: unknown Gender '{g}', using Both"));
            Gender::Both
        }),
    };
    match (weapon_type, id) {
        (WeaponType::Musical, _) | (_, WEDDING_RING_M) => gender = Gender::Male,
        (WeaponType::Whip, _) | (_, WEDDING_RING_F) => gender = Gender::Female,
        _ => {}
    }
    let jobs = jobs(id, raw.jobs, warnings);
    let classes = classes(id, raw.classes, warnings);

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
        weapon_type,
        ammo_type,
        attack,
        magic_attack,
        defense,
        range,
        weapon_level,
        armor_level,
        equip_level_min,
        equip_level_max,
        gender,
        jobs,
        classes,
        refineable: raw.refineable.unwrap_or(false),
        stack_amount,
        stack_inventory,
        unique_id,
        bind_on_equip: flags.bind_on_equip.unwrap_or(false),
        drop_effect,
        no_drop: trade.no_drop.unwrap_or(false),
        no_drop_override: trade.override_level.unwrap_or(DEFAULT_TRADE_OVERRIDE),
    })
}
