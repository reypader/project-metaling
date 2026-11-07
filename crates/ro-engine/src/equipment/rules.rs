//! Equip rules: positions, who may wear what, ammo pairing and the final equip mask.

use ro_core::{InventoryItem, Sex};
use ro_db::{AmmoType, Gender, ItemData, ItemType, WeaponType};

use crate::equipment::gear::saturate;
use crate::equipment::{
    Gear, Refusal, Wearer, WornSlots, EQP_ACC, EQP_ACC_L, EQP_ACC_R, EQP_AMMO, EQP_ARMS,
    EQP_HAND_L, EQP_HAND_R, EQP_SHADOW_ACC, EQP_SHADOW_ACC_L, EQP_SHADOW_ACC_R, EQP_SHADOW_ARMS,
    EQP_SHADOW_SHIELD, EQP_SHADOW_WEAPON, EQP_VISIBLE, EQUIP_FAIL, EQUIP_FAIL_LEVEL,
    MSI_FAIL_NEED_EQUIPPED_BOW, MSI_USESKILL_FAIL_CART, MSI_USESKILL_FAIL_MADOGEAR,
    MSI_WRONG_BULLET,
};
use crate::job::{class_can_use, dual_wields, is_genetic, is_mechanic, job_can_use};

/// rAthena `pc_equippoint`: the item's `Locations`, widened to both hands for a dagger,
/// one-hand sword or one-hand axe on an Assassin or Kagerou/Oboro (shadow weapons likewise).
pub(crate) fn equip_point(item: &ItemData, mapid: u32) -> u32 {
    let dual = item.item_type == ItemType::Weapon
        && matches!(
            item.weapon_type,
            WeaponType::Dagger | WeaponType::OneHandSword | WeaponType::OneHandAxe
        )
        && dual_wields(mapid);
    match item.locations {
        EQP_HAND_R if dual => EQP_ARMS,
        EQP_SHADOW_WEAPON if dual => EQP_SHADOW_ARMS,
        locations => locations,
    }
}

/// The `pc_isequip` subset, in order (spec §2.2 step 7). `weapon_type` is the worn set's
/// [`Gear::weapon_type`], for the ammo pairing.
pub(crate) fn check_equip(
    item: &ItemData,
    entry: &InventoryItem,
    wearer: &Wearer,
    weapon_type: u16,
) -> Result<(), Refusal> {
    let fail = Err(Refusal::Result(EQUIP_FAIL));
    if !level_allows(item, wearer) {
        return Err(Refusal::Result(EQUIP_FAIL_LEVEL));
    }
    if !gender_allows(item, wearer) || entry.attribute != 0 {
        return fail;
    }
    if let Some(ammo) = item.ammo_type.filter(|_| item.item_type == ItemType::Ammo) {
        ammo_check(ammo, weapon_type, wearer.mapid)?;
    }
    if !job_can_use(&item.jobs, wearer.mapid) || !class_can_use(item.classes, wearer.mapid) {
        return fail;
    }
    Ok(())
}

/// `pc_isUseitem`'s restrictions on the user (catalog `mechanics/inventory.md` §7.2 row 15):
/// gender, base level, job and class, as for equipment.
pub(crate) fn usable_by(item: &ItemData, user: &Wearer) -> bool {
    gender_allows(item, user)
        && level_allows(item, user)
        && job_can_use(&item.jobs, user.mapid)
        && class_can_use(item.classes, user.mapid)
}

fn level_allows(item: &ItemData, wearer: &Wearer) -> bool {
    (item.equip_level_min == 0 || wearer.base_level >= item.equip_level_min)
        && (item.equip_level_max == 0 || wearer.base_level <= item.equip_level_max)
}

fn gender_allows(item: &ItemData, wearer: &Wearer) -> bool {
    match item.gender {
        Gender::Both => true,
        Gender::Female => wearer.sex == Sex::Female,
        Gender::Male => wearer.sex == Sex::Male,
    }
}

