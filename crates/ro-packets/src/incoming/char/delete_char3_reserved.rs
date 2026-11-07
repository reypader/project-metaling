use ro_core::{CharId, ClientIntent};

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CH_DELETE_CHAR3_RESERVED` (0x0827).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChDeleteChar3ReservedPacket {
    pub char_id: CharId,
}

impl ChDeleteChar3ReservedPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let char_id = CharId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { char_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::ReserveDeleteCharacter {
            char_id: self.char_id,
        }
    }
}
