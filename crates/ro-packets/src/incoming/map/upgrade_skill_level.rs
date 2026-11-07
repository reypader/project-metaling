use ro_core::ClientIntent;

use crate::incoming::helpers::read_u16_le;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_UPGRADE_SKILLLEVEL` (0x0112): `skillId:u16` at offset 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzUpgradeSkillLevelPacket {
    pub id: u16,
}

impl CzUpgradeSkillLevelPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self {
            id: read_u16_le(&raw.payload, 0, raw.opcode)?,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::UpgradeSkill { id: self.id }
    }
}
