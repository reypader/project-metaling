use axum::{routing::{get, post}, Json, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

use ro_contracts::{
    HealthResponse, ServerHeartbeat, ServerHeartbeatResponse, ServerStatus, ServerType,
    ValidateSessionRequest, ValidateSessionResponse, TARGET_PACKETVER,
};

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
    Json(payload): Json<ro_contracts::RegisterCharServerRequest>,
) -> Json<ro_contracts::RegisterCharServerResponse> {
    info!(
        "Registering character server {} at {}:{}",
        payload.name, payload.client_host, payload.client_port
    );
    Json(ro_contracts::RegisterCharServerResponse {
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

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/sessions/validate", post(validate_session))
        .route("/api/v1/servers/register", post(register_char_server))
        .route("/api/v1/servers/heartbeat", post(handle_heartbeat));

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

    let client_task = tokio::spawn(async move {
        loop {
            match client_listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!("Accepted client connection from {}", peer_addr);
                    // Connection handshake logic will be handled here
                    drop(stream);
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
