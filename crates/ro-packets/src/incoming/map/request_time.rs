use ro_core::ClientIntent;

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQUEST_TIME` (0x0360).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestTimePacket {
    pub client_tick: u32,
}

impl CzRequestTimePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let client_tick = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { client_tick })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestTime {
            client_tick: self.client_tick,
        }
    }
}
