use ro_core::ClientIntent;

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQ_TAKEOFF_EQUIP_ALL` (0x0bf5): `EQP_*` mask at offset 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzReqTakeoffEquipAllPacket {
    pub location: u32,
}

impl CzReqTakeoffEquipAllPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let location = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { location })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::UnequipAll {
            location: self.location,
        }
    }
}
