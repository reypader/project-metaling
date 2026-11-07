//! Client UI payloads: the skill list, status window and achievement totals.

use std::sync::Arc;

/// One `0x010f` skill-list entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInfo {
    pub id: u16,
    /// Target type (rAthena `skill_get_inf`); 0 for a passive.
    pub inf: i32,
    pub level: u16,
    pub sp: u16,
    pub range: u16,
    /// Aegis name, e.g. `NV_BASIC`; at most 23 bytes go on the wire.
    pub name: Arc<str>,
    /// The client shows the level-up button.
    pub upgradable: bool,
}

/// The `0x00bd` status window: allocated stats with the cost of raising each by one, and the
/// derived values. Stats above 255 are saturated on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusWindow {
    pub status_point: u16,
    /// STR, AGI, VIT, INT, DEX, LUK.
    pub stats: [u16; 6],
    /// Points to raise each stat by one; 0 at the cap.
    pub costs: [u8; 6],
    /// Status ATK (left side of ATK).
    pub atk: i16,
    /// Weapon and equipment ATK (right side).
    pub atk_bonus: i16,
    /// Weapon and equipment MATK (right side).
    pub matk_bonus: i16,
    /// Status MATK (left side).
    pub matk: i16,
    pub soft_def: i16,
    pub hard_def: i16,
    pub soft_mdef: i16,
    pub hard_mdef: i16,
    pub hit: i16,
    pub flee: i16,
    pub perfect_dodge: i16,
    pub critical: i16,
    /// rAthena `amotion`.
    pub aspd: i16,
}

/// The achievement totals in `0x0a24` (and later `0x0a23`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AchievementSummary {
    pub total_score: u32,
    pub level: u16,
    pub exp: u32,
    /// Exp needed for the next achievement level.
    pub exp_to_next: u32,
}
