//! Binary reading helpers for incoming packet frames.

use ro_core::ids::Sex;
use ro_core::packet::{PacketCodecError, PacketOpcode};

/// Reads a null-terminated or null-padded ASCII or UTF-8 string from a fixed byte slice.
pub fn read_fixed_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// Converts a raw wire byte to the domain Sex enum.
pub fn parse_sex(byte: u8) -> Sex {
    match byte {
        0 => Sex::Female,
        1 => Sex::Male,
        _ => Sex::Server,
    }
}

/// Reads a 2-byte little-endian unsigned integer from payload at the specified offset.
///
/// Returns `PacketCodecError::TruncatedPayload` when insufficient bytes remain.
pub fn read_u16_le(
    payload: &[u8],
    offset: usize,
    opcode: PacketOpcode,
) -> Result<u16, PacketCodecError> {
    let expected = offset + 2;
    if payload.len() < expected {
        return Err(PacketCodecError::TruncatedPayload {
            opcode,
            expected,
            actual: payload.len(),
        });
    }
    let slice: [u8; 2] = [payload[offset], payload[offset + 1]];
    Ok(u16::from_le_bytes(slice))
}

/// Reads a 4-byte little-endian unsigned integer from payload at the specified offset.
///
/// Returns `PacketCodecError::TruncatedPayload` when insufficient bytes remain.
pub fn read_u32_le(
    payload: &[u8],
    offset: usize,
    opcode: PacketOpcode,
) -> Result<u32, PacketCodecError> {
    let expected = offset + 4;
    if payload.len() < expected {
        return Err(PacketCodecError::TruncatedPayload {
            opcode,
            expected,
            actual: payload.len(),
        });
    }
    let slice: [u8; 4] = [
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ];
    Ok(u32::from_le_bytes(slice))
}
