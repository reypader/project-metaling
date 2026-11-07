use ro_core::{CharacterInventory, CharacterStatus, LearnedSkill};

/// What the map-server loads for a character beyond its `CharacterSummary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayState {
    pub status: CharacterStatus,
    /// The account's permission group.
    pub group_id: i32,
    /// Loaded in row order: no holes.
    pub inventory: CharacterInventory,
    /// The learned skills, sorted by id.
    pub skills: Vec<LearnedSkill>,
    /// See `CharacterProgress::change_level`.
    pub change_level: [u16; 3],
}
