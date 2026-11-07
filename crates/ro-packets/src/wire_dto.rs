//! Wire-level framing types: packet length classes and the raw framed packet.

use bytes::Bytes;

use crate::PacketOpcode;

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
