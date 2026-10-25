//! Integration tests for PostgresRepo implementing test scenarios PERSIST-AUTH-001 through PERSIST-SCHEMA-002.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::sync::atomic::{AtomicU32, Ordering};

use ro_core::{Account, AccountId, AccountState, CharId, CreateCharacterRequest, MapPosition, Sex};
use ro_persistence::{AccountRepository, AuthError, PostgresRepo, RepositoryError};
use ro_persistence::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, DeleteReserveOutcome,
};

static ACCOUNT_COUNTER: std::sync::OnceLock<AtomicU32> = std::sync::OnceLock::new();

fn next_account_id() -> AccountId {
    let counter = ACCOUNT_COUNTER.get_or_init(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32;
        let base = 2_100_000 + (now % 1_000_000) * 1000;
        AtomicU32::new(base)
    });
    AccountId(counter.fetch_add(1, Ordering::SeqCst))
}

fn test_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string())
}

async fn get_repo() -> PostgresRepo {
    PostgresRepo::connect(&test_db_url())
        .await
        .expect("failed to connect to test database")
}

async fn create_test_account(repo: &PostgresRepo, state: AccountState) -> Account {
    let id = next_account_id();
    let _ = sqlx::query("DELETE FROM characters WHERE account_id = $1")
        .bind(id.0 as i32)
        .execute(repo.pool())
        .await;
    let _ = sqlx::query("DELETE FROM accounts WHERE account_id = $1")
        .bind(id.0 as i32)
        .execute(repo.pool())
        .await;

    let username = format!("u_{}", id.0);
    let account = Account {
        id,
        username: username.clone(),
        password_hash: "password".to_string(),
        sex: Sex::Male,
        email: format!("{}@example.com", username),
        group_id: 0,
        state,
        unban_time: None,
        expiration_time: None,
    };
    repo.insert_account(&account)
        .await
        .expect("account insertion failed");
    account
}

#[tokio::test]
async fn test_persist_auth_001_valid_credentials() {
    let repo = get_repo().await;
    let account = repo.authenticate("test", "password").await.unwrap();

    assert_eq!(account.id, AccountId(2000000));
    assert_eq!(account.username, "test");
    assert_eq!(account.sex, Sex::Male);
    assert_eq!(account.email, "test@example.com");
    assert_eq!(account.state, AccountState::Active);
}

#[tokio::test]
async fn test_persist_auth_002_invalid_password() {
    let repo = get_repo().await;
    let err = repo
        .authenticate("test", "wrong_password")
        .await
        .unwrap_err();

    assert_eq!(err, AuthError::IncorrectPassword);
    assert_eq!(err.wire_error_code(), 1);
}

#[tokio::test]
async fn test_persist_auth_003_unregistered_username() {
    let repo = get_repo().await;
    let err = repo
        .authenticate("unregistered_user_99999", "password")
        .await
        .unwrap_err();

    assert_eq!(err, AuthError::UnregisteredId);
    assert_eq!(err.wire_error_code(), 0);
}

#[tokio::test]
async fn test_persist_auth_004_state_gating() {
    let repo = get_repo().await;

    let banned_acc = create_test_account(&repo, AccountState::Banned).await;
    let banned_err = repo
        .authenticate(&banned_acc.username, "password")
        .await
        .unwrap_err();
    assert_eq!(banned_err, AuthError::AccountBanned);
    assert_eq!(banned_err.wire_error_code(), 4);

    let locked_acc = create_test_account(&repo, AccountState::Locked).await;
    let locked_err = repo
        .authenticate(&locked_acc.username, "password")
        .await
        .unwrap_err();
    assert_eq!(locked_err, AuthError::AccountBanned);
    assert_eq!(locked_err.wire_error_code(), 4);

    let expired_acc = create_test_account(&repo, AccountState::Expired).await;
    let expired_err = repo
        .authenticate(&expired_acc.username, "password")
        .await
        .unwrap_err();
    assert_eq!(expired_err, AuthError::AccountExpired);
    assert_eq!(expired_err.wire_error_code(), 2);
}

