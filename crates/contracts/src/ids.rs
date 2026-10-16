//! Core domain identifiers, newtypes, and primitives.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CharId(pub u32);

/// Modern PACKETVER uses 32-bit Item IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u32);

/// Unique identifier for an active network session connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sex {
    Female = 0,
    Male = 1,
    Server = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTicket {
    pub account_id: AccountId,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: Sex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPosition {
    pub x: u16,
    pub y: u16,
    pub dir: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub char_id: CharId,
    pub account_id: AccountId,
    pub slot: u8,
    pub name: String,
    pub sex: Sex,
    pub job_class: u16,
    pub base_level: u16,
    pub job_level: u16,
    pub hair_style: u16,
    pub hair_color: u16,
    pub last_map: String,
    pub last_pos: MapPosition,
}
