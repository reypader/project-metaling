//! A player's inventory: slots, weight, `unique_id` assignment, and the add/remove rules
//! (spec `specs/items/spec.md` §2.1; catalog `mechanics/inventory.md` §3-§5, §8).

use ro_core::{
    CharId, CharacterInventory, InventoryItem, ItemEntry, INVENTORY_SLOTS, MAX_ITEM_AMOUNT,
};
use ro_db::ItemDb;

use super::wire::{is_stackable, item_weight, list_entry};
use super::Added;
use crate::equipment::{Gear, WornSlots};

/// `e_additem_result` codes; they go on the wire unchanged (`0x0b41` `result`).
pub(crate) const ADD_INVALID: u8 = 1;
pub(crate) const ADD_OVERWEIGHT: u8 = 2;
pub(crate) const ADD_NO_SLOT: u8 = 4;
pub(crate) const ADD_OVER_AMOUNT: u8 = 5;
pub(crate) const ADD_STACK_LIMIT: u8 = 7;

/// One player's inventory component.
#[derive(Debug, Clone)]
pub(crate) struct PlayerInventory {
    /// Always `INVENTORY_SLOTS` long; `None` is a hole.
    slots: Vec<Option<InventoryItem>>,
    /// Σ Weight × amount, 0.1 units. Unknown ids weigh 0.
    weight: u32,
    max_weight: u32,
    char_id: CharId,
    unique_item_counter: u32,
    /// Bumped on every change; the map-server saves when it differs from the last save.
    revision: u64,
    /// The slot worn at each position, from the items' `equip` masks.
    worn: WornSlots,
}

impl PlayerInventory {
    /// Builds the component from a loaded or transferred inventory. Slots past
    /// `INVENTORY_SLOTS` are dropped. `max_weight` comes from the status (job `MaxWeight` +
    /// allocated STR × 300); the recalc keeps it current.
    pub(crate) fn new(
        inventory: CharacterInventory,
        char_id: CharId,
        max_weight: u32,
        items: &ItemDb,
    ) -> Self {
        let mut slots = inventory.slots;
        slots.resize(INVENTORY_SLOTS, None);
        // Two rows claiming one position (a hand-edited database): the first keeps it and the
        // later one loads unworn, so the item lists and the worn-slot table agree.
        let mut taken = 0u32;
        for item in slots.iter_mut().flatten() {
            if item.equip & taken != 0 {
                item.equip = 0;
            }
            taken |= item.equip;
        }
        let weight = slots
            .iter()
            .flatten()
            .map(|item| item_weight(items, item.item_id).saturating_mul(u32::from(item.amount)))
            .fold(0u32, u32::saturating_add);
        let worn = WornSlots::from_slots(&slots);
        Self {
            slots,
            worn,
            weight,
            max_weight,
            char_id,
            unique_item_counter: inventory.unique_item_counter,
            revision: 0,
        }
    }

    pub(crate) fn weight(&self) -> u32 {
        self.weight
    }

    pub(crate) fn max_weight(&self) -> u32 {
        self.max_weight
    }