#[tokio::test]
async fn test_persist_auth_005_find_by_id_and_username() {
    let repo = get_repo().await;

    let by_id = AccountRepository::find_by_id(&repo, AccountId(2000000))
        .await
        .unwrap();
    assert!(by_id.is_some());
    let account = by_id.unwrap();
    assert_eq!(account.username, "test");

    let by_missing_id = AccountRepository::find_by_id(&repo, AccountId(9999999))
        .await
        .unwrap();
    assert!(by_missing_id.is_none());

    let by_username = repo.find_by_username("test").await.unwrap();
    assert_eq!(by_username, Some(account));

    let by_missing_username = repo
        .find_by_username("non_existent_username")
        .await
        .unwrap();
    assert!(by_missing_username.is_none());
}

#[tokio::test]
async fn test_persist_char_001_list_by_account_sorted() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req_slot_2 = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("CharSlot2_{}", acc.id.0),
        slot: 2,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let req_slot_0 = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("CharSlot0_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let req_slot_1 = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("CharSlot1_{}", acc.id.0),
        slot: 1,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };

    repo.create_character(req_slot_2, 9).await.unwrap();
    repo.create_character(req_slot_0, 9).await.unwrap();
    repo.create_character(req_slot_1, 9).await.unwrap();

    let list = repo.list_by_account(acc.id).await.unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].slot, 0);
    assert_eq!(list[1].slot, 1);
    assert_eq!(list[2].slot, 2);
}

#[tokio::test]
async fn test_persist_char_002_list_empty_account() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let list = repo.list_by_account(acc.id).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn test_persist_char_003_find_by_slot_and_id() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("SlotFinder_{}", acc.id.0),
        slot: 1,
        hair_color: 2,
        hair_style: 3,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    let by_slot = repo.find_by_slot(acc.id, 1).await.unwrap();
    assert_eq!(by_slot, Some(created.clone()));

    let by_missing_slot = repo.find_by_slot(acc.id, 0).await.unwrap();
    assert!(by_missing_slot.is_none());

    let by_id = CharacterRepository::find_by_id(&repo, created.char_id)
        .await
        .unwrap();
    assert_eq!(by_id, Some(created));

    let by_missing_id = CharacterRepository::find_by_id(&repo, CharId(9999999))
        .await
        .unwrap();
    assert!(by_missing_id.is_none());
}

#[tokio::test]
async fn test_persist_char_004_create_novice() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("Novice_{}", acc.id.0),
        slot: 0,
        hair_color: 1,
        hair_style: 2,
        starting_job: 0,
        sex: Sex::Male,
    };

    let summary = repo.create_character(req, 9).await.unwrap();
    assert!(summary.char_id.0 >= 150000);
    assert_eq!(summary.account_id, acc.id);
    assert_eq!(summary.slot, 0);
    assert_eq!(summary.job_class, 0);
    assert_eq!(summary.base_level, 1);
    assert_eq!(summary.job_level, 1);
    assert_eq!(summary.last_map, "prontera.gat");
    assert_eq!(summary.last_pos.x, 156);
    assert_eq!(summary.last_pos.y, 191);
    assert_eq!(summary.last_pos.dir, 0);
    assert!(summary.delete_date.is_none());
}

#[tokio::test]
async fn test_persist_char_005_create_doram() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("Doram_{}", acc.id.0),
        slot: 0,
        hair_color: 3,
        hair_style: 4,
        starting_job: 4218,
        sex: Sex::Female,
    };

    let summary = repo.create_character(req, 9).await.unwrap();
    assert_eq!(summary.job_class, 4218);
    assert_eq!(summary.sex, Sex::Female);
}

