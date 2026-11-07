//! Helpers shared by the test modules.

use bytes::Bytes;
use ro_core::{ServerEvent, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, DecodedFrame, EventEncoder, ModernEventEncoder, PacketCodecError, PacketOpcode,
    RawPacket,
};

pub fn decode(op: u16, payload: &[u8]) -> Result<DecodedFrame, PacketCodecError> {
    let raw = RawPacket::new(PacketOpcode(op), Bytes::copy_from_slice(payload));
    decode_frame(&raw, TARGET_PACKETVER)
}

pub fn encode(event: ServerEvent) -> Vec<u8> {
    ModernEventEncoder
        .encode_event(&event, TARGET_PACKETVER)
        .expect("encodes")
        .to_vec()
}

/// `opcode` little-endian, then `payload`.
pub fn frame(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = opcode.to_le_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}
