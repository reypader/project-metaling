#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::{sync::Arc, time::Duration};

use axum::{extract::State, http::StatusCode, Json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use login_server::{
    handle_heartbeat, handle_in_memory_session, health_check, register_char_server,
    validate_session, LoginService,
};
use ro_core::{
    account::AccountRepository,
    ids::{AccountId, Sex},
    packet::ServerEvent,
    RegisterCharServerRequest, ServerHeartbeat, ServerStatus, ServerType, ValidateSessionRequest,
    TARGET_PACKETVER,
};
use ro_persistence::PostgresRepo;

fn make_ca_login_frame(username: &str, password: &str) -> [u8; 55] {
    let mut frame = [0u8; 55];
    frame[0..2].copy_from_slice(&0x0064u16.to_le_bytes());
    frame[2..6].copy_from_slice(&0x8000_0000u32.to_le_bytes());
    let user_bytes = username.as_bytes();
    frame[6..6 + user_bytes.len().min(24)].copy_from_slice(&user_bytes[..user_bytes.len().min(24)]);
    let pass_bytes = password.as_bytes();
    frame[30..30 + pass_bytes.len().min(24)]
        .copy_from_slice(&pass_bytes[..pass_bytes.len().min(24)]);
    frame[54] = 0;
    frame
}

async fn setup_test_repo() -> Arc<dyn AccountRepository> {
    let test_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = PostgresRepo::connect(&test_db_url).await.unwrap();
    let _ = repo.seed_test_data().await;
    Arc::new(repo)
}

#[tokio::test]
async fn test_trace_session_1_bad_password() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let (mut client_stream, server_stream) = tokio::io::duplex(1024);
    let (server_reader, server_writer) = tokio::io::split(server_stream);

    tokio::spawn(handle_in_memory_session(
        server_reader,
        server_writer,
        service,
    ));

    let frame = make_ca_login_frame("test", "passwords");
    client_stream.write_all(&frame).await.unwrap();
    client_stream.flush().await.unwrap();

    let mut response_buf = [0u8; 64];
    let bytes_read = client_stream.read(&mut response_buf).await.unwrap();
    assert_eq!(bytes_read, 26);

    let mut expected_refuse = [0u8; 26];
    expected_refuse[0..2].copy_from_slice(&0x083eu16.to_le_bytes());
    expected_refuse[2..6].copy_from_slice(&1u32.to_le_bytes());

    assert_eq!(&response_buf[..26], &expected_refuse);

    // Verify socket remains open (no premature EOF from server)
    let timeout_result = tokio::time::timeout(
        Duration::from_millis(50),
        client_stream.read(&mut response_buf),
    )
    .await;
    assert!(
        timeout_result.is_err(),
        "Expected socket to remain open waiting for client closure"
    );

    // Client initiates connection closure
    drop(client_stream);
}

