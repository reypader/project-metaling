use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_RESTART` (0x00b2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRestartPacket {
    pub restart_type: u8,
}

impl CzRestartPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.is_empty() {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 1,
                actual: raw.payload.len(),
            });
        }

        Ok(Self {
            restart_type: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::Restart {
            restart_type: self.restart_type,
        }
    }
}
