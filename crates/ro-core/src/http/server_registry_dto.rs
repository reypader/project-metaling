//! Inter-server HTTP DTOs for server registration, heartbeats and health checks.

use serde::{Deserialize, Serialize};

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
