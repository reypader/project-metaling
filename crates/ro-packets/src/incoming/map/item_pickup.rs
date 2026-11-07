use ro_core::ClientIntent;

use crate::incoming::helpers::read_u32_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_ITEM_PICKUP` (0x0362): the floor item id at offset 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzItemPickupPacket {
    pub floor_id: u32,
}

impl CzItemPickupPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let floor_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { floor_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::PickUpItem {
            floor_id: self.floor_id,
        }
    }
}
