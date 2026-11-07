//! Repository contracts the servers depend on, implemented by `PostgresRepo` and by test doubles.

mod account;
mod auth_error;
mod character;
mod character_outcome_dto;
mod error;
mod inventory;
mod play_state;

pub use account::AccountRepository;
pub use auth_error::AuthError;
pub use character::CharacterRepository;
pub use character_outcome_dto::{CancelDeleteOutcome, CreateCharacterError, DeleteReserveOutcome};
pub use error::RepositoryError;
pub use inventory::InventoryRepository;
pub use play_state::PlayState;
