mod config;
mod service;
mod session;

pub use config::LoginConfig;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use ro_contracts::{
    HealthResponse, RegisterCharServerRequest, RegisterCharServerResponse,
    ServerHeartbeat, ServerHeartbeatResponse, ValidateSessionRequest,
    ValidateSessionResponse, TARGET_PACKETVER,
};
use ro_persistence::InMemoryStubRepo;

pub use service::{LoginService, LoginServiceHandle};
pub use session::{handle_client_session, handle_in_memory_session, handle_stream};

pub async fn health_check(State(service): State<LoginServiceHandle>) -> Json<HealthResponse> {
    Json(service.health_response())
}

pub async fn validate_session(
    State(service): State<LoginServiceHandle>,
    Json(payload): Json<ValidateSessionRequest>,
) -> Json<ValidateSessionResponse> {
    info!("Validating session for account id {}", payload.account_id.0);
    Json(service.validate_session(&payload))
}

pub async fn register_char_server(
    State(service): State<LoginServiceHandle>,
    Json(payload): Json<RegisterCharServerRequest>,
) -> Result<Json<RegisterCharServerResponse>, (StatusCode, Json<RegisterCharServerResponse>)> {
    info!(
        "Registering character server {} at {}:{}",
        payload.name, payload.client_host, payload.client_port
    );
    let server_id = payload.server_id;
    if service.register_char_server(payload) {
        Ok(Json(RegisterCharServerResponse {
            accepted: true,
            server_id,
        }))
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(RegisterCharServerResponse {
                accepted: false,
                server_id,
            }),
        ))
    }
}

pub async fn handle_heartbeat(
    State(service): State<LoginServiceHandle>,
    Json(payload): Json<ServerHeartbeat>,
) -> Json<ServerHeartbeatResponse> {
    info!(
        "Heartbeat received from server {} with {} active users",
        payload.server_id, payload.user_count
    );
    let acknowledged = service.update_heartbeat(&payload);
    Json(ServerHeartbeatResponse { acknowledged })
}

