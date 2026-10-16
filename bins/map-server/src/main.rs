use axum::{routing::{get, post}, Json, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse, ServerStatus,
    ServerType, TARGET_PACKETVER,
};

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Map,
        server_name: "metaling-map".to_string(),
        uptime_seconds: 0,
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

async fn accept_transfer(
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Map-server accepted transfer reservation for character id {} on map {}",
        payload.char_id.0, payload.destination_map
    );
    Json(PrepareMapTransferResponse { accepted: true })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!(
        "Starting map-server (Target PACKETVER: {}, Client TCP: 5121, Internal HTTP: 5122)",
        TARGET_PACKETVER
    );

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/transfers/accept", post(accept_transfer));

    let http_addr: SocketAddr = "0.0.0.0:5122".parse()?;
    let http_listener = TcpListener::bind(http_addr).await?;
    info!("Map HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = "0.0.0.0:5121".parse()?;
    let client_listener = TcpListener::bind(client_addr).await?;
    info!("Map client TCP listener active on {}", client_addr);

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
