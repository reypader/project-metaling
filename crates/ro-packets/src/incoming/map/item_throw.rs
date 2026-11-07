use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_ITEM_THROW` (0x0363): wire index at offset 2, amount at 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzItemThrowPacket {
    pub index: u16,
    pub amount: u16,
}

impl CzItemThrowPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let index = read_u16_le(&raw.payload, 0, raw.opcode)?;
        let amount = read_u16_le(&raw.payload, 2, raw.opcode)?;
        Ok(Self { index, amount })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::DropItem {
            index: self.index,
            amount: self.amount,
        }
    }
}