#[tokio::test]
async fn test_persist_char_006_unique_name_conflict() {
    let repo = get_repo().await;
    let acc1 = create_test_account(&repo, AccountState::Active).await;
    let acc2 = create_test_account(&repo, AccountState::Active).await;

    let char_name = format!("Conflict_{}", acc1.id.0);

    let req1 = CreateCharacterRequest {
        account_id: acc1.id,
        name: char_name.clone(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    repo.create_character(req1, 9).await.unwrap();

    let req2 = CreateCharacterRequest {
        account_id: acc2.id,
        name: char_name.clone(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let err = repo.create_character(req2, 9).await.unwrap_err();
    assert_eq!(err, CreateCharacterError::NameAlreadyExists);

    let req3 = CreateCharacterRequest {
        account_id: acc2.id,
        name: char_name.to_uppercase(),
        slot: 1,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let err_case = repo.create_character(req3, 9).await.unwrap_err();
    assert_eq!(err_case, CreateCharacterError::NameAlreadyExists);
}

#[tokio::test]
async fn test_persist_char_007_reserved_name_rejection() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: "sErVeR".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let err = repo.create_character(req, 9).await.unwrap_err();
    assert_eq!(err, CreateCharacterError::NameAlreadyExists);
}

#[tokio::test]
async fn test_persist_char_008_slot_boundary_and_conflict() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req_out_of_bounds = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("OutOfBounds_{}", acc.id.0),
        slot: 9,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let err_bounds = repo
        .create_character(req_out_of_bounds, 9)
        .await
        .unwrap_err();
    assert_eq!(err_bounds, CreateCharacterError::IneligibleSlot);

    let req_slot_0 = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("Occupied_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    repo.create_character(req_slot_0, 9).await.unwrap();

    let req_slot_0_dup = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("DupSlot_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let err_dup = repo.create_character(req_slot_0_dup, 9).await.unwrap_err();
    assert_eq!(err_dup, CreateCharacterError::CreationDenied);

    let req_slot_2 = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("SlotTwo_{}", acc.id.0),
        slot: 2,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    repo.create_character(req_slot_2, 9).await.unwrap();

    let req_capacity_exceeded = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("Capacity_{}", acc.id.0),
        slot: 1,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let err_cap = repo
        .create_character(req_capacity_exceeded, 2)
        .await
        .unwrap_err();
    assert_eq!(err_cap, CreateCharacterError::CreationDenied);
}

#[tokio::test]
async fn test_persist_char_009_name_and_symbol_validation() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req_short = CreateCharacterRequest {
        account_id: acc.id,
        name: "abc".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    assert_eq!(
        repo.create_character(req_short, 9).await.unwrap_err(),
        CreateCharacterError::CreationDenied
    );

    let req_long = CreateCharacterRequest {
        account_id: acc.id,
        name: "abcdefghijklmnopqrstuvwx".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    assert_eq!(
        repo.create_character(req_long, 9).await.unwrap_err(),
        CreateCharacterError::CreationDenied
    );

    let req_space = CreateCharacterRequest {
        account_id: acc.id,
        name: " BadName".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    assert_eq!(
        repo.create_character(req_space, 9).await.unwrap_err(),
        CreateCharacterError::InvalidNameSymbols
    );

    let req_hash = CreateCharacterRequest {
        account_id: acc.id,
        name: "#BadName".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    assert_eq!(
        repo.create_character(req_hash, 9).await.unwrap_err(),
        CreateCharacterError::InvalidNameSymbols
    );

    let req_control = CreateCharacterRequest {
        account_id: acc.id,
        name: "Bad\u{0001}Char".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    assert_eq!(
        repo.create_character(req_control, 9).await.unwrap_err(),
        CreateCharacterError::InvalidNameSymbols
    );

    let req_server_sex = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("ValidName_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Server,
    };
    assert_eq!(
        repo.create_character(req_server_sex, 9).await.unwrap_err(),
        CreateCharacterError::CreationDenied
    );

    let req_bad_job = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("ValidJob_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 1,
        sex: Sex::Male,
    };
    assert_eq!(
        repo.create_character(req_bad_job, 9).await.unwrap_err(),
        CreateCharacterError::CreationDenied
    );
}

#[tokio::test]
async fn test_persist_char_010_deletion_reservation() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("DeleteRes_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    let outcome = repo
        .reserve_delete(acc.id, created.char_id, 86400)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        DeleteReserveOutcome::Success {
            delete_delay_seconds: 86400
        }
    );

    let updated = CharacterRepository::find_by_id(&repo, created.char_id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.delete_date.is_some());
}

#[tokio::test]
async fn test_persist_char_011_invalid_reservation_rejection() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;
    let other_acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("InvRes_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    let outcome = repo
        .reserve_delete(acc.id, created.char_id, 86400)
        .await
        .unwrap();
    assert!(matches!(outcome, DeleteReserveOutcome::Success { .. }));

    let duplicate_outcome = repo
        .reserve_delete(acc.id, created.char_id, 86400)
        .await
        .unwrap();
    assert_eq!(duplicate_outcome, DeleteReserveOutcome::Denied);

    let wrong_acc_outcome = repo
        .reserve_delete(other_acc.id, created.char_id, 86400)
        .await
        .unwrap();
    assert_eq!(wrong_acc_outcome, DeleteReserveOutcome::NotFoundOrDbError);

    let missing_char_outcome = repo
        .reserve_delete(acc.id, CharId(9999999), 86400)
        .await
        .unwrap();
    assert_eq!(
        missing_char_outcome,
        DeleteReserveOutcome::NotFoundOrDbError
    );
}

#[tokio::test]
async fn test_persist_char_012_cancellation_success() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("CancelSuccess_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    repo.reserve_delete(acc.id, created.char_id, 86400)
        .await
        .unwrap();

    let outcome = repo.cancel_delete(acc.id, created.char_id).await.unwrap();
    assert_eq!(outcome, CancelDeleteOutcome::Success);

    let updated = CharacterRepository::find_by_id(&repo, created.char_id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.delete_date.is_none());
}

#[tokio::test]
async fn test_persist_char_013_unreserved_cancellation_rejection() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;
    let other_acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("UnresCancel_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    let not_reserved_outcome = repo.cancel_delete(acc.id, created.char_id).await.unwrap();
    assert_eq!(not_reserved_outcome, CancelDeleteOutcome::NotReserved);

    let wrong_acc_outcome = repo
        .cancel_delete(other_acc.id, created.char_id)
        .await
        .unwrap();
    assert_eq!(wrong_acc_outcome, CancelDeleteOutcome::NotFound);

    let missing_char_outcome = repo.cancel_delete(acc.id, CharId(9999999)).await.unwrap();
    assert_eq!(missing_char_outcome, CancelDeleteOutcome::NotFound);
}

#[tokio::test]
async fn test_persist_char_014_coordinate_persistence() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("CoordPersist_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    let target_pos = MapPosition {
        x: 120,
        y: 100,
        dir: 4,
    };
    repo.update_position(created.char_id, "geffen.gat", target_pos)
        .await
        .unwrap();

    let updated = CharacterRepository::find_by_id(&repo, created.char_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.last_map, "geffen.gat");
    assert_eq!(updated.last_pos.x, 120);
    assert_eq!(updated.last_pos.y, 100);
    assert_eq!(updated.last_pos.dir, 4);
}

#[tokio::test]
async fn test_persist_char_015_absent_character_update_position() {
    let repo = get_repo().await;
    let err = repo
        .update_position(
            CharId(9999999),
            "morocc.gat",
            MapPosition {
                x: 50,
                y: 50,
                dir: 0,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(err, RepositoryError::NotFound);
}

#[tokio::test]
async fn test_persist_char_016_consecutive_hot_updates() {
    let repo = get_repo().await;
    let acc = create_test_account(&repo, AccountState::Active).await;

    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("HotUpdates_{}", acc.id.0),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo.create_character(req, 9).await.unwrap();

    let checkpoints = [
        (
            "prontera.gat",
            MapPosition {
                x: 160,
                y: 190,
                dir: 1,
            },
        ),
        (
            "prontera.gat",
            MapPosition {
                x: 165,
                y: 185,
                dir: 2,
            },
        ),
        (
            "payon.gat",
            MapPosition {
                x: 100,
                y: 100,
                dir: 3,
            },
        ),
        (
            "alberta.gat",
            MapPosition {
                x: 50,
                y: 60,
                dir: 4,
            },
        ),
        (
            "morocc.gat",
            MapPosition {
                x: 150,
                y: 150,
                dir: 0,
            },
        ),
    ];

    for (map, pos) in checkpoints {
        repo.update_position(created.char_id, map, pos)
            .await
            .unwrap();
        let current = CharacterRepository::find_by_id(&repo, created.char_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.last_map, map);
        assert_eq!(current.last_pos.x, pos.x);
        assert_eq!(current.last_pos.y, pos.y);
        assert_eq!(current.last_pos.dir, pos.dir);
    }
}

#[tokio::test]
async fn test_persist_schema_001_schema_initialization_idempotency() {
    let repo = get_repo().await;

    // Execute schema initialization consecutively to verify DDL idempotency
    repo.init_schema().await.unwrap();
    repo.init_schema().await.unwrap();

    // Verify accounts table exists and has proper columns
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert!(count >= 1);

    // Verify characters table exists
    let char_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert!(char_count >= 0);
}

#[tokio::test]
async fn test_persist_schema_002_connection_error_handling() {
    let invalid_url = "postgres://ragnarok:ragnarok@localhost:5433/ragnarok";
    let result = PostgresRepo::connect(invalid_url).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, RepositoryError::Unavailable(_)));
}

// ---- Inventory (spec PER-INV-01..04) ----

use ro_core::{
    CharacterInventory, CharacterLooks, CharacterStatus, InventoryItem, ItemId, ItemOption,
};
use ro_persistence::InventoryRepository;

/// A schema-initialized repo (the inventory table and counter column may be new) and a fresh
/// character on a fresh account.
async fn inventory_fixture(group_id: i32) -> (PostgresRepo, Account, CharId) {
    let repo = get_repo().await;
    repo.init_schema().await.expect("schema applies");
    let mut acc = create_test_account(&repo, AccountState::Active).await;
    acc.group_id = group_id;
    repo.insert_account(&acc).await.expect("group update");
    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("Inv_{}", acc.id.0),
        slot: 0,
        hair_color: 1,
        hair_style: 2,
        starting_job: 0,
        sex: Sex::Male,
    };
    let summary = repo.create_character(req, 9).await.expect("character");
    (repo, acc, summary.char_id)
}

fn full_item() -> InventoryItem {
    InventoryItem {
        item_id: ItemId(u32::MAX),
        amount: 30000,
        equip: u32::MAX,
        identified: false,
        refine: 20,
        attribute: 1,
        cards: [u32::MAX, 0xFF, 4001, 1],
        options: [
            ItemOption {
                id: i16::MAX,
                value: i16::MIN,
                param: u8::MAX,
            },
            ItemOption {
                id: 1,
                value: -1,
                param: 0,
            },
            ItemOption::default(),
            ItemOption::default(),
            ItemOption {
                id: 5,
                value: 5,
                param: 5,
            },
        ],
        expire_time: u32::MAX,
        favorite: true,
        bound: 4,
        unique_id: 0x8000_0001_0000_0002,
        equip_switch: 0x8000_0000,
        enchant_grade: 4,
    }
}

/// PER-INV-01: a round trip keeps every field, including u32::MAX cards and a u64 unique_id
/// with the top bit set.
#[tokio::test]
async fn per_inv_01_round_trip_keeps_every_field() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let inventory = CharacterInventory {
        slots: vec![Some(full_item()), Some(InventoryItem::new(ItemId(501), 5))],
        unique_item_counter: 7,
    };
    repo.save_inventory(char_id, &inventory, &CharacterLooks::default())
        .await
        .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("character exists");
    assert_eq!(state.inventory, inventory);
}

/// PER-INV-02: holes compact on load and slot order is kept.
#[tokio::test]
async fn per_inv_02_holes_compact() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let a = InventoryItem::new(ItemId(909), 3);
    let b = InventoryItem::new(ItemId(501), 1);
    let c = InventoryItem::new(ItemId(1201), 1);
    let inventory = CharacterInventory {
        slots: vec![
            None,
            Some(a.clone()),
            None,
            None,
            Some(b.clone()),
            Some(c.clone()),
        ],
        unique_item_counter: 0,
    };
    repo.save_inventory(char_id, &inventory, &CharacterLooks::default())
        .await
        .expect("saves");
    let loaded = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists")
        .inventory;
    assert_eq!(loaded.slots, vec![Some(a), Some(b), Some(c)]);
}

/// PER-INV-03: a save replaces the rows atomically; the counter is persisted.
#[tokio::test]
async fn per_inv_03_save_replaces_rows() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let first = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 4],
        unique_item_counter: 3,
    };
    repo.save_inventory(char_id, &first, &CharacterLooks::default())
        .await
        .expect("saves");
    let second = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(501), 2))],
        unique_item_counter: 9,
    };
    repo.save_inventory(char_id, &second, &CharacterLooks::default())
        .await
        .expect("saves");
    let loaded = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists")
        .inventory;
    assert_eq!(loaded, second);

    let empty = CharacterInventory {
        slots: Vec::new(),
        unique_item_counter: 9,
    };
    repo.save_inventory(char_id, &empty, &CharacterLooks::default())
        .await
        .expect("saves");
    let loaded = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(loaded.inventory, empty);

    assert_eq!(
        repo.save_inventory(CharId(u32::MAX >> 1), &second, &CharacterLooks::default())
            .await,
        Err(RepositoryError::NotFound),
        "unknown character"
    );
}