pub fn create_http_router(service: LoginServiceHandle) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/sessions/validate", post(validate_session))
        .route("/api/v1/servers/register", post(register_char_server))
        .route("/api/v1/servers/heartbeat", post(handle_heartbeat))
        .with_state(service)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = LoginConfig::from_env();
    info!(
        "Starting login-server (Target PACKETVER: {}, Client TCP: {}, Internal HTTP: {}, Advertised Char Port: {})",
        TARGET_PACKETVER, config.client_port, config.http_port, config.advertised_char_port
    );

    let account_repo = Arc::new(InMemoryStubRepo::new());
    let service_handle = LoginService::with_ttl_and_char_port(
        account_repo,
        std::time::Duration::from_secs(300),
        config.advertised_char_port,
    );

    let http_router = create_http_router(service_handle.clone());

    let http_addr: SocketAddr = format!("0.0.0.0:{}", config.http_port)
        .parse()
        .context("Failed to parse login HTTP address")?;
    let http_listener = TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("Failed to bind login HTTP listener to {}", http_addr))?;
    info!("Login HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = format!("0.0.0.0:{}", config.client_port)
        .parse()
        .context("Failed to parse login client TCP address")?;
    let client_listener = TcpListener::bind(client_addr)
        .await
        .with_context(|| format!("Failed to bind login client TCP listener to {}", client_addr))?;
    info!("Login client TCP listener active on {}", client_addr);

    let http_task = tokio::spawn(async move {
        if let Err(err) = axum::serve(http_listener, http_router).await {
            error!("HTTP server failure: {}", err);
        }
    });

    let client_service = service_handle.clone();
    let client_task = tokio::spawn(async move {
        loop {
            match client_listener.accept().await {
                Ok((stream, peer_addr)) => {
                    tokio::spawn(handle_client_session(
                        stream,
                        peer_addr,
                        client_service.clone(),
                    ));
                }
                Err(err) => {
                    warn!("Transient error accepting client connection: {}", err);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            }
        }
    });

    tokio::select! {
        _ = http_task => info!("HTTP server task terminated"),
        _ = client_task => info!("Client TCP listener task terminated"),
        _ = tokio::signal::ctrl_c() => info!("Shutdown signal received"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_contracts::{
        ids::{AccountId, Sex},
        ServerStatus, ServerType,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn make_ca_login_frame(username: &str, password: &str) -> [u8; 55] {
        let mut frame = [0u8; 55];
        frame[0..2].copy_from_slice(&0x0064u16.to_le_bytes());
        frame[2..6].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        let user_bytes = username.as_bytes();
        frame[6..6 + user_bytes.len().min(24)].copy_from_slice(&user_bytes[..user_bytes.len().min(24)]);
        let pass_bytes = password.as_bytes();
        frame[30..30 + pass_bytes.len().min(24)].copy_from_slice(&pass_bytes[..pass_bytes.len().min(24)]);
        frame[54] = 0;
        frame
    }

    #[tokio::test]
    async fn test_trace_session_1_bad_password() {
        let repo = Arc::new(InMemoryStubRepo::new());
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
            std::time::Duration::from_millis(50),
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
        let repo = Arc::new(InMemoryStubRepo::new());
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
            let n = client_stream.read(&mut response_buf[total_read..]).await.unwrap();
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
            std::time::Duration::from_millis(50),
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
        let res1 = validate_session(State(service.clone()), Json(validate_req.clone())).await.0;
        assert!(res1.valid);
        assert_eq!(res1.account_id, AccountId(account_id_val));
        assert!(res1.error_message.is_none());

        // Return to Character Selection (second validation with exact same ticket)
        let res2 = validate_session(State(service.clone()), Json(validate_req.clone())).await.0;
        assert!(res2.valid);
        assert_eq!(res2.account_id, AccountId(account_id_val));

        // Validation with incorrect login_id1
        let bad_validate_req = ValidateSessionRequest {
            account_id: AccountId(account_id_val),
            login_id1: login_id1.wrapping_add(1),
            login_id2,
            sex: Sex::Male,
        };
        let res3 = validate_session(State(service.clone()), Json(bad_validate_req)).await.0;
        assert!(!res3.valid);
    }

    #[tokio::test]
    async fn test_char_server_registration_and_heartbeat_flow() {
        let repo = Arc::new(InMemoryStubRepo::new());
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
        let hb_res = handle_heartbeat(State(service.clone()), Json(hb_req)).await.0;
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
        loop {
            let n = client_stream.read(&mut response_buf[total_read..]).await.unwrap();
            if n == 0 {
                break;
            }
            total_read += n;
            if total_read >= 224 {
                break;
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
        let repo = Arc::new(InMemoryStubRepo::new());
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
        while total_read < 224 {
            let n = client_stream.read(&mut response_buf[total_read..]).await.unwrap();
            assert!(n > 0, "Unexpected early EOF");
            total_read += n;
        }
        assert_eq!(total_read, 224);

        // Assert socket is still open after 100ms
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let mut check_buf = [0u8; 1];
        let timeout_check = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            client_stream.read(&mut check_buf),
        )
        .await;
        assert!(timeout_check.is_err(), "Socket must remain open waiting for client closure");

        // Server task must still be running
        assert!(!session_handle.is_finished(), "Server session task must not exit before client disconnects");

        // Client closes the stream
        drop(client_stream);

        // Server task now terminates cleanly within timeout
        let join_result = tokio::time::timeout(std::time::Duration::from_millis(500), session_handle).await;
        assert!(join_result.is_ok(), "Server session task should terminate upon client EOF");
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::new(repo);

        let health = health_check(State(service)).await.0;
        assert_eq!(health.status, ServerStatus::Ready);
        assert_eq!(health.server_type, ServerType::Login);
        assert_eq!(health.target_packetver, TARGET_PACKETVER);
    }

    #[tokio::test]
    async fn test_register_char_server_capacity_limit() {
        let repo = Arc::new(InMemoryStubRepo::new());
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
}
