//! rAthena names to Metaling enums and bit masks.

use ro_db::{AmmoType, Gender, ItemType, WeaponType, ALL_CLASSES, ALL_JOBS};

use crate::raw_dto::Switches;

pub(crate) fn item_type(name: &str) -> Option<ItemType> {
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
pub(crate) fn location_bits(name: &str) -> Option<u32> {
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
pub(crate) fn drop_effect(name: &str) -> Option<u8> {
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
pub(crate) fn normalized(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Weapon `SubType` names (`W_*` constants).
pub(crate) fn weapon_type(name: &str) -> Option<WeaponType> {
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
pub(crate) fn ammo_type(name: &str) -> Option<AmmoType> {
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

pub(crate) fn gender(name: &str) -> Option<Gender> {
    Some(match normalized(name).as_str() {
        "female" => Gender::Female,
        "male" => Gender::Male,
        "both" => Gender::Both,
        _ => return None,
    })
}

/// `Jobs`: start from nothing, apply `All`, then each name in file order. An unknown name makes
/// the item usable by every job.
pub(crate) fn jobs(id: u32, switches: Option<Switches>, warnings: &mut Vec<String>) -> [u64; 3] {
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
pub(crate) fn classes(id: u32, switches: Option<Switches>, warnings: &mut Vec<String>) -> u8 {
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
