use ro_core::{AccountId, ClientIntent};

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `PING` (0x0187).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingPacket {
    pub account_id: AccountId,
}

impl PingPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { account_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CharPing {
            account_id: self.account_id,
        }
    }
}