/// PER-INV-04: the play state carries the stats, points, exp, HP/SP and the account group (and DB-PERS of
/// `specs/map-entry-settings` §3); deleting a character cascades.
#[tokio::test]
async fn per_inv_04_play_state_and_cascade() {
    let (repo, _acc, char_id) = inventory_fixture(99).await;
    sqlx::query(
        "UPDATE characters SET str = 11, agi = 12, vit = 13, int_stat = 14, dex = 15, luk = 16, \
         status_point = 7, skill_point = 3, base_exp = 5000000000, job_exp = 42, \
         hp = 31, max_hp = 45, sp = 9, max_sp = 12 \
         WHERE char_id = $1",
    )
    .bind(char_id.0 as i32)
    .execute(repo.pool())
    .await
    .expect("status update");
    let inventory = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1))],
        unique_item_counter: 1,
    };
    repo.save_inventory(char_id, &inventory, &CharacterLooks::default())
        .await
        .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(
        state.status,
        CharacterStatus {
            str: 11,
            agi: 12,
            vit: 13,
            int: 14,
            dex: 15,
            luk: 16,
            status_point: 7,
            skill_point: 3,
            base_exp: 5_000_000_000,
            job_exp: 42,
            hp: 31,
            max_hp: 45,
            sp: 9,
            max_sp: 12,
        }
    );
    assert_eq!(state.group_id, 99);

    sqlx::query("DELETE FROM characters WHERE char_id = $1")
        .bind(char_id.0 as i32)
        .execute(repo.pool())
        .await
        .expect("delete");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory WHERE char_id = $1")
        .bind(char_id.0 as i32)
        .fetch_one(repo.pool())
        .await
        .expect("count");
    assert_eq!(rows, 0, "inventory rows cascade");
    assert_eq!(repo.load_play_state(char_id).await.expect("loads"), None);
}