fn ammo_check(ammo: AmmoType, weapon_type: u16, mapid: u32) -> Result<(), Refusal> {
    let message = |msg_id| Err(Refusal::Message { msg_id });
    match ammo {
        AmmoType::Arrow if !ammo_fits_weapon(ammo, weapon_type) => {
            message(MSI_FAIL_NEED_EQUIPPED_BOW)
        }
        AmmoType::Bullet | AmmoType::Shell | AmmoType::Grenade
            if !ammo_fits_weapon(ammo, weapon_type) =>
        {
            message(MSI_WRONG_BULLET)
        }
        // Needs `AS_VENOMKNIFE`; no skills exist yet.
        AmmoType::Dagger => Err(Refusal::Result(EQUIP_FAIL)),
        // Needs a Mado Gear / a cart on those jobs; neither exists yet.
        AmmoType::Cannonball if is_mechanic(mapid) => message(MSI_USESKILL_FAIL_MADOGEAR),
        AmmoType::Cannonball if is_genetic(mapid) => message(MSI_USESKILL_FAIL_CART),
        _ => Ok(()),
    }
}

/// Whether worn ammo still pairs with the weapon type (the ammo cascade). Only arrows, bullets,
/// shells and grenades need a weapon.
pub(crate) fn ammo_fits_weapon(ammo: AmmoType, weapon_type: u16) -> bool {
    let is = |w: WeaponType| weapon_type == u16::from(w.id());
    match ammo {
        AmmoType::Arrow => is(WeaponType::Bow) || is(WeaponType::Musical) || is(WeaponType::Whip),
        AmmoType::Bullet | AmmoType::Shell => {
            is(WeaponType::Revolver)
                || is(WeaponType::Rifle)
                || is(WeaponType::Gatling)
                || is(WeaponType::Shotgun)
                || is(WeaponType::Grenade)
        }
        AmmoType::Grenade => is(WeaponType::Grenade),
        _ => true,
    }
}

/// Weapons whose removal takes the ammo off too.
pub(crate) fn is_ranged(weapon: WeaponType) -> bool {
    matches!(
        weapon,
        WeaponType::Bow
            | WeaponType::Musical
            | WeaponType::Whip
            | WeaponType::Revolver
            | WeaponType::Rifle
            | WeaponType::Gatling
            | WeaponType::Shotgun
            | WeaponType::Grenade
    )
}

/// The final mask (spec §2.2 step 9): one accessory slot, the dual-wield hand choice, or `pos`
/// unchanged. For a request naming both choices: the second (left) one only when the first is
/// taken and it is free, otherwise the first.
pub(crate) fn resolve_position(item: &ItemData, pos: u32, requested: u32, worn: &WornSlots) -> u32 {
    let pick = |first: u32, second: u32| {
        let asked = requested & (first | second);
        if asked != first | second {
            asked
        } else if worn.at(first).is_some() && worn.at(second).is_none() {
            second
        } else {
            first
        }
    };
    match pos {
        EQP_ACC => pick(EQP_ACC_R, EQP_ACC_L),
        EQP_SHADOW_ACC => pick(EQP_SHADOW_ACC_L, EQP_SHADOW_ACC_R),
        EQP_ARMS if item.locations == EQP_HAND_R => pick(EQP_HAND_R, EQP_HAND_L),
        EQP_SHADOW_ARMS if item.locations == EQP_SHADOW_WEAPON => {
            pick(EQP_SHADOW_WEAPON, EQP_SHADOW_SHIELD)
        }
        pos => pos,
    }
}

/// Spec §2.2 steps 3-9 for an item the character holds: the final mask, or why not. The dead
/// and slot checks (steps 1-2) are the caller's.
pub(crate) fn prepare_equip(
    entry: &InventoryItem,
    requested: u32,
    wearer: &Wearer,
    gear: &Gear<'_>,
) -> Result<u32, Refusal> {
    if !entry.identified {
        return Err(Refusal::Result(EQUIP_FAIL));
    }
    let item = gear.data(entry.item_id).ok_or(Refusal::NoAck)?;
    if !matches!(
        item.item_type,
        ItemType::Weapon | ItemType::Armor | ItemType::Ammo | ItemType::ShadowGear
    ) {
        return Err(Refusal::NoAck);
    }
    let requested = if item.item_type == ItemType::Ammo {
        EQP_AMMO
    } else {
        requested
    };
    let pos = equip_point(item, wearer.mapid);
    check_equip(item, entry, wearer, gear.weapon_type())?;
    if pos & requested == 0 || entry.equip != 0 {
        return Err(Refusal::Result(EQUIP_FAIL));
    }
    Ok(resolve_position(item, pos, requested, gear.worn()))
}

/// The `view` of a `0x0999` ack: the item's `View` on a visible position, else 0.
pub(crate) fn equip_view(item: &ItemData, mask: u32) -> u16 {
    if mask & EQP_VISIBLE != 0 {
        saturate(item.view)
    } else {
        0
    }
}
