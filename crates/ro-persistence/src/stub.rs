//! In-memory stub repository for accounts and character data.

use std::sync::RwLock;

use async_trait::async_trait;
use rustc_hash::FxHashMap;

use ro_contracts::account::{Account, AccountRepository, AccountState, AuthError};
use ro_contracts::ids::{AccountId, Sex};

#[derive(Debug, Default)]
struct StubStorage {
    accounts_by_id: FxHashMap<AccountId, Account>,
    accounts_by_username: FxHashMap<String, AccountId>,
}

/// In-memory repository implementing contract traits for testing and development.
#[derive(Debug)]
pub struct InMemoryStubRepo {
    storage: RwLock<StubStorage>,
}

impl InMemoryStubRepo {
    /// Creates a new stub repository pre-populated with the default test account.
    pub fn new() -> Self {
        let repo = Self {
            storage: RwLock::new(StubStorage::default()),
        };

        repo.insert_account(Account {
            id: AccountId(2000000),
            username: "test".to_string(),
            password_hash: "password".to_string(),
            sex: Sex::Male,
            email: "test@example.com".to_string(),
            group_id: 0,
            state: AccountState::Active,
            unban_time: None,
            expiration_time: None,
        });

        repo
    }

    /// Inserts or updates an account in the repository.
    pub fn insert_account(&self, account: Account) {
        let mut storage = self
            .storage
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let existing_id = storage.accounts_by_username.get(&account.username).copied();
        if let Some(existing_id) = existing_id {
            if existing_id != account.id {
                storage.accounts_by_id.remove(&existing_id);
            }
        }

        let old_username = storage
            .accounts_by_id
            .get(&account.id)
            .map(|a| a.username.clone());
        if let Some(old_username) = old_username {
            storage.accounts_by_username.remove(&old_username);
        }

        storage
            .accounts_by_username
            .insert(account.username.clone(), account.id);
        storage.accounts_by_id.insert(account.id, account);
    }
}

impl Default for InMemoryStubRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AccountRepository for InMemoryStubRepo {
    async fn authenticate(&self, username: &str, password: &str) -> Result<Account, AuthError> {
        let storage = self
            .storage
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let account_id = storage
            .accounts_by_username
            .get(username)
            .copied()
            .ok_or(AuthError::UnregisteredId)?;

        let account = storage
            .accounts_by_id
            .get(&account_id)
            .cloned()
            .ok_or(AuthError::UnregisteredId)?;

        match account.state {
            AccountState::Banned | AccountState::Locked => return Err(AuthError::AccountBanned),
            AccountState::Expired => return Err(AuthError::AccountExpired),
            AccountState::Active => {}
        }

        if account.password_hash != password {
            return Err(AuthError::IncorrectPassword);
        }

        Ok(account)
    }

