mod claim;
pub mod config;
pub mod service;
pub mod session;

pub use claim::{Claim, MapRepository};

pub use config::MapConfig;
pub use service::{
    is_valid_map_name, load_battle_config, load_mob_spawns, load_npcs, load_registries,
    load_walkability, map_data_loader, Entry, JoinSession, MapData, MapDataLoader, MapHandoff,
    MapServiceHandle, MapServiceOptions, RegistryPaths, SessionControl, WorldCommand,
    AUTOSAVE_INTERVAL, FALLBACK_MAP_SIZE, TICK_INTERVAL,
};
pub use session::handle_client_session;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use ro_core::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse, ServerStatus,
    ServerType, TARGET_PACKETVER,
};
use tracing::info;

pub async fn health_check(State(service): State<MapServiceHandle>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Map,
        server_name: "metaling-map".to_string(),
        uptime_seconds: service.uptime_seconds(),
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

pub async fn accept_transfer(
    State(service): State<MapServiceHandle>,
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Map-server processing transfer reservation for character id {} on map {}",
        payload.char_id.0, payload.destination_map
    );

    let accepted = service.accept_transfer(
        payload.char_id,
        payload.destination_map,
        Some(payload.destination_pos),
    );

    Json(PrepareMapTransferResponse { accepted })
}

pub fn create_http_router(service_handle: MapServiceHandle) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/transfers/accept", post(accept_transfer))
        .with_state(service_handle)
}
