//! Integration tests for PostgresRepo implementing test scenarios PERSIST-AUTH-001 through PERSIST-SCHEMA-002.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::sync::atomic::{AtomicU32, Ordering};

use ro_core::{
    account::{Account, AccountState},
    character::CreateCharacterRequest,
    ids::{AccountId, CharId, MapPosition, Sex},
};
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