    async fn find_by_id(&self, account_id: AccountId) -> Result<Option<Account>, AuthError> {
        let storage = self
            .storage
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        Ok(storage.accounts_by_id.get(&account_id).cloned())
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<Account>, AuthError> {
        let storage = self
            .storage
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let maybe_account = storage
            .accounts_by_username
            .get(username)
            .and_then(|id| storage.accounts_by_id.get(id))
            .cloned();

        Ok(maybe_account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_successful_authentication() {
        let repo = InMemoryStubRepo::new();
        let account = repo.authenticate("test", "password").await.unwrap();

        assert_eq!(account.id, AccountId(2000000));
        assert_eq!(account.username, "test");
        assert_eq!(account.sex, Sex::Male);
        assert_eq!(account.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_failed_authentication_incorrect_password() {
        let repo = InMemoryStubRepo::new();
        let err = repo.authenticate("test", "passwords").await.unwrap_err();

        assert_eq!(err, AuthError::IncorrectPassword);
        assert_eq!(err.wire_error_code(), 1);
    }

    #[tokio::test]
    async fn test_failed_authentication_unregistered_id() {
        let repo = InMemoryStubRepo::new();
        let err = repo
            .authenticate("unknown_user", "password")
            .await
            .unwrap_err();

        assert_eq!(err, AuthError::UnregisteredId);
        assert_eq!(err.wire_error_code(), 0);
    }

    #[tokio::test]
    async fn test_account_states_banned_locked_expired() {
        let repo = InMemoryStubRepo::new();

        repo.insert_account(Account {
            id: AccountId(2000001),
            username: "banned_user".to_string(),
            password_hash: "password".to_string(),
            sex: Sex::Female,
            email: "banned@example.com".to_string(),
            group_id: 0,
            state: AccountState::Banned,
            unban_time: None,
            expiration_time: None,
        });

        repo.insert_account(Account {
            id: AccountId(2000002),
            username: "locked_user".to_string(),
            password_hash: "password".to_string(),
            sex: Sex::Male,
            email: "locked@example.com".to_string(),
            group_id: 0,
            state: AccountState::Locked,
            unban_time: None,
            expiration_time: None,
        });

        repo.insert_account(Account {
            id: AccountId(2000003),
            username: "expired_user".to_string(),
            password_hash: "password".to_string(),
            sex: Sex::Female,
            email: "expired@example.com".to_string(),
            group_id: 0,
            state: AccountState::Expired,
            unban_time: None,
            expiration_time: None,
        });

        let banned_err = repo
            .authenticate("banned_user", "password")
            .await
            .unwrap_err();
        assert_eq!(banned_err, AuthError::AccountBanned);
        assert_eq!(banned_err.wire_error_code(), 4);

        let locked_err = repo
            .authenticate("locked_user", "password")
            .await
            .unwrap_err();
        assert_eq!(locked_err, AuthError::AccountBanned);
        assert_eq!(locked_err.wire_error_code(), 4);

        let expired_err = repo
            .authenticate("expired_user", "password")
            .await
            .unwrap_err();
        assert_eq!(expired_err, AuthError::AccountExpired);
        assert_eq!(expired_err.wire_error_code(), 2);
    }

    #[tokio::test]
    async fn test_find_by_id_and_username() {
        let repo = InMemoryStubRepo::new();

        let account_by_id = repo.find_by_id(AccountId(2000000)).await.unwrap();
        assert!(account_by_id.is_some());
        let account = account_by_id.unwrap();
        assert_eq!(account.username, "test");

        let account_by_username = repo.find_by_username("test").await.unwrap();
        assert_eq!(account_by_username, Some(account));

        let non_existent_id = repo.find_by_id(AccountId(9999999)).await.unwrap();
        assert!(non_existent_id.is_none());

        let non_existent_user = repo.find_by_username("non_existent").await.unwrap();
        assert!(non_existent_user.is_none());
    }

    #[tokio::test]
    async fn test_insert_account_index_synchronization() {
        let repo = InMemoryStubRepo::new();

        let updated_account = Account {
            id: AccountId(2000000),
            username: "test_renamed".to_string(),
            password_hash: "password".to_string(),
            sex: Sex::Male,
            email: "test@example.com".to_string(),
            group_id: 0,
            state: AccountState::Active,
            unban_time: None,
            expiration_time: None,
        };
        repo.insert_account(updated_account.clone());

        let old_lookup = repo.find_by_username("test").await.unwrap();
        assert!(old_lookup.is_none());

        let new_lookup = repo.find_by_username("test_renamed").await.unwrap();
        assert_eq!(new_lookup, Some(updated_account.clone()));

        let competing_account = Account {
            id: AccountId(2000010),
            username: "test_renamed".to_string(),
            password_hash: "secret".to_string(),
            sex: Sex::Female,
            email: "other@example.com".to_string(),
            group_id: 0,
            state: AccountState::Active,
            unban_time: None,
            expiration_time: None,
        };
        repo.insert_account(competing_account.clone());

        let by_id_original = repo.find_by_id(AccountId(2000000)).await.unwrap();
        assert!(by_id_original.is_none());

        let by_username_new = repo.find_by_username("test_renamed").await.unwrap();
        assert_eq!(by_username_new, Some(competing_account));
    }
}
