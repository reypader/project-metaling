//! Client packet abstractions, wire framing, and decoded intents.
//!
//! Patterned after `reference/rust-ro/lib/packets/` and `reference/rathena/src/config/packets.hpp`.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::ids::{AccountId, CharId, CharServerInfo, CharacterSummary, MapPosition, SessionId, Sex};

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

impl From<BroadcastScope> for OutboundTarget {
    fn from(scope: BroadcastScope) -> Self {
        Self::Broadcast(scope)
    }
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

/// High-level intents decoded from incoming client packets (`CA_`, `CH_`, `CZ_`).
///
/// Variants contain only the fields physically present in the wire frame.
/// Session-level identity (`SessionId`, `AccountId`, `CharId`) is carried by [`SessionIntent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientIntent {
    Login {
        username: String,
        password: String,
        client_version: u32,
        client_type: u8,
    },
    EnterCharServer {
        account_id: AccountId,
        login_id1: u32,
        login_id2: u32,
        client_version: u16,
        sex: Sex,
    },
    SelectCharacter {
        slot: u8,
    },
    EnterMap {
        account_id: AccountId,
        char_id: CharId,
        login_id1: u32,
        client_tick: u32,
        sex: Sex,
    },
    NotifyActorInit,
    RequestTime {
        client_tick: u32,
    },
    RequestMove {
        dest_x: u16,
        dest_y: u16,
    },
    ChangeDirection {
        head_dir: u16,
        body_dir: u8,
    },
    RequestAction {
        target_id: u32,
        action_type: u8,
    },
    RequestEntityName {
        entity_id: u32,
    },
}

/// Session-authenticated envelope wrapping a wire `ClientIntent` for dispatch to server logic actors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIntent {
    pub session_id: SessionId,
    pub account_id: Option<AccountId>,
    pub char_id: Option<CharId>,
    pub intent: ClientIntent,
}

impl SessionIntent {
    pub fn new(
        session_id: SessionId,
        account_id: Option<AccountId>,
        char_id: Option<CharId>,
        intent: ClientIntent,
    ) -> Self {
        Self {
            session_id,
            account_id,
            char_id,
            intent,
        }
    }
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

    #[error("Unsupported server event for serialization: {0}")]
    UnsupportedEvent(String),
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

/// High-level domain events emitted by logic modules and engine systems, decoupled from wire serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    LoginAccepted {
        account_id: AccountId,
        login_id1: u32,
        login_id2: u32,
        sex: Sex,
        char_servers: Vec<CharServerInfo>,
    },
    LoginRefused {
        error_code: u8,
    },
    CharacterListLoaded {
        account_id: AccountId,
        total_slots: u8,
        characters: Vec<CharacterSummary>,
    },
    CharacterSelected {
        char_id: CharId,
        map_name: String,
        map_host: [u8; 4],
        map_port: u16,
    },
    AcceptEnter {
        server_tick: u32,
        pos: MapPosition,
    },
    TimeSynchronized {
        server_tick: u32,
    },
    EntityMoved {
        char_id: CharId,
        start_pos: MapPosition,
        dest_pos: MapPosition,
        move_start_tick: u32,
    },
    EntityAppeared {
        char_id: CharId,
        pos: MapPosition,
        name: String,
        hair_style: u16,
        hair_color: u16,
    },
    EntityVanished {
        char_id: CharId,
        vanish_type: u8,
    },
    EntityAction {
        source_id: u32,
        target_id: u32,
        action_type: u8,
        action_tick: u32,
    },
    ParameterChanged {
        var_id: u16,
        value: i32,
    },
}

/// High-level domain event tagged with its target broadcast scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedServerEvent {
    pub scope: BroadcastScope,
    pub event: ServerEvent,
}

impl ScopedServerEvent {
    pub fn new(scope: BroadcastScope, event: ServerEvent) -> Self {
        Self { scope, event }
    }
}

/// Trait defining the serializer contract that translates a domain `ServerEvent` into wire `Bytes`.
pub trait EventEncoder: Send + Sync {
    fn encode_event(
        &self,
        event: &ServerEvent,
        packetver: u32,
    ) -> Result<Bytes, PacketCodecError>;

    fn encode_scoped_event(
        &self,
        scoped: &ScopedServerEvent,
        packetver: u32,
    ) -> Result<OutboundPacket, PacketCodecError> {
        let data = self.encode_event(&scoped.event, packetver)?;
        Ok(OutboundPacket::new(scoped.scope.clone().into(), data))
    }
}
