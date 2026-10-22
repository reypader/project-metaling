//! Wire-level framing types and the parser/encoder traits.

use bytes::Bytes;
use std::io;

use ro_core::message::{ClientIntent, ServerEvent};

use crate::opcode::PacketOpcode;

/// Maximum allowed packet size across all servers to prevent unbounded allocation.
pub const MAX_PACKET_SIZE: usize = 65535;

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

/// Trait defining the serializer contract that translates a domain `ServerEvent` into wire `Bytes`.
pub trait EventEncoder: Send + Sync {
    fn encode_event(&self, event: &ServerEvent, packetver: u32) -> Result<Bytes, PacketCodecError>;
}
