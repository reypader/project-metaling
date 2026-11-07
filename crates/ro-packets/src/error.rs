use std::io;

use crate::PacketOpcode;

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
