//! Inputs and results of the skill tree computation.

use ro_core::Sex;
use ro_db::{JobDb, SkillDb, SkillTreeDb};
use ro_mechanics::BattleConfig;
use smallvec::SmallVec;

/// The read-only data the tree needs.
#[derive(Clone, Copy)]
pub(crate) struct TreeData<'a> {
    pub(crate) skills: &'a SkillDb,
    pub(crate) trees: &'a SkillTreeDb,
    pub(crate) jobs: &'a JobDb,
    pub(crate) config: &'a BattleConfig,
}

/// Who the tree is computed for.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TreeOwner {
    pub(crate) class: u16,
    pub(crate) sex: Sex,
    pub(crate) base_level: u16,
    pub(crate) job_level: u16,
}

/// What a `compute_visible` call changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TreeChange {
    /// Some slot appeared or vanished.
    pub(crate) changed: bool,
    /// Ids that went from visible to hidden, ascending.
    pub(crate) vanished: SmallVec<[u16; 8]>,
}
