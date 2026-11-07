use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQ_TAKEOFF_EQUIP` (0x00ab): wire index at offset 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzReqTakeoffEquipPacket {
    pub index: u16,
}

impl CzReqTakeoffEquipPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let index = read_u16_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { index })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::UnequipItem { index: self.index }
    }
}