/// The seed makes the `test` account a GM (group 99).
#[tokio::test]
async fn per_inv_04_seed_sets_test_account_group() {
    let repo = get_repo().await;
    repo.init_schema().await.expect("schema");
    repo.seed_test_data().await.expect("seed");
    let account = repo
        .find_by_username("test")
        .await
        .expect("query")
        .expect("seeded");
    assert_eq!(account.group_id, 99);
}

/// More rows than the inventory holds are refused, not silently dropped (the next save would
/// delete them); a save with too many slots is refused too.
#[tokio::test]
async fn per_inv_02_over_capacity_is_refused() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let full = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 100],
        unique_item_counter: 0,
    };
    repo.save_inventory(char_id, &full, &CharacterLooks::default())
        .await
        .expect("100 fits");
    let too_many = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 101],
        unique_item_counter: 0,
    };
    assert!(matches!(
        repo.save_inventory(char_id, &too_many, &CharacterLooks::default())
            .await,
        Err(RepositoryError::Internal(_))
    ));
    sqlx::query("INSERT INTO inventory (char_id, nameid, amount) VALUES ($1, 909, 1)")
        .bind(char_id.0 as i32)
        .execute(repo.pool())
        .await
        .expect("101st row by hand");
    assert!(matches!(
        repo.load_play_state(char_id).await,
        Err(RepositoryError::Internal(_))
    ));
}