#[tokio::test]
async fn test_trace_session_2_valid_password_and_ticket_lifecycle() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let (mut client_stream, server_stream) = tokio::io::duplex(2048);
    let (server_reader, server_writer) = tokio::io::split(server_stream);

    tokio::spawn(handle_in_memory_session(
        server_reader,
        server_writer,
        service.clone(),
    ));

    // Send Session #2 CA_LOGIN
    let frame = make_ca_login_frame("test", "password");
    client_stream.write_all(&frame).await.unwrap();
    client_stream.flush().await.unwrap();

    let mut response_buf = [0u8; 512];
    let mut total_read = 0;
    loop {
        let n = client_stream
            .read(&mut response_buf[total_read..])
            .await
            .unwrap();
        if n == 0 {
            break;
        }
        total_read += n;
        if total_read >= 224 {
            break;
        }
    }
    assert_eq!(total_read, 224);

    // Opcode 0x0ac4
    let opcode = u16::from_le_bytes([response_buf[0], response_buf[1]]);
    assert_eq!(opcode, 0x0ac4);

    // Total length 224
    let packet_len = u16::from_le_bytes([response_buf[2], response_buf[3]]);
    assert_eq!(packet_len, 224);

    // Account ID 2000000
    let account_id_val = u32::from_le_bytes(response_buf[8..12].try_into().unwrap());
    assert_eq!(account_id_val, 2000000);

    // Non-zero login IDs
    let login_id1 = u32::from_le_bytes(response_buf[4..8].try_into().unwrap());
    let login_id2 = u32::from_le_bytes(response_buf[12..16].try_into().unwrap());
    assert_ne!(login_id1, 0);
    assert_ne!(login_id2, 0);

    // Sex: 1 (Male)
    let sex_byte = response_buf[46];
    assert_eq!(sex_byte, 1);

    // Char server port: 6121
    let char_port = u16::from_le_bytes(response_buf[68..70].try_into().unwrap());
    assert_eq!(char_port, 6121);

    // Verify socket remains open (no premature EOF from server)
    let mut eof_check = [0u8; 1];
    let timeout_result = tokio::time::timeout(
        Duration::from_millis(50),
        client_stream.read(&mut eof_check),
    )
    .await;
    assert!(
        timeout_result.is_err(),
        "Expected socket to remain open waiting for client closure"
    );

    // Client initiates connection closure after receiving AC_ACCEPT_LOGIN
    drop(client_stream);

    // Initial validation from char-server (CH_ENTER)
    let validate_req = ValidateSessionRequest {
        account_id: AccountId(account_id_val),
        login_id1,
        login_id2,
        sex: Sex::Male,
    };
    let res1 = validate_session(State(service.clone()), Json(validate_req.clone()))
        .await
        .0;
    assert!(res1.valid);
    assert_eq!(res1.account_id, AccountId(account_id_val));
    assert!(res1.error_message.is_none());

    // Return to Character Selection (second validation with exact same ticket)
    let res2 = validate_session(State(service.clone()), Json(validate_req.clone()))
        .await
        .0;
    assert!(res2.valid);
    assert_eq!(res2.account_id, AccountId(account_id_val));

    // Validation with incorrect login_id1
    let bad_validate_req = ValidateSessionRequest {
        account_id: AccountId(account_id_val),
        login_id1: login_id1.wrapping_add(1),
        login_id2,
        sex: Sex::Male,
    };
    let res3 = validate_session(State(service.clone()), Json(bad_validate_req))
        .await
        .0;
    assert!(!res3.valid);
}

#[tokio::test]
async fn test_char_server_registration_and_heartbeat_flow() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    // Register char-server with updated port and name
    let reg_req = RegisterCharServerRequest {
        server_id: 1,
        name: "Custom-Char".to_string(),
        client_host: "127.0.0.1".to_string(),
        client_port: 6128,
        internal_http_url: "http://127.0.0.1:6129".to_string(),
        max_users: 1500,
    };
    let reg_res = register_char_server(State(service.clone()), Json(reg_req))
        .await
        .expect("Registration should succeed")
        .0;
    assert!(reg_res.accepted);
    assert_eq!(reg_res.server_id, 1);

    // Send heartbeat updating active users to 77
    let hb_req = ServerHeartbeat {
        server_id: 1,
        user_count: 77,
        timestamp_ms: 99999,
    };
    let hb_res = handle_heartbeat(State(service.clone()), Json(hb_req))
        .await
        .0;
    assert!(hb_res.acknowledged);

    // Connect client via duplex stream and assert updated descriptor in 0x0ac4
    let (mut client_stream, server_stream) = tokio::io::duplex(2048);
    let (server_reader, server_writer) = tokio::io::split(server_stream);

    tokio::spawn(handle_in_memory_session(
        server_reader,
        server_writer,
        service.clone(),
    ));

    let frame = make_ca_login_frame("test", "password");
    client_stream.write_all(&frame).await.unwrap();
    client_stream.flush().await.unwrap();

    let mut response_buf = [0u8; 512];
    let mut total_read = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while total_read < 224 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) = tokio::time::timeout(
            Duration::from_millis(100),
            client_stream.read(&mut response_buf[total_read..]),
        )
        .await
        {
            if n == 0 {
                break;
            }
            total_read += n;
        }
    }
    assert_eq!(total_read, 224);

    // Char server port should be 6128
    let char_port = u16::from_le_bytes(response_buf[68..70].try_into().unwrap());
    assert_eq!(char_port, 6128);

    // Active users should be 77
    let users = u16::from_le_bytes(response_buf[90..92].try_into().unwrap());
    assert_eq!(users, 77);

    drop(client_stream);
}

