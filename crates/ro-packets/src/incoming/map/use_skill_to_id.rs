use ro_core::ClientIntent;

use crate::incoming::helpers::{read_u16_le, read_u32_le};
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_USE_SKILL_TO_ID` (0x0438): `level:u16` at offset 2,
/// `skill_id:u16` at 4, `target:u32` at 6 (catalog `scripting/skills.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzUseSkillToIdPacket {
    pub level: u16,
    pub skill_id: u16,
    pub target: u32,
}

impl CzUseSkillToIdPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self {
            level: read_u16_le(&raw.payload, 0, raw.opcode)?,
            skill_id: read_u16_le(&raw.payload, 2, raw.opcode)?,
            target: read_u32_le(&raw.payload, 4, raw.opcode)?,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::UseSkill {
            level: self.level,
            skill_id: self.skill_id,
            target: self.target,
        }
    }
}
