//! Helpers shared by the test modules.

mod consts;

use std::collections::HashMap;

use ro_scripting::SkillLevels;

pub use consts::*;

/// Learned skill levels by id.
#[derive(Default)]
pub struct Levels(pub HashMap<u16, u8>);

impl SkillLevels for Levels {
    fn level(&self, skill: u16) -> u8 {
        self.0.get(&skill).copied().unwrap_or(0)
    }
}
