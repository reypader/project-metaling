/// The caster's learned skill levels.
pub trait SkillLevels {
    /// The level of `skill`; 0 when not learned.
    fn level(&self, skill: u16) -> u8;
}
