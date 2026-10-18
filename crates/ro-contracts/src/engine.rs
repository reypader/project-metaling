//! Engine error types and session lifecycle contracts.

use crate::ids::{AccountId, CharId, SessionId};

/// Errors returned by the map engine during session and entity lifecycle interactions.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    #[error("Session not found: {0:?}")]
    UnknownSession(SessionId),

    #[error("Account not found: {0:?}")]
    UnknownAccount(AccountId),

    #[error("Character not found: {0:?}")]
    UnknownCharacter(CharId),

    #[error("Entity not found: {0}")]
    EntityNotFound(u32),

    #[error("Invalid coordinate cell ({x}, {y}) on map")]
    InvalidCell { x: u16, y: u16 },

    #[error("Path could not be found to destination ({dest_x}, {dest_y})")]
    PathNotFound { dest_x: u16, dest_y: u16 },

    #[error("Client intent rejected: {0}")]
    IntentRejected(&'static str),
}
