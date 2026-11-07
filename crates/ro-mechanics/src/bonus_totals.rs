use crate::{BonusSlot, BONUS_SLOT_COUNT};

/// Summed modifiers per slot. Fixed-size and allocation-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BonusTotals {
    values: [i32; BONUS_SLOT_COUNT],
}

impl Default for BonusTotals {
    fn default() -> Self {
        Self {
            values: [0; BONUS_SLOT_COUNT],
        }
    }
}

impl BonusTotals {
    pub fn get(&self, slot: BonusSlot) -> i32 {
        self.values[slot as usize]
    }

    /// Adds `value` to `slot` (every slot sums today).
    pub fn add(&mut self, slot: BonusSlot, value: i32) {
        let total = &mut self.values[slot as usize];
        *total = total.saturating_add(value);
    }
}
