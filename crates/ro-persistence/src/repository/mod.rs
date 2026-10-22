//! Repository contracts the servers depend on, implemented by `PostgresRepo` and by test doubles.

use serde::{Deserialize, Serialize};

mod account;
mod character;

pub use account::{AccountRepository, AuthError};
pub use character::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, DeleteReserveOutcome,
};

/// Standard error variants returned by persistence repositories.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Ownership mismatch or forbidden access")]
    Forbidden,

    #[error("Resource conflict: {0}")]
    Conflict(String),

    #[error("Identifier space exhausted: {0}")]
    Exhausted(String),

    #[error("Repository backend unavailable: {0}")]
    Unavailable(String),

    #[error("Internal repository failure: {0}")]
    Internal(String),
}
