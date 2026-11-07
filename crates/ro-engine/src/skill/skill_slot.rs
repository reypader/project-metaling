//! One learned skill: id, level and how it was granted.

use ro_core::SkillFlag;

/// One skill a player has or can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SkillSlot {
    pub(crate) id: u16,
    pub(crate) level: u8,
    pub(crate) flag: SkillFlag,
    /// In the client's list (rAthena `skill[].id != 0`). A hidden slot with a level is one from
    /// a previous class; it counts toward nothing and is not saved.
    pub(crate) visible: bool,
    /// `visible` before the current `compute_visible`, to report what changed.
    pub(crate) was_visible: bool,
}

impl SkillSlot {
    pub(crate) fn new(id: u16, level: u8, flag: SkillFlag) -> Self {
        Self {
            id,
            level,
            flag,
            visible: false,
            was_visible: false,
        }
    }
}
