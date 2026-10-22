//! Client packet abstractions, wire framing, and decoded intents.

use bytes::Bytes;
use std::fmt;
use std::io;

use crate::ids::{
    AccountId, CharId, CharServerInfo, CharacterSummary, MapPosition, SessionId, Sex,
};

/// Maximum allowed packet size across all servers to prevent unbounded allocation.
pub const MAX_PACKET_SIZE: usize = 65535;

pub mod opcode;
pub use opcode::PacketOpcode;

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
    /// Everyone whose character is in AoI range of `center`, minus `exclude`.
    AreaOfInterest {
        map_name: String,
        center: MapPosition,
        exclude: Option<CharId>,
    },
    MapWide {
        map_name: String,
    },
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

    pub fn broadcast_aoi(
        map_name: String,
        center: MapPosition,
        exclude: Option<CharId>,
        data: Bytes,
    ) -> Self {
        Self {
            target: OutboundTarget::Broadcast(BroadcastScope::AreaOfInterest {
                map_name,
                center,
                exclude,
            }),
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
#[derive(Clone, PartialEq, Eq)]
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
    RequestCharacterList,
    SelectCharacter {
        slot: u8,
    },
    CreateCharacter {
        name: String,
        slot: u8,
        hair_color: u16,
        hair_style: u16,
        starting_job: u32,
        sex: Sex,
    },
    ReserveDeleteCharacter {
        char_id: CharId,
    },
    CancelDeleteCharacter {
        char_id: CharId,
    },
    CharPing {
        account_id: AccountId,
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
    Restart {
        restart_type: u8,
    },
}

impl fmt::Debug for ClientIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login {
                username,
                password: _,
                client_version,
                client_type,
            } => f
                .debug_struct("Login")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .field("client_version", client_version)
                .field("client_type", client_type)
                .finish(),
            Self::EnterCharServer {
                account_id,
                login_id1,
                login_id2,
                client_version,
                sex,
            } => f
                .debug_struct("EnterCharServer")
                .field("account_id", account_id)
                .field("login_id1", login_id1)
                .field("login_id2", login_id2)
                .field("client_version", client_version)
                .field("sex", sex)
                .finish(),
            Self::RequestCharacterList => write!(f, "RequestCharacterList"),
            Self::SelectCharacter { slot } => f
                .debug_struct("SelectCharacter")
                .field("slot", slot)
                .finish(),
            Self::CreateCharacter {
                name,
                slot,
                hair_color,
                hair_style,
                starting_job,
                sex,
            } => f
                .debug_struct("CreateCharacter")
                .field("name", name)
                .field("slot", slot)
                .field("hair_color", hair_color)
                .field("hair_style", hair_style)
                .field("starting_job", starting_job)
                .field("sex", sex)
                .finish(),
            Self::ReserveDeleteCharacter { char_id } => f
                .debug_struct("ReserveDeleteCharacter")
                .field("char_id", char_id)
                .finish(),
            Self::CancelDeleteCharacter { char_id } => f
                .debug_struct("CancelDeleteCharacter")
                .field("char_id", char_id)
                .finish(),
            Self::CharPing { account_id } => f
                .debug_struct("CharPing")
                .field("account_id", account_id)
                .finish(),
            Self::EnterMap {
                account_id,
                char_id,
                login_id1,
                client_tick,
                sex,
            } => f
                .debug_struct("EnterMap")
                .field("account_id", account_id)
                .field("char_id", char_id)
                .field("login_id1", login_id1)
                .field("client_tick", client_tick)
                .field("sex", sex)
                .finish(),
            Self::NotifyActorInit => write!(f, "NotifyActorInit"),
            Self::RequestTime { client_tick } => f
                .debug_struct("RequestTime")
                .field("client_tick", client_tick)
                .finish(),
            Self::RequestMove { dest_x, dest_y } => f
                .debug_struct("RequestMove")
                .field("dest_x", dest_x)
                .field("dest_y", dest_y)
                .finish(),
            Self::ChangeDirection { head_dir, body_dir } => f
                .debug_struct("ChangeDirection")
                .field("head_dir", head_dir)
                .field("body_dir", body_dir)
                .finish(),
            Self::RequestAction {
                target_id,
                action_type,
            } => f
                .debug_struct("RequestAction")
                .field("target_id", target_id)
                .field("action_type", action_type)
                .finish(),
            Self::RequestEntityName { entity_id } => f
                .debug_struct("RequestEntityName")
                .field("entity_id", entity_id)
                .finish(),
            Self::Restart { restart_type } => f
                .debug_struct("Restart")
                .field("restart_type", restart_type)
                .finish(),
        }
    }
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
    Io(#[from] io::Error),

    #[error(
        "Packet length {length} exceeds maximum permitted threshold {max} for opcode {opcode:?}"
    )]
    PacketTooLarge {
        opcode: PacketOpcode,
        length: usize,
        max: usize,
    },

    #[error("Truncated payload for opcode {opcode:?}: expected {expected} bytes, got {actual}")]
    TruncatedPayload {
        opcode: PacketOpcode,
        expected: usize,
        actual: usize,
    },

    #[error("Invalid field value in opcode {opcode:?}: field '{field}' has invalid value {value}")]
    InvalidField {
        opcode: PacketOpcode,
        field: &'static str,
        value: u32,
    },

    #[error("Malformed packet header: {0}")]
    MalformedHeader(String),

    #[error("Unknown or unregistered packet opcode: {0:?}")]
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

