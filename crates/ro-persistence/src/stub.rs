//! In-memory stub repository for accounts and character data.

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rustc_hash::FxHashMap;

use ro_contracts::account::{Account, AccountRepository, AccountState, AuthError};
use ro_contracts::character::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, CreateCharacterRequest,
    DeleteReserveOutcome,
};
use ro_contracts::ids::{AccountId, CharId, CharacterSummary, MapPosition, Sex};
use ro_contracts::repository::RepositoryError;

#[derive(Debug)]
struct StubStorage {
    accounts_by_id: FxHashMap<AccountId, Account>,
    accounts_by_username: FxHashMap<String, AccountId>,
    characters_by_id: FxHashMap<CharId, CharacterSummary>,
    next_char_id: u32,
}

impl Default for StubStorage {
    fn default() -> Self {
        Self {
            accounts_by_id: FxHashMap::default(),
            accounts_by_username: FxHashMap::default(),
            characters_by_id: FxHashMap::default(),
            next_char_id: 150001,
        }
    }
}

/// In-memory repository implementing contract traits for testing and development.
#[derive(Debug)]
pub struct InMemoryStubRepo {
    storage: RwLock<StubStorage>,
}

impl InMemoryStubRepo {
    /// Creates a new stub repository pre-populated with the default test account and character.
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

        repo.insert_character(CharacterSummary {
            char_id: CharId(150000),
            account_id: AccountId(2000000),
            slot: 0,
            name: "Novice".to_string(),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            job_level: 1,
            hair_style: 1,
            hair_color: 0,
            last_map: "prontera.gat".to_string(),
            last_pos: MapPosition {
                x: 156,
                y: 191,
                dir: 0,
            },
            delete_date: None,
        })
        .expect("default character seed failed");

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