/// Concurrent saves of one character serialize: the result is one whole inventory, never both.
#[tokio::test]
async fn per_inv_03_concurrent_saves_serialize() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let a = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(501), 1)); 30],
        unique_item_counter: 1,
    };
    let b = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 40],
        unique_item_counter: 2,
    };
    let bare = CharacterLooks::default();
    for _ in 0..10 {
        let (ra, rb) = tokio::join!(
            repo.save_inventory(char_id, &a, &bare),
            repo.save_inventory(char_id, &b, &bare)
        );
        ra.expect("save a");
        rb.expect("save b");
        let loaded = repo
            .load_play_state(char_id)
            .await
            .expect("loads")
            .expect("exists")
            .inventory;
        assert!(loaded == a || loaded == b, "{} rows", loaded.slots.len());
    }
}

// ---- Equipment looks (spec PER-EQ-01..02) ----

/// PER-EQ-01: `save_inventory` writes the look columns with the inventory, and every summary
/// query returns them. A new character starts bare.
#[tokio::test]
async fn per_eq_01_looks_saved_with_inventory() {
    let (repo, acc, char_id) = inventory_fixture(0).await;
    let created = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(created.looks, CharacterLooks::default());

    let looks = CharacterLooks {
        weapon_type: 28,
        shield: 1,
        head_top: 16,
        head_mid: 3,
        head_bottom: 4,
        robe: 2,
    };
    let inventory = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(1201), 1))],
        unique_item_counter: 1,
    };
    repo.save_inventory(char_id, &inventory, &looks)
        .await
        .expect("saves");

    let by_id = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(by_id.looks, looks);
    let by_slot = repo
        .find_by_slot(acc.id, 0)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(by_slot.looks, looks);
    let listed = repo.list_by_account(acc.id).await.expect("lists");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].looks, looks);

    // A refused save (too many slots) leaves the looks alone.
    let too_many = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 101],
        unique_item_counter: 0,
    };
    assert!(repo
        .save_inventory(char_id, &too_many, &CharacterLooks::default())
        .await
        .is_err());
    let after = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(after.looks, looks);
}

