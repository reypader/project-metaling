//! A player's skills, kept sorted by id.

use ro_core::{LearnedSkill, SkillFlag};

use crate::skill::SkillSlot;

/// A player's skills: a slot for every visible tree skill (level 0 allowed) and for every
/// skill with a level. Sorted by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SkillSet {
    pub(crate) slots: Vec<SkillSlot>,
    /// Bumped by every change to a level or flag, so a save writes the rows only when they
    /// changed (spec §2.9).
    revision: u32,
}

impl SkillSet {
    /// The set of the stored rows, all visible as rAthena loads them (`id` set), so the first
    /// `compute_visible` counts them; then a transfer's hidden levels, hidden.
    pub(crate) fn from_spawn(rows: &[LearnedSkill], hidden: &[LearnedSkill]) -> Self {
        let mut slots: Vec<SkillSlot> = rows
            .iter()
            .map(|row| (row, true))
            .chain(hidden.iter().map(|row| (row, false)))
            .filter(|(row, _)| row.level > 0)
            .map(|(row, visible)| SkillSlot {
                visible,
                ..SkillSlot::new(row.id, row.level, row.flag)
            })
            .collect();
        // Stable: a visible row wins over a hidden one with the same id.
        slots.sort_by_key(|slot| slot.id);
        slots.dedup_by_key(|slot| slot.id);
        Self { slots, revision: 0 }
    }

    pub(crate) fn revision(&self) -> u32 {
        self.revision
    }

    /// Records a level or flag change.
    pub(crate) fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn get(&self, id: u16) -> Option<&SkillSlot> {
        self.index(id).ok().map(|i| &self.slots[i])
    }

    pub(crate) fn get_mut(&mut self, id: u16) -> Option<&mut SkillSlot> {
        self.index(id).ok().map(|i| &mut self.slots[i])
    }

    /// The learned level of `id`; 0 when absent.
    pub(crate) fn level(&self, id: u16) -> u8 {
        self.get(id).map_or(0, |slot| slot.level)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &SkillSlot> {
        self.slots.iter()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut SkillSlot> {
        self.slots.iter_mut()
    }

    /// The rows a save writes: visible slots with a level (spec §2.9). Hidden levels are not
    /// saved, so they are lost at the next save (captured).
    pub(crate) fn saved_rows(&self) -> Vec<LearnedSkill> {
        self.rows(|slot| slot.visible)
    }

    /// Hidden slots with a level: what a cross-map transfer carries besides the saved rows.
    pub(crate) fn hidden_rows(&self) -> Vec<LearnedSkill> {
        self.rows(|slot| !slot.visible)
    }

    fn rows(&self, keep: impl Fn(&SkillSlot) -> bool) -> Vec<LearnedSkill> {
        self.slots
            .iter()
            .filter(|slot| slot.level > 0 && keep(slot))
            .map(|slot| LearnedSkill {
                id: slot.id,
                level: slot.level,
                flag: slot.flag,
            })
            .collect()
    }

    fn index(&self, id: u16) -> Result<usize, usize> {
        self.slots.binary_search_by_key(&id, |slot| slot.id)
    }

    /// The slot for `id`, inserted (level 0, `Permanent`, hidden) if missing.
    pub(crate) fn ensure(&mut self, id: u16) -> &mut SkillSlot {
        let i = match self.index(id) {
            Ok(i) => i,
            Err(i) => {
                self.slots
                    .insert(i, SkillSlot::new(id, 0, SkillFlag::Permanent));
                i
            }
        };
        &mut self.slots[i]
    }
}
