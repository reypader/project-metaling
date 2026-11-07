use ro_core::ClientIntent;

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQNAME` (0x0368).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzReqNamePacket {
    pub entity_id: u32,
}

impl CzReqNamePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let entity_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { entity_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestEntityName {
            entity_id: self.entity_id,
        }
    }
}
