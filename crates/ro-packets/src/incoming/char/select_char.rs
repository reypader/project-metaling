use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CH_SELECT_CHAR` (0x0066).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChSelectCharPacket {
    pub slot: u8,
}

impl ChSelectCharPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.is_empty() {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 1,
                actual: raw.payload.len(),
            });
        }

        Ok(Self {
            slot: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::SelectCharacter { slot: self.slot }
    }
}
