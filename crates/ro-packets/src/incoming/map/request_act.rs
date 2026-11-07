use ro_core::ClientIntent;

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQUEST_ACT` (0x0437).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestActPacket {
    pub target_id: u32,
    pub action_type: u8,
}

impl CzRequestActPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 5 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 5,
                actual: raw.payload.len(),
            });
        }

        let target_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
        let action_type = raw.payload[4];
        Ok(Self {
            target_id,
            action_type,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestAction {
            target_id: self.target_id,
            action_type: self.action_type,
        }
    }
}