/// Appearance snapshot shared by the unit entry packets (`0x09fd`/`0x09fe`/`0x09ff`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAppearance {
    /// Wire id (`bl.id`): the account_id for players.
    pub gid: u32,
    /// Written to the `GID` field; `None` (non-players) writes 0.
    pub char_id: Option<CharId>,
    pub name: String,
    pub job: u16,
    pub sex: Sex,
    pub hair_style: u16,
    pub hair_color: u16,
    /// 0 straight, 1 clockwise, 2 counter-clockwise.
    pub head_dir: u16,
    /// Cell and body direction.
    pub pos: MapPosition,
    pub speed: u16,
    /// rAthena `dead_sit`: 0 standing, 1 dead, 2 sitting. Goes only to the `0x09ff` `state`
    /// byte (offset 68). It is NOT the wire `body_state` (`sc.opt1`, offset 15), which stays 0.
    pub dead_sit: u8,
    pub clevel: u16,
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
    CharacterListRefused {
        error_code: u8,
    },
    CharacterPageLoaded {
        characters: Vec<CharacterSummary>,
    },
    CharacterSelected {
        char_id: CharId,
        map_name: String,
        map_host: [u8; 4],
        map_port: u16,
    },
    CharacterCreated {
        character: CharacterSummary,
    },
    CharacterCreateRefused {
        error_code: u8,
    },
    CharacterDeleteReserved {
        char_id: CharId,
        result: u32,
        delete_delay_seconds: u32,
    },
    CharacterDeleteCancelled {
        char_id: CharId,
        result: u32,
    },
    Aid {
        account_id: AccountId,
    },
    InventoryExpansionInfo {
        expansion_size: i16,
    },
    AcceptEnter {
        account_id: AccountId,
        server_tick: u32,
        pos: MapPosition,
    },
    TimeSynchronized {
        server_tick: u32,
    },
    /// `0x0087` to the mover: its own walk started or was redirected.
    PlayerMoved {
        start: (u16, u16),
        dest: (u16, u16),
        move_start_tick: u32,
    },
    /// `0x09fd`: a walking unit, sent to observers. The encoder takes the cells from
    /// `start`/`dest`; `unit.pos` is ignored (`0x09fd` has no `pos_dir` field).
    UnitWalking {
        unit: UnitAppearance,
        start: (u16, u16),
        dest: (u16, u16),
        move_start_tick: u32,
    },
    /// `0x09ff`: an idle unit came into an observer's view.
    UnitStanding {
        unit: UnitAppearance,
    },
    /// `0x09fe`: a unit spawned in view.
    UnitSpawned {
        unit: UnitAppearance,
    },
    /// `0x0080`: `vanish_type` 0 out of sight, 1 died, 2 logged out, 3 teleport.
    UnitVanished {
        gid: u32,
        vanish_type: u8,
    },
    /// `0x0088`: a unit's walk stopped at (x, y).
    UnitStopped {
        gid: u32,
        x: u16,
        y: u16,
    },
    /// `0x009c`.
    DirectionChanged {
        gid: u32,
        head_dir: u16,
        body_dir: u8,
    },
    /// `0x0a30`: reply to `CZ_REQNAME` for a player.
    AckEntityName {
        gid: u32,
        name: String,
        party_name: String,
        guild_name: String,
        position_name: String,
        title_id: i32,
    },
    /// `0x0091`.
    PlayerMapMove {
        dest_map: String,
        dest_x: u16,
        dest_y: u16,
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
    RestartAck {
        result: u8,
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
    fn encode_event(&self, event: &ServerEvent, packetver: u32) -> Result<Bytes, PacketCodecError>;

    fn encode_scoped_event(
        &self,
        scoped: &ScopedServerEvent,
        packetver: u32,
    ) -> Result<OutboundPacket, PacketCodecError> {
        let data = self.encode_event(&scoped.event, packetver)?;
        Ok(OutboundPacket::new(scoped.scope.clone().into(), data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_opcode_name_and_display_known() {
        assert_eq!(PacketOpcode::CA_LOGIN.name(), Some("CA_LOGIN"));
        assert_eq!(PacketOpcode::CA_LOGIN.to_string(), "0x0064 (CA_LOGIN)");

        assert_eq!(PacketOpcode::ZC_NOTIFY_TIME.name(), Some("ZC_NOTIFY_TIME"));
        assert_eq!(
            PacketOpcode::ZC_NOTIFY_TIME.to_string(),
            "0x007f (ZC_NOTIFY_TIME)"
        );

        assert_eq!(
            PacketOpcode::HC_REFUSE_DELETECHAR.name(),
            Some("HC_REFUSE_DELETECHAR")
        );
        assert_eq!(
            PacketOpcode::HC_ACK_DELETE_CHAR.name(),
            Some("HC_REFUSE_DELETECHAR")
        );
        assert_eq!(
            PacketOpcode::HC_ACK_DELETE_CHAR.to_string(),
            "0x0070 (HC_REFUSE_DELETECHAR)"
        );

        assert_eq!(
            PacketOpcode::HC_ACK_MAKE_CHAR.name(),
            Some("HC_ACK_MAKE_CHAR")
        );
        assert_eq!(
            PacketOpcode::HC_ACCEPT_MAKECHAR.name(),
            Some("HC_ACK_MAKE_CHAR")
        );
        assert_eq!(
            PacketOpcode::HC_ACCEPT_MAKECHAR.to_string(),
            "0x0b6f (HC_ACK_MAKE_CHAR)"
        );
    }

    #[test]
    fn test_packet_opcode_name_and_display_unknown() {
        let unknown = PacketOpcode(0x0001);
        assert_eq!(unknown.name(), None);
        assert_eq!(unknown.to_string(), "0x0001");

        let unknown_high = PacketOpcode(0xffff);
        assert_eq!(unknown_high.name(), None);
        assert_eq!(unknown_high.to_string(), "0xffff");
    }
}
