use std::{sync::Arc, time::Duration};

use ro_core::{
    AccountId, RegisterCharServerRequest, ServerEvent, ServerHeartbeat, Sex, ValidateSessionRequest,
};
use ro_persistence::PostgresRepo;

use super::*;

async fn setup_test_repo() -> Arc<dyn ro_persistence::AccountRepository> {
    let test_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = PostgresRepo::connect(&test_db_url).await.unwrap();
    let _ = repo.seed_test_data().await;
    Arc::new(repo)
}

#[tokio::test]
async fn test_process_login_success() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let event = service
        .process_login("test", "password", 0x80000000, 0)
        .await;

    match event {
        ServerEvent::LoginAccepted {
            account_id,
            login_id1,
            login_id2,
            sex,
            char_servers,
        } => {
            assert_eq!(account_id, AccountId(2000000));
            assert_eq!(sex, Sex::Male);
            assert_ne!(login_id1, 0);
            assert_ne!(login_id2, 0);
            assert_eq!(char_servers.len(), 1);
            assert_eq!(char_servers[0].port, 6121);
        }
        other => panic!("Expected LoginAccepted, got {:?}", other),
    }
}

#[tokio::test]
async fn test_process_login_bad_password() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let event = service
        .process_login("test", "passwords", 0x80000000, 0)
        .await;

    assert_eq!(event, ServerEvent::LoginRefused { error_code: 1 });
}

#[tokio::test]
async fn test_process_login_unknown_user() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let event = service
        .process_login("unknown", "password", 0x80000000, 0)
        .await;

    assert_eq!(event, ServerEvent::LoginRefused { error_code: 0 });
}

#[tokio::test]
async fn test_session_ticket_lifecycle_and_reauth() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let event = service
        .process_login("test", "password", 0x80000000, 0)
        .await;

    let (account_id, login_id1, login_id2, sex) = match event {
        ServerEvent::LoginAccepted {
            account_id,
            login_id1,
            login_id2,
            sex,
            ..
        } => (account_id, login_id1, login_id2, sex),
        _ => panic!("Expected LoginAccepted"),
    };

    // Initial validation (CH_ENTER)
    let validate_req = ValidateSessionRequest {
        account_id,
        login_id1,
        login_id2,
        sex,
    };
    let res1 = service.validate_session(&validate_req);
    assert!(res1.valid);
    assert_eq!(res1.account_id, account_id);
    assert!(res1.error_message.is_none());

    // Second validation with same ticket (Return to Character Selection)
    let res2 = service.validate_session(&validate_req);
    assert!(res2.valid);
    assert_eq!(res2.account_id, account_id);

    // Validation with incorrect credentials
    let bad_req = ValidateSessionRequest {
        account_id,
        login_id1: login_id1.wrapping_add(1),
        login_id2,
        sex,
    };
    let res3 = service.validate_session(&bad_req);
    assert!(!res3.valid);

    // Validation with incorrect sex
    let bad_sex_req = ValidateSessionRequest {
        account_id,
        login_id1,
        login_id2,
        sex: Sex::Female,
    };
    let res4 = service.validate_session(&bad_sex_req);
    assert!(!res4.valid);
}

#[tokio::test]
async fn test_session_ticket_expiration() {
    let repo = setup_test_repo().await;
    // Configure with 1ms TTL
    let service = LoginService::with_ttl(repo, Duration::from_millis(1));

    let event = service
        .process_login("test", "password", 0x80000000, 0)
        .await;

    let (account_id, login_id1, login_id2, sex) = match event {
        ServerEvent::LoginAccepted {
            account_id,
            login_id1,
            login_id2,
            sex,
            ..
        } => (account_id, login_id1, login_id2, sex),
        _ => panic!("Expected LoginAccepted"),
    };

    tokio::time::sleep(Duration::from_millis(10)).await;

    let validate_req = ValidateSessionRequest {
        account_id,
        login_id1,
        login_id2,
        sex,
    };
    let res = service.validate_session(&validate_req);
    assert!(!res.valid);
    assert_eq!(res.error_message.as_deref(), Some("Session ticket expired"));
}

#[tokio::test]
async fn test_char_server_registration_and_heartbeat() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let reg_req = RegisterCharServerRequest {
        server_id: 1,
        name: "Updated-Char".to_string(),
        client_host: "127.0.0.1".to_string(),
        client_port: 6125,
        internal_http_url: "http://127.0.0.1:6126".to_string(),
        max_users: 500,
    };
    assert!(service.register_char_server(reg_req));

    let hb = ServerHeartbeat {
        server_id: 1,
        user_count: 99,
        timestamp_ms: 12345,
    };
    assert!(service.update_heartbeat(&hb));

    let infos = service.char_server_infos();
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].name, "Updated-Char");
    assert_eq!(infos[0].port, 6125);
    assert_eq!(infos[0].users, 99);
}
