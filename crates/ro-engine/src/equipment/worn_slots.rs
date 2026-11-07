//! Which inventory slot is worn at each equip position.

use ro_core::InventoryItem;

use crate::equipment::EQI_ORDER;

/// The inventory slot worn at each position (rAthena `equip_index`), indexed like `EQI_ORDER`.
/// Built from the items' `equip` masks at spawn; an equip fills it one position at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WornSlots {
    pub(crate) slots: [Option<u8>; EQI_ORDER.len()],
}

impl WornSlots {
    /// Every set `equip` bit points at its item's slot.
    pub(crate) fn from_slots(slots: &[Option<InventoryItem>]) -> Self {
        let mut worn = Self::default();
        for (slot, item) in slots.iter().enumerate() {
            if let Some(item) = item.as_ref().filter(|i| i.equip != 0) {
                worn.assign(item.equip, slot);
            }
        }
        worn
    }

    /// The slot at the first position (in `EQI` order) of `positions` that holds an item.
    pub(crate) fn at(&self, positions: u32) -> Option<usize> {
        EQI_ORDER
            .iter()
            .zip(&self.slots)
            .find_map(|(&p, &s)| s.filter(|_| p & positions != 0))
            .map(usize::from)
    }

    /// Points every position in `positions` at `slot`.
    pub(crate) fn assign(&mut self, positions: u32, slot: usize) {
        let Ok(slot) = u8::try_from(slot) else {
            return;
        };
        for (p, s) in EQI_ORDER.iter().zip(&mut self.slots) {
            if p & positions != 0 {
                *s = Some(slot);
            }
        }
    }

    /// Frees every position `slot` holds.
    pub(crate) fn clear_slot(&mut self, slot: usize) {
        for s in &mut self.slots {
            if s.is_some_and(|s| usize::from(s) == slot) {
                *s = None;
            }
        }
    }
}