    /// Set by the status recalc. Items already held stay, even over the new limit.
    pub(crate) fn set_max_weight(&mut self, max_weight: u32) {
        self.max_weight = max_weight;
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn get(&self, slot: usize) -> Option<&InventoryItem> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// The worn set, for looks and gear status.
    pub(crate) fn gear<'a>(&'a self, items: &'a ItemDb) -> Gear<'a> {
        Gear::new(&self.worn, &self.slots, items)
    }

    /// The slot worn at the first of `positions` (in `EQI` order) that holds an item.
    pub(crate) fn worn_at(&self, positions: u32) -> Option<usize> {
        self.worn.at(positions)
    }

    /// Points `position` at `slot` in the worn-slot table. An equip takes its positions one at a
    /// time (spec §2.2 step 10); [`Self::set_equip`] records the final mask.
    pub(crate) fn wear_position(&mut self, position: u32, slot: usize) {
        self.worn.assign(position, slot);
    }

    /// Sets the worn mask of the item at `slot`. Bumps the revision so autosave sees it.
    pub(crate) fn set_equip(&mut self, slot: usize, mask: u32) {
        if let Some(item) = self.slots.get_mut(slot).and_then(Option::as_mut) {
            item.equip = mask;
            self.revision += 1;
        }
    }

    /// Takes the item at `slot` off: frees its positions and clears its mask. Returns the mask it
    /// was worn on, or `None` when it wasn't worn.
    pub(crate) fn take_off(&mut self, slot: usize) -> Option<u32> {
        let item = self.slots.get_mut(slot)?.as_mut()?;
        let mask = std::mem::take(&mut item.equip);
        if mask == 0 {
            return None;
        }
        self.worn.clear_slot(slot);
        self.revision += 1;
        Some(mask)
    }

    /// A copy for saving; trailing holes are trimmed.
    pub(crate) fn snapshot(&self) -> CharacterInventory {
        let used = self
            .slots
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        CharacterInventory {
            slots: self.slots[..used].to_vec(),
            unique_item_counter: self.unique_item_counter,
        }
    }

    /// Moves the inventory out (cross-map warp); trailing holes are trimmed.
    pub(crate) fn into_character_inventory(mut self) -> CharacterInventory {
        let used = self
            .slots
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        self.slots.truncate(used);
        CharacterInventory {
            slots: self.slots,
            unique_item_counter: self.unique_item_counter,
        }
    }

    fn next_unique_id(&mut self) -> u64 {
        let id = (u64::from(self.char_id.0) << 32) | u64::from(self.unique_item_counter);
        self.unique_item_counter = self.unique_item_counter.wrapping_add(1);
        id
    }

    /// rAthena `pc_additem`: adds `amount` of `item` (its own `amount` is ignored). Checks run in
    /// rAthena's order and the first failure returns its result code with nothing changed.
    pub(crate) fn add(
        &mut self,
        item: &InventoryItem,
        amount: u32,
        items: &ItemDb,
    ) -> Result<Added, u8> {
        if item.item_id.0 == 0 || amount == 0 {
            return Err(ADD_INVALID);
        }
        let amount = match u16::try_from(amount) {
            Ok(a) if a <= MAX_ITEM_AMOUNT => a,
            _ => return Err(ADD_OVER_AMOUNT),
        };
        let data = items.get(item.item_id);
        let stack_cap = data.filter(|d| d.stack_inventory).map(|d| d.stack_amount);
        if stack_cap.is_some_and(|cap| amount > cap) {
            return Err(ADD_STACK_LIMIT);
        }
        let added_weight = data
            .map_or(0, |d| d.weight)
            .saturating_mul(u32::from(amount));
        if u64::from(self.weight) + u64::from(added_weight) > u64::from(self.max_weight) {
            return Err(ADD_OVERWEIGHT);
        }
        let flag_unique = data.is_some_and(|d| d.unique_id);
        let stackable = is_stackable(data);

        let mut key_unique_id = item.unique_id;
        let mut generated = false;
        if flag_unique && key_unique_id == 0 {
            key_unique_id = (u64::from(self.char_id.0) << 32) | u64::from(self.unique_item_counter);
            generated = true;
        }

        if stackable && item.expire_time == 0 {
            let merge = self.slots.iter().position(|s| {
                s.as_ref().is_some_and(|s| {
                    s.item_id == item.item_id
                        && s.bound == item.bound
                        && s.unique_id == key_unique_id
                        && s.expire_time == 0
                        && s.cards == item.cards
                })
            });
            if let Some(slot) = merge {
                let Some(stack) = self.slots[slot].as_mut() else {
                    return Err(ADD_INVALID);
                };
                let limit = stack_cap.unwrap_or(MAX_ITEM_AMOUNT).min(MAX_ITEM_AMOUNT);
                if u32::from(stack.amount) + u32::from(amount) > u32::from(limit) {
                    return Err(ADD_OVER_AMOUNT);
                }
                stack.amount += amount;
                self.finish_add(added_weight, generated);
                return Ok(Added { slot, amount });
            }
        }

        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            return Err(ADD_NO_SLOT);
        };
        let mut new = item.clone();
        new.amount = amount;
        new.equip = 0;
        new.equip_switch = 0;
        new.favorite = false;
        if generated {
            new.unique_id = key_unique_id;
        } else if !stackable && new.unique_id == 0 {
            new.unique_id = self.next_unique_id();
        }
        self.slots[slot] = Some(new);
        self.finish_add(added_weight, generated);
        Ok(Added { slot, amount })
    }

    fn finish_add(&mut self, added_weight: u32, generated_unique_id: bool) {
        if generated_unique_id {
            self.unique_item_counter = self.unique_item_counter.wrapping_add(1);
        }
        self.weight = self.weight.saturating_add(added_weight);
        self.revision += 1;
    }

    /// rAthena `pc_delitem`: removes `amount` from `slot`. Returns the removed part (the stack
    /// with `amount` set). Fails on an empty slot, a zero amount or more than the stack.
    pub(crate) fn remove(
        &mut self,
        slot: usize,
        amount: u16,
        items: &ItemDb,
    ) -> Option<InventoryItem> {
        let entry = self.slots.get_mut(slot)?;
        let stack = entry.as_mut()?;
        if amount == 0 || amount > stack.amount {
            return None;
        }
        let mut removed = stack.clone();
        removed.amount = amount;
        stack.amount -= amount;
        if stack.amount == 0 {
            // Callers take a worn item off first (`unequip_before_removal`, spec §2.8), so the
            // looks, status and `0x099a` follow. Release builds still never keep a stale position.
            debug_assert_eq!(
                stack.equip, 0,
                "removing worn slot {slot} without unequipping"
            );
            *entry = None;
            self.worn.clear_slot(slot);
        }
        self.weight = self
            .weight
            .saturating_sub(item_weight(items, removed.item_id).saturating_mul(u32::from(amount)));
        self.revision += 1;
        Some(removed)
    }

    /// Stackable and non-stackable list entries, in slot order.
    pub(crate) fn list_entries(&self, items: &ItemDb) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
        let mut normal = Vec::new();
        let mut equip = Vec::new();
        for (slot, item) in self.slots.iter().enumerate() {
            let Some(item) = item else { continue };
            let data = items.get(item.item_id);
            let entry = list_entry(slot, item, item.amount, data, items);
            if is_stackable(data) {
                normal.push(entry);
            } else {
                equip.push(entry);
            }
        }
        (normal, equip)
    }

    /// The `0x0b41` entry for `amount` just added at `slot`.
    pub(crate) fn pickup_entry(
        &self,
        slot: usize,
        amount: u16,
        items: &ItemDb,
    ) -> Option<ItemEntry> {
        let item = self.get(slot)?;
        let data = items.get(item.item_id);
        let mut entry = list_entry(slot, item, amount, data, items);
        let bind_on_equip = data.is_some_and(|d| d.bind_on_equip);
        entry.bind_on_equip_type = if item.bound != 0 && !is_stackable(data) {
            2
        } else {
            u16::from(bind_on_equip)
        };
        entry.look = data.map_or(0, |d| u16::try_from(d.view).unwrap_or(0));
        Some(entry)
    }
}
