//! Account authentication and lookup (PERSIST-AUTH-001..005).

use ro_core::{AccountId, AccountState, Sex};
use ro_persistence::{AccountRepository, AuthError};

use crate::common::*;

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
