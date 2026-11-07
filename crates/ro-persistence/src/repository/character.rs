//! Character repository contract.

use async_trait::async_trait;

use ro_core::{AccountId, CharId, CharacterSummary, CreateCharacterRequest, MapPosition};

use super::{CancelDeleteOutcome, CreateCharacterError, DeleteReserveOutcome, RepositoryError};

/// Asynchronous repository contract for character slot queries, creation, and deletion reservation.
#[async_trait]
pub trait CharacterRepository: Send + Sync {
    /// Lists all characters belonging to the given account, ordered by slot.
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError>;

    /// Finds a character belonging to `account_id` at `slot`.
    async fn find_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> Result<Option<CharacterSummary>, RepositoryError>;

    /// Finds a character by its unique `CharId`.
    async fn find_by_id(
        &self,
        char_id: CharId,
    ) -> Result<Option<CharacterSummary>, RepositoryError>;

    /// Validates and persists a newly created character.
    async fn create_character(
        &self,
        req: CreateCharacterRequest,
        max_slots: u8,
    ) -> Result<CharacterSummary, CreateCharacterError>;

    /// Reserves a character for delayed deletion (`0x0827` -> `0x0828`).
    async fn reserve_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
        delay_seconds: u32,
    ) -> Result<DeleteReserveOutcome, RepositoryError>;

    /// Cancels an active deletion reservation (`0x082b` -> `0x082c`).
    async fn cancel_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
    ) -> Result<CancelDeleteOutcome, RepositoryError>;

    /// Persists updated coordinates and map location for an active character.
    async fn update_position(
        &self,
        char_id: CharId,
        map_name: &str,
        pos: MapPosition,
    ) -> Result<(), RepositoryError>;
}
