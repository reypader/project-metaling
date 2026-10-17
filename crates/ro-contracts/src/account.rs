//! Account entities, authentication error types, and repository traits.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, Sex};

/// Status state of a player account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountState {
    Active,
    Banned,
    Expired,
    Locked,
}

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
            Self::Internal(_) => 3,
        }
    }
}

/// Data transfer object representing an account record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub password_hash: String,
    pub sex: Sex,
    pub email: String,
    pub group_id: i32,
    pub state: AccountState,
    pub unban_time: Option<u64>,
    pub expiration_time: Option<u64>,
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
