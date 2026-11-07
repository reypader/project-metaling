use crate::{PacketLength, PacketOpcode};

/// Table trait defining expected frame lengths for opcodes under a specific client version.
pub trait PacketLengthTable: Send + Sync {
    fn packet_length(&self, opcode: PacketOpcode, packetver: u32) -> Option<PacketLength>;
}
