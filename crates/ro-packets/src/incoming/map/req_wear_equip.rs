use ro_core::ClientIntent;

use crate::incoming::helpers::{read_u16_le, read_u32_le};
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQ_WEAR_EQUIP` (0x0998): wire index at offset 2, `EQP_*`
/// position mask at 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzReqWearEquipPacket {
    pub index: u16,
    pub position: u32,
}

impl CzReqWearEquipPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let index = read_u16_le(&raw.payload, 0, raw.opcode)?;
        let position = read_u32_le(&raw.payload, 2, raw.opcode)?;
        Ok(Self { index, position })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::EquipItem {
            index: self.index,
            position: self.position,
        }
    }
}
