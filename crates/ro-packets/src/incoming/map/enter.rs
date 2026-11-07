use ro_core::{AccountId, CharId, ClientIntent, Sex};

use crate::incoming::helpers::{parse_sex, read_u32_le};
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_ENTER` (0x0436).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzEnterPacket {
    pub account_id: AccountId,
    pub char_id: CharId,
    pub login_id1: u32,
    pub client_tick: u32,
    pub sex: Sex,
}

impl CzEnterPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 17 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 17,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        let char_id = CharId(read_u32_le(&raw.payload, 4, raw.opcode)?);
        let login_id1 = read_u32_le(&raw.payload, 8, raw.opcode)?;
        let client_tick = read_u32_le(&raw.payload, 12, raw.opcode)?;
        let sex = if raw.payload.len() >= 21 {
            parse_sex(raw.payload[20])
        } else {
            parse_sex(raw.payload[16])
        };

        Ok(Self {
            account_id,
            char_id,
            login_id1,
            client_tick,
            sex,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::EnterMap {
            account_id: self.account_id,
            char_id: self.char_id,
            login_id1: self.login_id1,
            client_tick: self.client_tick,
            sex: self.sex,
        }
    }
}
