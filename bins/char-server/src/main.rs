use axum::{routing::{get, post}, Json, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse,
    RegisterMapServerRequest, RegisterMapServerResponse, ServerStatus, ServerType,
    TARGET_PACKETVER,
};

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Char,
        server_name: "metaling-char".to_string(),
        uptime_seconds: 0,
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

async fn register_map_server(
    Json(payload): Json<RegisterMapServerRequest>,
) -> Json<RegisterMapServerResponse> {
    info!(
        "Registering map server {} at {}:{} with {} maps",
        payload.name,
        payload.client_host,
        payload.client_port,
        payload.maps.len()
    );
    Json(RegisterMapServerResponse {
        accepted: true,
        registered_maps_count: payload.maps.len(),
    })
}

async fn prepare_transfer(
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Preparing map transfer for character id {} to map {}",
        payload.char_id.0, payload.destination_map
    );
    Json(PrepareMapTransferResponse { accepted: true })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!(
        "Starting char-server (Target PACKETVER: {}, Client TCP: 6121, Internal HTTP: 6122)",
        TARGET_PACKETVER
    );

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/maps/register", post(register_map_server))
        .route("/api/v1/transfers/prepare", post(prepare_transfer));

    let http_addr: SocketAddr = "0.0.0.0:6122".parse()?;
    let http_listener = TcpListener::bind(http_addr).await?;
    info!("Character HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = "0.0.0.0:6121".parse()?;
    let client_listener = TcpListener::bind(client_addr).await?;
    info!("Character client TCP listener active on {}", client_addr);

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
