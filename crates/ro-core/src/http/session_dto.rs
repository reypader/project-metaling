//! Inter-server HTTP DTOs for session tickets: validating a login and preparing a map transfer.

use serde::{Deserialize, Serialize};

use crate::char_select_dto::CharacterSummary;
use crate::ids_dto::{AccountId, CharId, MapPosition, Sex};

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
