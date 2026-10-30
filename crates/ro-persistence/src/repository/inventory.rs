//! Inventory and play-state repository contract (map-server).

use async_trait::async_trait;

use ro_core::{
    CharId, CharacterInventory, CharacterLooks, CharacterProgress, CharacterStatus, LearnedSkill,
};

use super::RepositoryError;

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

#[async_trait]
pub trait InventoryRepository: Send + Sync {
    /// Loads the stats, points and exp, the account group, the inventory, the skills and the
    /// class-change levels. `None` if the character doesn't exist.
    async fn load_play_state(&self, char_id: CharId) -> Result<Option<PlayState>, RepositoryError>;

    /// Saves what play changes, in one transaction: the progress (job, levels, stats, points,
    /// exp, HP/SP, class-change levels), the char-select looks of the worn gear, the
    /// `unique_id` counter, the inventory rows (replaced; slot order, holes skipped) and the
    /// skill rows (replaced; levels above 0 only). `NotFound` if the character doesn't exist.
    async fn save_play_state(
        &self,
        char_id: CharId,
        inventory: &CharacterInventory,
        looks: &CharacterLooks,
        progress: &CharacterProgress,
        skills: &[LearnedSkill],
    ) -> Result<(), RepositoryError>;
}
