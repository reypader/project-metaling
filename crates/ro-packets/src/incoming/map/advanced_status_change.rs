use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_ADVANCED_STATUS_CHANGE` (0x0b24): `var:i16` at offset 2 and
/// `amount:i16` at offset 4. The var is kept as its u16 bit pattern (only 219..224 are valid,
/// and the refusal echoes it in a u16 field); the amount stays signed so the engine can drop a
/// negative one (catalog `packets/status.md` "Trait stat-up (0x0b24)").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzAdvancedStatusChangePacket {
    pub var: u16,
    pub amount: i16,
}

impl CzAdvancedStatusChangePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let var = read_u16_le(&raw.payload, 0, raw.opcode)?;
        let amount = read_u16_le(&raw.payload, 2, raw.opcode)?;
        Ok(Self {
            var,
            amount: i16::from_le_bytes(amount.to_le_bytes()),
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RaiseTrait {
            var: self.var,
            amount: self.amount,
        }
    }
}
