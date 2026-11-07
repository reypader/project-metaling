//! Skill list wire entries (`0x010f`, `0x0b31`).

use std::sync::Arc;

use ro_core::{SkillFlag, SkillInfo};
use ro_db::{SkillData, SkillDb, SkillTreeDb};

use crate::skill::{range2, SkillSet, SkillSlot};

/// Entries that fit rAthena's 8192-byte `0x010f` (4-byte header, 37 bytes each).
const MAX_LIST_ENTRIES: usize = 221;

/// One list entry: the `0x010f` / `0x010e` values of a slot (spec §2.3).
pub(crate) fn skill_info(
    slot: &SkillSlot,
    data: &SkillData,
    set: &SkillSet,
    class: u16,
    skills: &SkillDb,
    trees: &SkillTreeDb,
) -> SkillInfo {
    SkillInfo {
        id: slot.id,
        inf: i32::from(data.inf),
        level: u16::from(slot.level),
        sp: data.sp(slot.level),
        range: range2(data, slot.level, set, skills),
        name: Arc::from(data.name.as_str()),
        upgradable: slot.flag == SkillFlag::Permanent
            && slot.level < trees.max_level(class, slot.id, skills),
    }
}

/// The `0x010f` list: every visible slot, ascending id. `upFlag` uses the real class's max.
pub(crate) fn skill_list(
    set: &SkillSet,
    class: u16,
    skills: &SkillDb,
    trees: &SkillTreeDb,
) -> Vec<SkillInfo> {
    let list: Vec<SkillInfo> = set
        .iter()
        .filter(|slot| slot.visible)
        .filter_map(|slot| {
            let data = skills.get(slot.id)?;
            Some(skill_info(slot, data, set, class, skills, trees))
        })
        .collect();
    // No tree comes near rAthena's one-packet cap (it would overflow into `0x0111`).
    debug_assert!(list.len() <= MAX_LIST_ENTRIES, "{} entries", list.len());
    list
}
