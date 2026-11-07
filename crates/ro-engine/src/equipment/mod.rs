//! Equipment rules, pure: where an item may go, whether a character may wear it, and the looks
//! and weapon type of a worn set (spec `specs/equipment/spec.md` §2.1, §2.2; catalog
//! `mechanics/equipment.md` §1, §3, §3a, §5).

mod eqp_consts;
mod gear;
mod gear_dto;
mod rules;
mod systems;
mod wearer_dto;
mod worn_slots;

pub(crate) use eqp_consts::*;
pub(crate) use gear::Gear;
pub(crate) use gear_dto::{GearStatus, WeaponGear};
pub(crate) use rules::{
    ammo_fits_weapon, check_equip, equip_view, is_ranged, prepare_equip, usable_by,
};
pub(crate) use wearer_dto::{Refusal, Wearer};
pub(crate) use worn_slots::WornSlots;

#[cfg(test)]
mod tests;
