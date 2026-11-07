//! Session tickets and char-server registrations that the login service tracks.

use std::time::Instant;

use ro_core::{AccountId, CharServerInfo, Sex};

/// Active session ticket issued upon successful authentication.
#[derive(Debug, Clone)]
pub struct ActiveSessionTicket {
    #[allow(dead_code)]
    pub account_id: AccountId,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: Sex,
    pub expires_at: Instant,
}

/// Registered character server state tracked by login-server.
#[derive(Debug, Clone)]
pub struct RegisteredCharServer {
    pub server_id: u32,
    pub info: CharServerInfo,
    #[allow(dead_code)]
    pub internal_http_url: String,
    #[allow(dead_code)]
    pub max_users: u32,
    pub last_heartbeat_ms: u64,
}
