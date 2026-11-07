use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_USE_ITEM2` (0x0439): `index:u16` at offset 2 (slot + 2), then
/// the user's AID, which is not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzUseItemPacket {
    pub index: u16,
}

impl CzUseItemPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self {
            index: read_u16_le(&raw.payload, 0, raw.opcode)?,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::UseItem { index: self.index }
    }
}
