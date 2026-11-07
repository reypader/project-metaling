use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQ_DISCONNECT` (0x018a), sent by "Exit to Windows".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzReqDisconnectPacket {
    pub disconnect_type: u16,
}

impl CzReqDisconnectPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self {
            disconnect_type: read_u16_le(&raw.payload, 0, raw.opcode)?,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestDisconnect {
            disconnect_type: self.disconnect_type,
        }
    }
}
