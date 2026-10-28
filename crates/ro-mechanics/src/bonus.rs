//! Named modifier slots (ro-engine DESIGN.md §9.3). A source (gear, card, passive, status)
//! contributes `{ slot, value }`; the engine sums each source kind into a `BonusTotals`. No
//! source exists yet, so every total is zero today.

/// The slots, named after rAthena's `b*` bonus types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BonusSlot {
    Str,
    Agi,
    Vit,
    Int,
    Dex,
    Luk,
    /// Status ATK (`bBaseAtk`).
    BaseAtk,
    /// Equipment ATK (`bAtk`, rAthena `eatk`): the window's right side.
    Atk,
    /// Equipment MATK (`bMatk`): the window's right side.
    Matk,
    /// Hard DEF.
    Def,
    /// Soft DEF.
    Def2,
    /// Hard MDEF.
    Mdef,
    /// Soft MDEF.
    Mdef2,
    Hit,
    Flee,
    /// Perfect dodge, in whole points (rAthena adds ×10 internally).
    Flee2,
    /// Critical, in whole points (rAthena adds ×10 internally).
    Critical,
    MaxHp,
    MaxSp,
    /// `bAspd`: flat ASPD points, scaled by AGI.
    AspdFixed,
    /// `bAspdAddRate`-style flat amotion change.
    AspdAdd,
    MaxWeight,
    Range,
    // Percent pools. Kept for the item-bonus plan; `calc_status` doesn't apply them yet.
    AtkRate,
    MatkRate,
    DefRate,
    MdefRate,
    HitRate,
    FleeRate,
    CriticalRate,
    MaxHpRate,
    MaxHpRateItem,
    MaxSpRate,
    MaxSpRateItem,
    AspdRate,
    SpeedRate,
    HpRecovRate,
    SpRecovRate,
}

pub const BONUS_SLOT_COUNT: usize = BonusSlot::SpRecovRate as usize + 1;

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
