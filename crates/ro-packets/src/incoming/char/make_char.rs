use ro_core::{ClientIntent, Sex};

use crate::incoming::helpers::{read_fixed_string, read_u16_le, read_u32_le};
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CH_MAKE_CHAR` (0x0a39).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChMakeCharPacket {
    pub name: String,
    pub slot: u8,
    pub hair_color: u16,
    pub hair_style: u16,
    pub starting_job: u32,
    pub sex: Sex,
}

impl ChMakeCharPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 34 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 34,
                actual: raw.payload.len(),
            });
        }

        let name = read_fixed_string(&raw.payload[0..24]);
        let slot = raw.payload[24];
        let hair_color = read_u16_le(&raw.payload, 25, raw.opcode)?;
        let hair_style = read_u16_le(&raw.payload, 27, raw.opcode)?;
        let starting_job = read_u32_le(&raw.payload, 29, raw.opcode)?;
        let sex = match raw.payload[33] {
            0 => Sex::Female,
            1 => Sex::Male,
            2 => Sex::Server,
            other => {
                return Err(PacketCodecError::InvalidField {
                    opcode: raw.opcode,
                    field: "sex",
                    value: other as u32,
                });
            }
        };

        Ok(Self {
            name,
            slot,
            hair_color,
            hair_style,
            starting_job,
            sex,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CreateCharacter {
            name: self.name.clone(),
            slot: self.slot,
            hair_color: self.hair_color,
            hair_style: self.hair_style,
            starting_job: self.starting_job,
            sex: self.sex,
        }
    }
}
