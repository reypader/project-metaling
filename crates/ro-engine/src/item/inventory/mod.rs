//! A player's inventory and its wire helpers.

mod added_dto;
mod player_inventory;
mod wire;

pub(crate) use added_dto::Added;
pub(crate) use player_inventory::{PlayerInventory, ADD_INVALID, ADD_OVER_AMOUNT};
pub(crate) use wire::{slot_of, wire_index};

#[cfg(test)]
mod tests;
