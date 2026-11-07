//! Account repository contract.

use async_trait::async_trait;

use ro_core::{Account, AccountId};

use super::AuthError;

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