#[tokio::test]
async fn test_login_socket_remains_open_until_client_disconnects() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let (mut client_stream, server_stream) = tokio::io::duplex(2048);
    let (server_reader, server_writer) = tokio::io::split(server_stream);

    let session_handle = tokio::spawn(handle_in_memory_session(
        server_reader,
        server_writer,
        service.clone(),
    ));

    let frame = make_ca_login_frame("test", "password");
    client_stream.write_all(&frame).await.unwrap();
    client_stream.flush().await.unwrap();

    let mut response_buf = [0u8; 512];
    let mut total_read = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while total_read < 224 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) = tokio::time::timeout(
            Duration::from_millis(100),
            client_stream.read(&mut response_buf[total_read..]),
        )
        .await
        {
            assert!(n > 0, "Unexpected early EOF");
            total_read += n;
        }
    }
    assert_eq!(total_read, 224);

    // Assert socket is still open after 100ms
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut check_buf = [0u8; 1];
    let timeout_check = tokio::time::timeout(
        Duration::from_millis(50),
        client_stream.read(&mut check_buf),
    )
    .await;
    assert!(
        timeout_check.is_err(),
        "Socket must remain open waiting for client closure"
    );

    // Server task must still be running
    assert!(
        !session_handle.is_finished(),
        "Server session task must not exit before client disconnects"
    );

    // Client closes the stream
    drop(client_stream);

    // Server task now terminates cleanly within timeout
    let join_result = tokio::time::timeout(Duration::from_millis(500), session_handle).await;
    assert!(
        join_result.is_ok(),
        "Server session task should terminate upon client EOF"
    );
}

#[tokio::test]
async fn test_health_check_endpoint() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    let health = health_check(State(service)).await.0;
    assert_eq!(health.status, ServerStatus::Ready);
    assert_eq!(health.server_type, ServerType::Login);
    assert_eq!(health.target_packetver, TARGET_PACKETVER);
}

#[tokio::test]
async fn test_register_char_server_capacity_limit() {
    let repo = setup_test_repo().await;
    let service = LoginService::new(repo);

    for id in 2..=32 {
        let req = RegisterCharServerRequest {
            server_id: id,
            name: format!("Char{}", id),
            client_host: "127.0.0.1".to_string(),
            client_port: 6120 + id as u16,
            internal_http_url: format!("http://127.0.0.1:{}", 7120 + id),
            max_users: 1000,
        };
        let res = register_char_server(State(service.clone()), Json(req))
            .await
            .expect("Registration should succeed")
            .0;
        assert!(res.accepted);
    }

    let overflow_req = RegisterCharServerRequest {
        server_id: 33,
        name: "OverflowChar".to_string(),
        client_host: "127.0.0.1".to_string(),
        client_port: 6199,
        internal_http_url: "http://127.0.0.1:7199".to_string(),
        max_users: 1000,
    };
    let err = register_char_server(State(service.clone()), Json(overflow_req))
        .await
        .expect_err("Registration should exceed capacity");
    assert_eq!(err.0, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!err.1 .0.accepted);
}

#[tokio::test]
async fn test_bin_cfg_persist_002_login_startup_failure_on_unreachable_db() {
    let unreachable_url = "postgres://invalid:invalid@127.0.0.1:5433/nonexistent";
    let connect_result = PostgresRepo::connect(unreachable_url).await;
    assert!(
        connect_result.is_err(),
        "Expected connection to unreachable DB to fail fast"
    );
}

#[tokio::test]
async fn test_bin_login_persist_001_auth_against_postgres() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = match PostgresRepo::connect(&db_url).await {
        Ok(r) => r,
        Err(err) => {
            eprintln!(
                "Skipping test_bin_login_persist_001: DB unavailable ({})",
                err
            );
            return;
        }
    };
    let _ = repo.seed_test_data().await;

    let service = LoginService::new(Arc::new(repo));

    let event = service
        .process_login("test", "password", 0x80000000, 0)
        .await;
    match event {
        ServerEvent::LoginAccepted {
            account_id,
            sex,
            char_servers,
            ..
        } => {
            assert_eq!(account_id, AccountId(2000000));
            assert_eq!(sex, Sex::Male);
            assert!(!char_servers.is_empty());
        }
        other => panic!("Expected LoginAccepted, got {:?}", other),
    }

    let bad_event = service
        .process_login("test", "wrongpassword", 0x80000000, 0)
        .await;
    assert_eq!(bad_event, ServerEvent::LoginRefused { error_code: 1 });
}
