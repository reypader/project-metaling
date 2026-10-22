//! Shared repository error types and outcomes.

use serde::{Deserialize, Serialize};

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
