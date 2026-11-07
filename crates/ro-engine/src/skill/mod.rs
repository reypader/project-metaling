//! Skills, learn only (spec `specs/skills/spec.md` §2; catalog `db/skills.md` "Runtime",
//! `mechanics/skills.md` §2-§3): the learned set, which of it the client sees, and the
//! `0x010f` list.
//!
//! A skill's effects are not modelled yet: the learned set is what casting and passive bonuses
//! will read (DESIGN.md §9.9).

mod commands;
mod skill_consts;
mod skill_set;
mod skill_slot;
mod systems;
mod tree;
mod tree_dto;
mod wire;

pub(crate) use skill_set::SkillSet;
pub(crate) use skill_slot::SkillSlot;
pub(crate) use tree::{compute_visible, range2};
pub(crate) use tree_dto::{TreeChange, TreeData, TreeOwner};
pub(crate) use wire::{skill_info, skill_list};

#[cfg(test)]
mod tests;