    /// Inserts or updates a character summary in the repository.
    pub fn insert_character(&self, character: CharacterSummary) -> Result<(), RepositoryError> {
        let mut storage = self
            .storage
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if character.char_id.0 >= storage.next_char_id {
            storage.next_char_id = character
                .char_id
                .0
                .checked_add(1)
                .ok_or_else(|| RepositoryError::Exhausted("Character ID space exhausted".into()))?;
        }
        storage.characters_by_id.insert(character.char_id, character);
        Ok(())
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

#[async_trait]
impl CharacterRepository for InMemoryStubRepo {
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError> {
        let storage = self
            .storage
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut characters: Vec<CharacterSummary> = storage
            .characters_by_id
            .values()
            .filter(|c| c.account_id == account_id)
            .cloned()
            .collect();

        characters.sort_by_key(|c| c.slot);
        Ok(characters)
    }

    async fn find_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> Result<Option<CharacterSummary>, RepositoryError> {
        let storage = self
            .storage
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let character = storage
            .characters_by_id
            .values()
            .find(|c| c.account_id == account_id && c.slot == slot)
            .cloned();

        Ok(character)
    }

    async fn find_by_id(&self, char_id: CharId) -> Result<Option<CharacterSummary>, RepositoryError> {
        let storage = self
            .storage
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        Ok(storage.characters_by_id.get(&char_id).cloned())
    }

    async fn create_character(
        &self,
        req: CreateCharacterRequest,
        max_slots: u8,
    ) -> Result<CharacterSummary, CreateCharacterError> {
        if req.slot >= max_slots {
            return Err(CreateCharacterError::IneligibleSlot);
        }

        if req.sex != Sex::Female && req.sex != Sex::Male {
            return Err(CreateCharacterError::CreationDenied);
        }

        if req.starting_job != 0 && req.starting_job != 4218 {
            return Err(CreateCharacterError::CreationDenied);
        }

        let name_len = req.name.len();
        if !(4..=23).contains(&name_len) {
            return Err(CreateCharacterError::CreationDenied);
        }

        if req.name.starts_with('#') || !req.name.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
            return Err(CreateCharacterError::InvalidNameSymbols);
        }

        if req.name.eq_ignore_ascii_case("server") {
            return Err(CreateCharacterError::NameAlreadyExists);
        }

        let mut storage = self
            .storage
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let duplicate_name = storage
            .characters_by_id
            .values()
            .any(|c| c.name.eq_ignore_ascii_case(&req.name));
        if duplicate_name {
            return Err(CreateCharacterError::NameAlreadyExists);
        }

        let account_characters: Vec<&CharacterSummary> = storage
            .characters_by_id
            .values()
            .filter(|c| c.account_id == req.account_id)
            .collect();

        if account_characters.len() >= max_slots as usize {
            return Err(CreateCharacterError::CreationDenied);
        }

        if account_characters.iter().any(|c| c.slot == req.slot) {
            return Err(CreateCharacterError::CreationDenied);
        }

        let new_char_id = CharId(storage.next_char_id);
        storage.next_char_id = storage
            .next_char_id
            .checked_add(1)
            .ok_or_else(|| RepositoryError::Exhausted("Character ID space exhausted".into()))?;

        let summary = CharacterSummary {
            char_id: new_char_id,
            account_id: req.account_id,
            slot: req.slot,
            name: req.name,
            sex: req.sex,
            job_class: req.starting_job as u16,
            base_level: 1,
            job_level: 1,
            hair_style: req.hair_style,
            hair_color: req.hair_color,
            last_map: "prontera.gat".to_string(),
            last_pos: MapPosition {
                x: 156,
                y: 191,
                dir: 0,
            },
            delete_date: None,
        };

        storage.characters_by_id.insert(new_char_id, summary.clone());
        Ok(summary)
    }

    async fn reserve_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
        delay_seconds: u32,
    ) -> Result<DeleteReserveOutcome, RepositoryError> {
        let mut storage = self
            .storage
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let character = match storage.characters_by_id.get_mut(&char_id) {
            Some(c) => c,
            None => return Ok(DeleteReserveOutcome::NotFoundOrDbError),
        };

        if character.account_id != account_id {
            return Ok(DeleteReserveOutcome::NotFoundOrDbError);
        }

        if character.delete_date.is_some() {
            return Ok(DeleteReserveOutcome::Denied);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        character.delete_date = Some(now.saturating_add(delay_seconds as u64));
        Ok(DeleteReserveOutcome::Success {
            delete_delay_seconds: delay_seconds,
        })
    }

    async fn cancel_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
    ) -> Result<CancelDeleteOutcome, RepositoryError> {
        let mut storage = self
            .storage
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let character = match storage.characters_by_id.get_mut(&char_id) {
            Some(c) => c,
            None => return Ok(CancelDeleteOutcome::NotFound),
        };

        if character.account_id != account_id {
            return Ok(CancelDeleteOutcome::NotFound);
        }

        if character.delete_date.is_some() {
            character.delete_date = None;
            Ok(CancelDeleteOutcome::Success)
        } else {
            Ok(CancelDeleteOutcome::NotReserved)
        }
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

        let account_by_id =
            AccountRepository::find_by_id(&repo, AccountId(2000000)).await.unwrap();
        assert!(account_by_id.is_some());
        let account = account_by_id.unwrap();
        assert_eq!(account.username, "test");

        let account_by_username = repo.find_by_username("test").await.unwrap();
        assert_eq!(account_by_username, Some(account));

        let non_existent_id =
            AccountRepository::find_by_id(&repo, AccountId(9999999)).await.unwrap();
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

        let by_id_original =
            AccountRepository::find_by_id(&repo, AccountId(2000000)).await.unwrap();
        assert!(by_id_original.is_none());

        let by_username_new = repo.find_by_username("test_renamed").await.unwrap();
        assert_eq!(by_username_new, Some(competing_account));
    }