/// Column names of `characters` in `schema`.
async fn character_columns(repo: &PostgresRepo, schema: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = 'characters'",
    )
    .bind(schema)
    .fetch_all(repo.pool())
    .await
    .expect("columns")
}

/// PER-EQ-02: the schema pass is idempotent on a fresh database and on one that still has the
/// old `weapon_view` column (renamed to `weapon_type`, values kept; `robe_view` added). Runs in
/// its own Postgres schema so the shared tables aren't touched.
#[tokio::test]
async fn per_eq_02_schema_migrates_look_columns() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::str::FromStr;

    let main = get_repo().await;
    let schema = format!("per_eq_02_{}", next_account_id().0);
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(main.pool())
        .await
        .expect("schema created");
    let options = PgConnectOptions::from_str(&test_db_url())
        .expect("db url")
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connects");
    let repo = PostgresRepo::new(pool);

    // Fresh database, twice.
    repo.init_schema().await.expect("fresh schema");
    repo.init_schema().await.expect("idempotent");
    let cols = character_columns(&main, &schema).await;
    assert!(cols.iter().any(|c| c == "weapon_type"), "{cols:?}");
    assert!(cols.iter().any(|c| c == "robe_view"), "{cols:?}");
    assert!(!cols.iter().any(|c| c == "weapon_view"), "{cols:?}");

    // Turn it back into the old layout, with a character wearing something.
    sqlx::raw_sql(
        "ALTER TABLE characters RENAME COLUMN weapon_type TO weapon_view; \
         ALTER TABLE characters DROP COLUMN robe_view; \
         INSERT INTO accounts (account_id, username, password_hash) VALUES (2000000, 'old', 'x'); \
         INSERT INTO characters (account_id, slot, name, sex, weapon_view, head_top_view) \
         VALUES (2000000, 0, 'Old', 1, 7, 16);",
    )
    .execute(repo.pool())
    .await
    .expect("old layout");

    repo.init_schema().await.expect("migrates");
    repo.init_schema()
        .await
        .expect("idempotent after migration");
    let cols = character_columns(&main, &schema).await;
    assert!(cols.iter().any(|c| c == "weapon_type"), "{cols:?}");
    assert!(cols.iter().any(|c| c == "robe_view"), "{cols:?}");
    assert!(!cols.iter().any(|c| c == "weapon_view"), "{cols:?}");
    let old = repo
        .list_by_account(AccountId(2_000_000))
        .await
        .expect("lists");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].looks.weapon_type, 7);
    assert_eq!(old[0].looks.head_top, 16);
    assert_eq!(old[0].looks.robe, 0);

    repo.pool().close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(main.pool())
        .await
        .expect("schema dropped");
}
