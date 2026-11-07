use ro_core::{AccountId, ClientIntent, Sex};

use crate::incoming::helpers::{parse_sex, read_u16_le, read_u32_le};
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CH_ENTER` (0x0065).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChEnterPacket {
    pub account_id: AccountId,
    pub login_id1: u32,
    pub login_id2: u32,
    pub client_version: u16,
    pub sex: Sex,
}

impl ChEnterPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 15 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 15,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        let login_id1 = read_u32_le(&raw.payload, 4, raw.opcode)?;
        let login_id2 = read_u32_le(&raw.payload, 8, raw.opcode)?;
        let client_version = read_u16_le(&raw.payload, 12, raw.opcode)?;
        let sex = parse_sex(raw.payload[14]);

        Ok(Self {
            account_id,
            login_id1,
            login_id2,
            client_version,
            sex,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::EnterCharServer {
            account_id: self.account_id,
            login_id1: self.login_id1,
            login_id2: self.login_id2,
            client_version: self.client_version,
            sex: self.sex,
        }
    }
}
