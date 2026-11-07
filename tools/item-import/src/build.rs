//! Building one `ItemData` from a merged rAthena entry.

use ro_core::ItemId;
use ro_db::{Gender, ItemData, ItemType, WeaponType};

use crate::names::{
    ammo_type, classes, drop_effect, gender, item_type, jobs, location_bits, weapon_type,
};
use crate::raw_dto::RawItem;

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
pub(crate) fn build(raw: RawItem, warnings: &mut Vec<String>) -> Option<ItemData> {
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
    let on_use = raw
        .script
        .as_deref()
        .filter(|_| is_consumable(item_type))
        .and_then(crate::script::lift);

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
        on_use,
    })
}

/// The types whose `Script` runs on use (equipment scripts are bonuses).
pub(crate) fn is_consumable(item_type: ItemType) -> bool {
    matches!(
        item_type,
        ItemType::Healing | ItemType::Usable | ItemType::Cash
    )
}
