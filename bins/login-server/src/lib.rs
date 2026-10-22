pub mod config;
pub mod service;
pub mod session;

pub use config::LoginConfig;
pub use service::{LoginService, LoginServiceHandle};
pub use session::{handle_client_session, handle_in_memory_session, handle_stream};

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use ro_core::{
    HealthResponse, RegisterCharServerRequest, RegisterCharServerResponse, ServerHeartbeat,
    ServerHeartbeatResponse, ValidateSessionRequest, ValidateSessionResponse,
};
use tracing::info;

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
