//! Inter-server HTTP contracts and DTOs for Axum endpoints.
//!
//! Enables structured REST communication between login-server, char-server, and map-server.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, CharId, CharacterSummary, MapPosition, Sex};

/// Operational status of an individual server node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    Starting,
    Ready,
    Maintenance,
    ShuttingDown,
}

/// Node role classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerType {
    Login,
    Char,
    Map,
}

/// Standard response payload for health check probes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: ServerStatus,
    pub server_type: ServerType,
    pub server_name: String,
    pub uptime_seconds: u64,
    pub active_connections: usize,
    pub target_packetver: u32,
}

/// Periodic heartbeat payload sent from child servers to upstream coordinators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHeartbeat {
    pub server_id: u32,
    pub user_count: u32,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHeartbeatResponse {
    pub acknowledged: bool,
}

/// Request sent by char-server to login-server to validate an incoming client session ticket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSessionRequest {
    pub account_id: AccountId,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: Sex,
}

/// Response returned by login-server verifying session validity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSessionResponse {
    pub valid: bool,
    pub account_id: AccountId,
    pub error_message: Option<String>,
}

/// Request sent by char-server to register its presence and public endpoint with login-server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCharServerRequest {
    pub server_id: u32,
    pub name: String,
    pub client_host: String,
    pub client_port: u16,
    pub internal_http_url: String,
    pub max_users: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCharServerResponse {
    pub accepted: bool,
    pub server_id: u32,
}

/// Request sent by map-server to char-server to register hosted maps and connection address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMapServerRequest {
    pub server_id: u32,
    pub name: String,
    pub client_host: String,
    pub client_port: u16,
    pub internal_http_url: String,
    pub maps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMapServerResponse {
    pub accepted: bool,
    pub registered_maps_count: usize,
}

/// Transfer ticket issued when a player selects a character and is routed to a map-server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapTransferToken {
    pub token: String,
    pub account_id: AccountId,
    pub char_id: CharId,
    pub login_id1: u32,
    pub destination_map: String,
    pub destination_pos: MapPosition,
    pub created_at_ms: u64,
}

/// Request sent by char-server to map-server informing it of an incoming client connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareMapTransferRequest {
    pub token: String,
    pub account_id: AccountId,
    pub char_id: CharId,
    pub login_id1: u32,
    pub destination_map: String,
    pub destination_pos: MapPosition,
    pub character: Option<CharacterSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareMapTransferResponse {
    pub accepted: bool,
}
