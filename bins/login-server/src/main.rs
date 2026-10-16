mod service;
mod session;

use std::net::SocketAddr;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::{error, info};

use ro_contracts::{
    ids::CharServerInfo, HealthResponse, RegisterCharServerRequest, RegisterCharServerResponse,
    ServerHeartbeat, ServerHeartbeatResponse, ServerStatus, ServerType, ValidateSessionRequest,
    ValidateSessionResponse, TARGET_PACKETVER,
};

use service::LoginServiceHandle;
use session::handle_client_session;

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Login,
        server_name: "metaling-login".to_string(),
        uptime_seconds: 0,
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

async fn validate_session(
    Json(payload): Json<ValidateSessionRequest>,
) -> Json<ValidateSessionResponse> {
    info!("Validating session for account id {}", payload.account_id.0);
    Json(ValidateSessionResponse {
        valid: true,
        account_id: payload.account_id,
        error_message: None,
    })
}

async fn register_char_server(
    State(service): State<LoginServiceHandle>,
    Json(payload): Json<RegisterCharServerRequest>,
) -> Json<RegisterCharServerResponse> {
    info!(
        "Registering character server {} at {}:{}",
        payload.name, payload.client_host, payload.client_port
    );

    let info = CharServerInfo {
        ip: [127, 0, 0, 1],
        port: payload.client_port,
        name: payload.name,
        users: 0,
        server_type: 0,
        new_server: 0,
    };
    service.register_char_server(info);

    Json(RegisterCharServerResponse {
        accepted: true,
        server_id: payload.server_id,
    })
}

async fn handle_heartbeat(Json(payload): Json<ServerHeartbeat>) -> Json<ServerHeartbeatResponse> {
    info!(
        "Heartbeat received from server {} with {} active users",
        payload.server_id, payload.user_count
    );
    Json(ServerHeartbeatResponse { acknowledged: true })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!(
        "Starting login-server (Target PACKETVER: {}, Client TCP: 6900, Internal HTTP: 6901)",
        TARGET_PACKETVER
    );

    let service_handle = LoginServiceHandle::new();

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/sessions/validate", post(validate_session))
        .route("/api/v1/servers/register", post(register_char_server))
        .route("/api/v1/servers/heartbeat", post(handle_heartbeat))
        .with_state(service_handle.clone());

    let http_addr: SocketAddr = "0.0.0.0:6901".parse()?;
    let http_listener = TcpListener::bind(http_addr).await?;
    info!("Login HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = "0.0.0.0:6900".parse()?;
    let client_listener = TcpListener::bind(client_addr).await?;
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
                    error!("Error accepting client connection: {}", err);
                    break;
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
    use bytes::BytesMut;
    use ro_contracts::PacketOpcode;
    use ro_packets::{PacketDecoder, RathenaPacketLengthTable};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    use tokio_util::codec::Decoder;

    #[tokio::test]
    async fn test_login_handshake_via_channel_service() {
        let service_handle = LoginServiceHandle::new();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (socket, peer_addr) = listener.accept().await.unwrap();
            handle_client_session(socket, peer_addr, service_handle).await;
        });

        let mut client_socket = TcpStream::connect(server_addr).await.unwrap();

        let mut ca_login_frame = [0u8; 55];
        ca_login_frame[0] = 0x64;
        ca_login_frame[1] = 0x00;
        ca_login_frame[2..6].copy_from_slice(&1u32.to_le_bytes());
        let username_bytes = b"testuser";
        ca_login_frame[6..6 + username_bytes.len()].copy_from_slice(username_bytes);
        let password_bytes = b"secret";
        ca_login_frame[30..30 + password_bytes.len()].copy_from_slice(password_bytes);
        ca_login_frame[54] = 0;

        client_socket.write_all(&ca_login_frame).await.unwrap();
        client_socket.flush().await.unwrap();

        let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
        let mut buffer = BytesMut::with_capacity(1024);

        loop {
            if let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                assert_eq!(raw.opcode, PacketOpcode::AC_ACCEPT_LOGIN);
                assert!(raw.payload_len() > 0);
                break;
            }
            let bytes_read = client_socket.read_buf(&mut buffer).await.unwrap();
            assert!(bytes_read > 0);
        }
    }
}
