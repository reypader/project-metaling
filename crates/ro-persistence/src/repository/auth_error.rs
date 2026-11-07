use serde::{Deserialize, Serialize};

use super::RepositoryError;

/// Authentication error variants matching Ragnarok Online wire error codes.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthError {
    #[error("Unregistered ID")]
    UnregisteredId,

    #[error("Incorrect password")]
    IncorrectPassword,

    #[error("Account expired")]
    AccountExpired,

    #[error("Account banned or blocked")]
    AccountBanned,

    #[error("Server under maintenance")]
    ServerMaintenance,

    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Internal repository failure: {0}")]
    Internal(String),
}

impl AuthError {
    /// Maps the authentication failure to standard Ragnarok Online wire error codes.
    pub fn wire_error_code(&self) -> u8 {
        match self {
            Self::UnregisteredId => 0,
            Self::IncorrectPassword => 1,
            Self::AccountExpired => 2,
            Self::AccountBanned => 4,
            Self::ServerMaintenance => 7,
            Self::Repository(_) | Self::Internal(_) => 3,
        }
    }
}