    #[tokio::test]
    async fn test_character_listing_and_lookup() {
        let repo = InMemoryStubRepo::new();
        let characters = repo.list_by_account(AccountId(2000000)).await.unwrap();
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].char_id, CharId(150000));
        assert_eq!(characters[0].name, "Novice");
        assert_eq!(characters[0].slot, 0);

        let by_slot = repo.find_by_slot(AccountId(2000000), 0).await.unwrap();
        assert_eq!(by_slot, Some(characters[0].clone()));

        let by_missing_slot = repo.find_by_slot(AccountId(2000000), 1).await.unwrap();
        assert!(by_missing_slot.is_none());

        let by_id =
            CharacterRepository::find_by_id(&repo, CharId(150000)).await.unwrap();
        assert_eq!(by_id, Some(characters[0].clone()));

        let by_missing_id =
            CharacterRepository::find_by_id(&repo, CharId(999999)).await.unwrap();
        assert!(by_missing_id.is_none());
    }

    #[tokio::test]
    async fn test_create_character_success() {
        let repo = InMemoryStubRepo::new();
        let req = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "SecondChar".to_string(),
            slot: 1,
            hair_color: 2,
            hair_style: 3,
            starting_job: 0,
            sex: Sex::Male,
        };

        let summary = repo.create_character(req, 9).await.unwrap();
        assert_eq!(summary.char_id, CharId(150001));
        assert_eq!(summary.slot, 1);
        assert_eq!(summary.name, "SecondChar");
        assert_eq!(summary.job_class, 0);
        assert_eq!(summary.base_level, 1);
        assert_eq!(summary.job_level, 1);
        assert_eq!(summary.hair_color, 2);
        assert_eq!(summary.hair_style, 3);
        assert_eq!(summary.last_map, "prontera.gat");
        assert_eq!(summary.last_pos.x, 156);
        assert_eq!(summary.last_pos.y, 191);
        assert!(summary.delete_date.is_none());

        let listed = repo.list_by_account(AccountId(2000000)).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].slot, 0);
        assert_eq!(listed[1].slot, 1);
    }

    #[tokio::test]
    async fn test_create_character_doram_success() {
        let repo = InMemoryStubRepo::new();
        let req = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "DoramCat".to_string(),
            slot: 2,
            hair_color: 1,
            hair_style: 1,
            starting_job: 4218,
            sex: Sex::Female,
        };

        let summary = repo.create_character(req, 9).await.unwrap();
        assert_eq!(summary.char_id, CharId(150001));
        assert_eq!(summary.job_class, 4218);
        assert_eq!(summary.sex, Sex::Female);
    }

    #[tokio::test]
    async fn test_create_character_validation_failures() {
        let repo = InMemoryStubRepo::new();

        let req_slot_out_of_bounds = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "ValidName".to_string(),
            slot: 9,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_slot_out_of_bounds, 9).await.unwrap_err(),
            CreateCharacterError::IneligibleSlot
        );

        let req_server_sex = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "ValidName".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Server,
        };
        assert_eq!(
            repo.create_character(req_server_sex, 9).await.unwrap_err(),
            CreateCharacterError::CreationDenied
        );

        let req_invalid_job = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "ValidName".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 1,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_invalid_job, 9).await.unwrap_err(),
            CreateCharacterError::CreationDenied
        );

        let req_short_name = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "abc".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_short_name, 9).await.unwrap_err(),
            CreateCharacterError::CreationDenied
        );

        let req_long_name = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "abcdefghijklmnopqrstuvwx".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_long_name, 9).await.unwrap_err(),
            CreateCharacterError::CreationDenied
        );

        let req_hash_name = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "#Invalid".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_hash_name, 9).await.unwrap_err(),
            CreateCharacterError::InvalidNameSymbols
        );

        let req_control_char = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "Test\u{0001}Name".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_control_char, 9).await.unwrap_err(),
            CreateCharacterError::InvalidNameSymbols
        );

        let req_reserved_server = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "sErVeR".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_reserved_server, 9).await.unwrap_err(),
            CreateCharacterError::NameAlreadyExists
        );

        let req_duplicate_name = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "nOvIcE".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_duplicate_name, 9).await.unwrap_err(),
            CreateCharacterError::NameAlreadyExists
        );

        let req_occupied_slot = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "AnotherChar".to_string(),
            slot: 0,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_occupied_slot, 9).await.unwrap_err(),
            CreateCharacterError::CreationDenied
        );

        repo.insert_character(CharacterSummary {
            char_id: CharId(150005),
            account_id: AccountId(2000000),
            slot: 3,
            name: "SlotThreeChar".to_string(),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            job_level: 1,
            hair_style: 1,
            hair_color: 0,
            last_map: "prontera.gat".to_string(),
            last_pos: MapPosition {
                x: 156,
                y: 191,
                dir: 0,
            },
            delete_date: None,
        })
        .unwrap();

        let req_max_slots_reached = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "CapacityChar".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        assert_eq!(
            repo.create_character(req_max_slots_reached, 2).await.unwrap_err(),
            CreateCharacterError::CreationDenied
        );
    }

    #[tokio::test]
    async fn test_reserve_and_cancel_deletion() {
        let repo = InMemoryStubRepo::new();

        let outcome = repo
            .reserve_delete(AccountId(2000000), CharId(150000), 86400)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            DeleteReserveOutcome::Success {
                delete_delay_seconds: 86400
            }
        );

        let character =
            CharacterRepository::find_by_id(&repo, CharId(150000)).await.unwrap().unwrap();
        assert!(character.delete_date.is_some());

        let duplicate_outcome = repo
            .reserve_delete(AccountId(2000000), CharId(150000), 86400)
            .await
            .unwrap();
        assert_eq!(duplicate_outcome, DeleteReserveOutcome::Denied);

        let wrong_account_outcome = repo
            .reserve_delete(AccountId(2000001), CharId(150000), 86400)
            .await
            .unwrap();
        assert_eq!(
            wrong_account_outcome,
            DeleteReserveOutcome::NotFoundOrDbError
        );

        let missing_char_outcome = repo
            .reserve_delete(AccountId(2000000), CharId(999999), 86400)
            .await
            .unwrap();
        assert_eq!(
            missing_char_outcome,
            DeleteReserveOutcome::NotFoundOrDbError
        );

        let wrong_account_cancel = repo
            .cancel_delete(AccountId(2000001), CharId(150000))
            .await
            .unwrap();
        assert_eq!(wrong_account_cancel, CancelDeleteOutcome::NotFound);

        let successful_cancel = repo
            .cancel_delete(AccountId(2000000), CharId(150000))
            .await
            .unwrap();
        assert_eq!(successful_cancel, CancelDeleteOutcome::Success);

        let character_after =
            CharacterRepository::find_by_id(&repo, CharId(150000)).await.unwrap().unwrap();
        assert!(character_after.delete_date.is_none());

        let redundant_cancel = repo
            .cancel_delete(AccountId(2000000), CharId(150000))
            .await
            .unwrap();
        assert_eq!(redundant_cancel, CancelDeleteOutcome::NotReserved);
    }

    #[tokio::test]
    async fn test_insert_character_overflow() {
        let repo = InMemoryStubRepo::new();
        let character = CharacterSummary {
            char_id: CharId(u32::MAX),
            account_id: AccountId(2000000),
            slot: 2,
            name: "MaxChar".to_string(),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            job_level: 1,
            hair_style: 1,
            hair_color: 0,
            last_map: "prontera.gat".to_string(),
            last_pos: MapPosition {
                x: 156,
                y: 191,
                dir: 0,
            },
            delete_date: None,
        };

        let err = repo.insert_character(character).unwrap_err();
        assert_eq!(
            err,
            RepositoryError::Exhausted("Character ID space exhausted".into())
        );
    }

    #[tokio::test]
    async fn test_create_character_overflow() {
        let repo = InMemoryStubRepo::new();
        {
            let mut storage = repo.storage.write().unwrap();
            storage.next_char_id = u32::MAX;
        }

        let req = CreateCharacterRequest {
            account_id: AccountId(2000000),
            name: "NextOverflow".to_string(),
            slot: 1,
            hair_color: 0,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };

        let err = repo.create_character(req, 9).await.unwrap_err();
        assert_eq!(
            err,
            CreateCharacterError::Repository(RepositoryError::Exhausted(
                "Character ID space exhausted".into()
            ))
        );
    }
}
