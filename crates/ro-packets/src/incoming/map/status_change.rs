use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_STATUS_CHANGE` (0x00bb): `var:u16` at offset 2, `amount:u8`
/// at offset 4. The amount is unsigned: a captured 231 (`0xe7`) raised the stat as far as it
/// could go (catalog `packets/status.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzStatusChangePacket {
    pub var: u16,
    pub amount: u8,
}

impl CzStatusChangePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let var = read_u16_le(&raw.payload, 0, raw.opcode)?;
        let amount = *raw
            .payload
            .get(2)
            .ok_or(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 3,
                actual: raw.payload.len(),
            })?;
        Ok(Self { var, amount })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RaiseStat {
            var: self.var,
            amount: self.amount,
        }
    }
}
