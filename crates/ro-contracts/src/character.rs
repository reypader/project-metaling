//! Character creation, deletion reservation, and repository traits.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, CharId, CharacterSummary, Sex};

/// Request DTO for creating a new character on an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateCharacterRequest {
    pub account_id: AccountId,
    pub name: String,
    pub slot: u8,
    pub hair_color: u16,
    pub hair_style: u16,
    pub starting_job: u32,
    pub sex: Sex,
}

/// Character creation error variants matching `HC_REFUSE_MAKECHAR` (`0x006e`) wire error codes.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateCharacterError {
    #[error("Character name already exists")]
    NameAlreadyExists,

    #[error("You are underaged")]
    Underaged,

    #[error("Symbols in character names are forbidden")]
    InvalidNameSymbols,

    #[error("You are not eligible to open the character slot")]
    IneligibleSlot,

    #[error("Character creation denied")]
    CreationDenied,

    #[error("Internal repository failure: {0}")]
    Internal(String),
}

impl CreateCharacterError {
    /// Maps the character creation failure to `HC_REFUSE_MAKECHAR` (`0x006e`) wire error codes.
    pub fn wire_error_code(&self) -> u8 {
        match self {
            Self::NameAlreadyExists => 0x00,
            Self::Underaged => 0x01,
            Self::InvalidNameSymbols => 0x02,
            Self::IneligibleSlot => 0x03,
            Self::CreationDenied | Self::Internal(_) => 0xFF,
        }
    }
}

/// Outcome of a character deletion reservation request (`CH_DELETE_CHAR3_RESERVED` `0x0827`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteReserveOutcome {
    /// Reservation succeeded (`result = 1`), returning the relative delay in seconds (`delete_date - now`).
    Success { delete_delay_seconds: u32 },
    /// Character is already queued for deletion or deletion is forbidden (`result = 0`).
    Denied,
    /// Character does not exist on the account or database failure (`result = 3`).
    NotFoundOrDbError,
    /// Character cannot be deleted while in a guild (`result = 4`).
    GuildRestricted,
    /// Character cannot be deleted while in a party (`result = 5`).
    PartyRestricted,
}

impl DeleteReserveOutcome {
    /// Returns the wire `(result, delete_delay_seconds)` pair for `HC_DELETE_CHAR3_RESERVED` (`0x0828`).
    pub fn wire_fields(self) -> (u32, u32) {
        match self {
            Self::Success {
                delete_delay_seconds,
            } => (1, delete_delay_seconds),
            Self::Denied => (0, 0),
            Self::NotFoundOrDbError => (3, 0),
            Self::GuildRestricted => (4, 0),
            Self::PartyRestricted => (5, 0),
        }
    }
}

/// Asynchronous repository contract for character slot queries, creation, and deletion reservation.
#[async_trait]
pub trait CharacterRepository: Send + Sync {
    /// Lists all characters belonging to the given account, ordered by slot.
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, String>;

    /// Finds a character belonging to `account_id` at `slot`.
    async fn find_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> Result<Option<CharacterSummary>, String>;

    /// Finds a character by its unique `CharId`.
    async fn find_by_id(&self, char_id: CharId) -> Result<Option<CharacterSummary>, String>;

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
    ) -> DeleteReserveOutcome;

    /// Cancels an active deletion reservation (`0x082b` -> `0x082c`), returning true if cancelled.
    async fn cancel_delete(&self, account_id: AccountId, char_id: CharId) -> bool;
}
