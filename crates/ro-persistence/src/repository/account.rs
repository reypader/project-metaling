//! Account repository contract and authentication errors.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use ro_core::{Account, AccountId};

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

/// Asynchronous repository contract for account authentication and queries.
#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// Authenticates a user by username and password.
    async fn authenticate(&self, username: &str, password: &str) -> Result<Account, AuthError>;

    /// Fetches an account record by unique account ID.
    async fn find_by_id(&self, account_id: AccountId) -> Result<Option<Account>, AuthError>;

    /// Fetches an account record by username.
    async fn find_by_username(&self, username: &str) -> Result<Option<Account>, AuthError>;
}
