//! Account records.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, Sex};

/// Status state of a player account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountState {
    Active,
    Banned,
    Expired,
    Locked,
}

/// Data transfer object representing an account record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub password_hash: String,
    pub sex: Sex,
    pub email: String,
    pub group_id: i32,
    pub state: AccountState,
    pub unban_time: Option<u64>,
    pub expiration_time: Option<u64>,
}
