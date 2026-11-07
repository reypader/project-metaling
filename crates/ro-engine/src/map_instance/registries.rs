//! The read-only registries every map instance shares.

use std::sync::Arc;

use ro_db::{ItemDb, JobDb, MobDb, RefineDb, SkillDb, SkillTreeDb};
use ro_mechanics::BattleConfig;

use crate::cast::SkillScripts;

/// The read-only data every map instance shares (`MapInstance::set_registries`).
#[derive(Debug, Clone, Default)]
pub struct Registries {
    pub items: Arc<ItemDb>,
    pub mobs: Arc<MobDb>,
    pub jobs: Arc<JobDb>,
    pub refine: Arc<RefineDb>,
    pub skills: Arc<SkillDb>,
    pub skill_tree: Arc<SkillTreeDb>,
    pub battle: Arc<BattleConfig>,
    /// Skill and item-use definitions with their hook backend (spec `specs/effects` §2.1).
    pub scripts: SkillScripts,
}
