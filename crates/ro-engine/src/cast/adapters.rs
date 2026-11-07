//! Adapters to the ro-scripting traits: the map RNG as `Roll`, a skill set as `SkillLevels`.

use ro_scripting::{Roll, SkillLevels};

use crate::ecs::SplitMix64;
use crate::skill::SkillSet;

/// A hook reads the caster's visible skill levels (hidden slots count toward nothing).
impl SkillLevels for SkillSet {
    fn level(&self, skill: u16) -> u8 {
        self.get(skill)
            .filter(|slot| slot.visible)
            .map_or(0, |slot| slot.level)
    }
}

/// The map's seeded RNG as a hook's roll source.
pub(crate) struct MapRoll<'a>(pub(crate) &'a mut SplitMix64);

impl Roll for MapRoll<'_> {
    fn roll(&mut self, n: u32) -> u32 {
        u32::try_from(self.0.below(u64::from(n))).unwrap_or(0)
    }
}
