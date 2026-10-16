//! Client packet abstractions, wire framing, and decoded intents.
//!
//! Patterned after `reference/rust-ro/lib/packets/` and `reference/rathena/src/config/packets.hpp`.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::ids::{AccountId, CharId, MapPosition, SessionId, Sex};

/// Maximum allowed packet size across all servers to prevent unbounded allocation.
pub const MAX_PACKET_SIZE: usize = 65535;

/// Two-byte little-endian opcode identifying a Ragnarok Online network packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PacketOpcode(pub u16);

impl PacketOpcode {
    pub const CA_LOGIN: Self = Self(0x0064);
    pub const AC_ACCEPT_LOGIN: Self = Self(0x0069);
    pub const AC_REFUSE_LOGIN: Self = Self(0x006a);

    pub const CH_ENTER: Self = Self(0x0065);
    pub const HC_ACCEPT_ENTER: Self = Self(0x006b);
    pub const CH_SELECT_CHAR: Self = Self(0x0066);
    pub const HC_NOTIFY_ZONESVR: Self = Self(0x0071);
    pub const CH_MAKE_CHAR: Self = Self(0x0067);
    pub const HC_ACK_MAKE_CHAR: Self = Self(0x006d);
    pub const CH_DELETE_CHAR: Self = Self(0x0068);
    pub const HC_ACK_DELETE_CHAR: Self = Self(0x0070);

    pub const CZ_ENTER: Self = Self(0x0072);
    pub const ZC_ACCEPT_ENTER: Self = Self(0x0073);
    pub const CZ_NOTIFY_ACTORINIT: Self = Self(0x007d);
    pub const CZ_REQUEST_TIME: Self = Self(0x007e);
    pub const ZC_NOTIFY_TIME: Self = Self(0x007f);
    pub const CZ_REQUEST_MOVE: Self = Self(0x0085);
    pub const ZC_NOTIFY_PLAYERMOVE: Self = Self(0x0087);
    pub const CZ_CHANGE_DIRECTION: Self = Self(0x009b);
    pub const CZ_REQUEST_ACT: Self = Self(0x0089);
    pub const ZC_NOTIFY_ACT: Self = Self(0x008a);
    pub const CZ_REQNAME: Self = Self(0x0094);
    pub const ZC_ACK_REQNAME: Self = Self(0x0095);
    pub const ZC_NPCACK_MAPMOVE: Self = Self(0x0091);
    pub const ZC_NOTIFY_NEWENTRY: Self = Self(0x009d);
    pub const ZC_NOTIFY_STANDENTRY: Self = Self(0x009c);
    pub const ZC_NOTIFY_VANISH: Self = Self(0x0080);
    pub const ZC_PAR_CHANGE: Self = Self(0x00be);

    pub const fn raw(self) -> u16 {
        self.0
    }
}

impl fmt::Display for PacketOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#06x}", self.0)
    }
}

/// Packet size classification in the Ragnarok wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketLength {
    /// Packet size is invariant for a given PACKETVER.
    Fixed(usize),
    /// Packet size varies; bytes 2..4 hold a little-endian 16-bit total length header.
    Dynamic,
}

/// A framed, unparsed packet received from a client TCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub opcode: PacketOpcode,
    pub payload: Bytes,
}

impl RawPacket {
    pub fn new(opcode: PacketOpcode, payload: Bytes) -> Self {
        Self { opcode, payload }
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }
}

/// Delivery target for server-generated packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundTarget {
    Session(SessionId),
    Account(AccountId),
    Character(CharId),
    Broadcast(BroadcastScope),
}

/// Scope definition for spatial or broadcast packets emitted by map-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastScope {
    SelfOnly(CharId),
    AreaOfInterest { map_name: String, center: MapPosition },
    MapWide { map_name: String },
}

/// Outbound packet containing serialized wire bytes and a destination target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundPacket {
    pub target: OutboundTarget,
    pub data: Bytes,
}

impl OutboundPacket {
    pub fn new(target: OutboundTarget, data: Bytes) -> Self {
        Self { target, data }
    }

    pub fn for_session(session_id: SessionId, data: Bytes) -> Self {
        Self {
            target: OutboundTarget::Session(session_id),
            data,
        }
    }

    pub fn for_character(char_id: CharId, data: Bytes) -> Self {
        Self {
            target: OutboundTarget::Character(char_id),
            data,
        }
    }

    pub fn broadcast_aoi(map_name: String, center: MapPosition, data: Bytes) -> Self {
        Self {
            target: OutboundTarget::Broadcast(BroadcastScope::AreaOfInterest { map_name, center }),
            data,
        }
    }

    pub fn broadcast_map(map_name: String, data: Bytes) -> Self {
        Self {
            target: OutboundTarget::Broadcast(BroadcastScope::MapWide { map_name }),
            data,
        }
    }
}

/// High-level intents decoded from incoming client packets (`CZ_`).
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(thiserror::Error, Debug)]
pub enum PacketCodecError {
    #[error("IO failure: {0}")]
    Io(#[from] std::io::Error),

    #[error("Packet length {length} exceeds maximum permitted threshold {max} for opcode {opcode}")]
    PacketTooLarge {
        opcode: PacketOpcode,
        length: usize,
        max: usize,
    },

    #[error("Malformed packet header: {0}")]
    MalformedHeader(String),

    #[error("Unknown or unregistered packet opcode: {0}")]
    UnknownOpcode(PacketOpcode),
}

/// Table trait defining expected frame lengths for opcodes under a specific client version.
pub trait PacketLengthTable: Send + Sync {
    fn packet_length(&self, opcode: PacketOpcode, packetver: u32) -> Option<PacketLength>;
}

/// Trait defining the parser contract that translates a `RawPacket` into a high-level `ClientIntent`.
pub trait PacketIntentParser: Send + Sync {
    fn parse_intent(
        &self,
        raw: &RawPacket,
        packetver: u32,
    ) -> Result<Option<ClientIntent>, PacketCodecError>;
}
