pub mod config;
pub mod service;
pub mod session;

pub use config::CharConfig;
pub use service::CharServiceHandle;
pub use session::handle_client_session;

use std::net::Ipv4Addr;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse,
    RegisterMapServerRequest, RegisterMapServerResponse, ServerStatus, ServerType,
    TARGET_PACKETVER,
};
use tracing::info;

pub async fn health_check(State(service): State<CharServiceHandle>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Char,
        server_name: "metaling-char".to_string(),
        uptime_seconds: service.uptime_seconds(),
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

pub async fn register_map_server(
    State(service): State<CharServiceHandle>,
    Json(payload): Json<RegisterMapServerRequest>,
) -> Json<RegisterMapServerResponse> {
    info!(
        "Registering map server {} at {}:{} with {} maps",
        payload.name,
        payload.client_host,
        payload.client_port,
        payload.maps.len()
    );

    let host = payload
        .client_host
        .parse::<Ipv4Addr>()
        .map(|addr| addr.octets())
        .unwrap_or([127, 0, 0, 1]);

    let registered_maps_count = payload.maps.len();
    let accepted =
        service.register_map_server(payload.name, payload.maps, host, payload.client_port);

    Json(RegisterMapServerResponse {
        accepted,
        registered_maps_count,
    })
}

pub async fn prepare_transfer(
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Preparing map transfer for character id {} to map {}",
        payload.char_id.0, payload.destination_map
    );
    Json(PrepareMapTransferResponse { accepted: true })
}

pub fn create_http_router(service: CharServiceHandle) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/maps/register", post(register_map_server))
        .route("/api/v1/transfers/prepare", post(prepare_transfer))
        .with_state(service)
}
