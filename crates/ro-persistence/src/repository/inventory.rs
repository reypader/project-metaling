//! Inventory and play-state repository contract (map-server).

use async_trait::async_trait;

use ro_core::{CharId, CharacterInventory};

use super::RepositoryError;

/// What the map-server loads for a character beyond its `CharacterSummary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayState {
    pub str: u16,
    /// The account's permission group.
    pub group_id: i32,
    /// Loaded in row order: no holes.
    pub inventory: CharacterInventory,
}

#[async_trait]
pub trait InventoryRepository: Send + Sync {
    /// Loads STR, the account group and the inventory. `None` if the character doesn't exist.
    async fn load_play_state(&self, char_id: CharId) -> Result<Option<PlayState>, RepositoryError>;

    /// Replaces the character's inventory rows (slot order, holes skipped) and stores the
    /// `unique_id` counter, in one transaction. `NotFound` if the character doesn't exist.
    async fn save_inventory(
        &self,
        char_id: CharId,
        inventory: &CharacterInventory,
    ) -> Result<(), RepositoryError>;
}
