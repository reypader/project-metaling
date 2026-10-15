//! Shared domain contracts, IDs, events, and repository traits.
//! Owned exclusively by the Main Orchestrator Agent.

use serde::{Deserialize, Serialize};

/// Target client version from `reference/rathena/src/config/packets.hpp`.
pub const TARGET_PACKETVER: u32 = 20211103;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CharId(pub u32);

/// Modern PACKETVER uses 32-bit Item IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u32);

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

/// High-level intents decoded from client-to-map (`CZ_`) packets.
#[derive(Debug, Clone)]
pub enum ClientIntent {
    EnterMap {
        account_id: AccountId,
        char_id: CharId,
        login_id1: u32,
        client_tick: u32,
        sex: Sex,
    },
    NotifyActorInit {
        char_id: CharId,
    },
    RequestTime {
        char_id: CharId,
        client_tick: u32,
    },
    RequestMove {
        char_id: CharId,
        dest_x: u16,
        dest_y: u16,
    },
    ChangeDirection {
        char_id: CharId,
        head_dir: u16,
        body_dir: u8,
    },
    RequestAction {
        char_id: CharId,
        target_id: u32,
        action_type: u8,
    },
    RequestEntityName {
        char_id: CharId,
        entity_id: u32,
    },
}

/// Broadcast scope for outbound map packets (`ZC_`).
#[derive(Debug, Clone)]
pub enum BroadcastScope {
    SelfOnly(CharId),
    AreaOfInterest { map_name: String, center: MapPosition },
    MapWide { map_name: String },
}
