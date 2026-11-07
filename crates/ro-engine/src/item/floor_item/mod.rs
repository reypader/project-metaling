//! Items on the floor: creation, expiry, pickup (`0x0362`) and player drop (`0x0363`)
//! (spec `specs/items/spec.md` §2.3-§2.5; catalog `packets/ground_items.md`).

use ro_core::CharId;

use crate::world::FloorItem;

mod systems;

#[cfg(test)]
mod tests;

/// Floor item ids cycle through `[MIN_FLOOR_ID, MAX_FLOOR_ID)` (rAthena `MIN_FLOORITEM`,
/// `MAX_FLOORITEM`): below account ids and NPC/mob ids.
pub(crate) const MIN_FLOOR_ID: u32 = 2;

pub(crate) const MAX_FLOOR_ID: u32 = 2_000_000;

/// rAthena `flooritem_lifetime`.
pub(crate) const FLOOR_ITEM_LIFETIME_MS: u64 = 60_000;

/// How a new floor item is owned and shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FloorItemSpec {
    pub(crate) owners: [Option<CharId>; 3],
    pub(crate) owner_ticks: [u64; 3],
    pub(crate) mob_id: u32,
    pub(crate) show_drop_effect: bool,
    pub(crate) drop_effect_mode: u16,
}

/// When `picker` may take `floor` (spec §2.4 step 4): owner 1 (or no owner) at once, owner 2
/// after tick 1, owner 3 after tick 2, anyone else after tick 3.
fn loot_tick(floor: &FloorItem, picker: Option<CharId>) -> u64 {
    match floor.owners[0] {
        None => 0,
        Some(first) if Some(first) == picker => 0,
        _ if picker.is_some() && floor.owners[1] == picker => floor.owner_ticks[0],
        _ if picker.is_some() && floor.owners[2] == picker => floor.owner_ticks[1],
        _ => floor.owner_ticks[2],
    }
}
