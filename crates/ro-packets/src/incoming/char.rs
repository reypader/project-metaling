//! Incoming character server client packet decoders supporting CH_* opcodes.

use ro_contracts::ids::{AccountId, CharId, Sex};
use ro_contracts::packet::{ClientIntent, PacketCodecError, RawPacket};

use crate::incoming::helpers::{parse_sex, read_fixed_string, read_u16_le, read_u32_le};

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

/// Decoded representation of `CH_SELECT_CHAR` (0x0066).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChSelectCharPacket {
    pub slot: u8,
}

impl ChSelectCharPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.is_empty() {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 1,
                actual: raw.payload.len(),
            });
        }

        Ok(Self {
            slot: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::SelectCharacter { slot: self.slot }
    }
}

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

/// Decoded representation of `CH_DELETE_CHAR3_RESERVED` (0x0827).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChDeleteChar3ReservedPacket {
    pub char_id: CharId,
}

impl ChDeleteChar3ReservedPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let char_id = CharId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { char_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::ReserveDeleteCharacter {
            char_id: self.char_id,
        }
    }
}

/// Decoded representation of `CH_DELETE_CHAR3_CANCEL` (0x082b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChDeleteChar3CancelPacket {
    pub char_id: CharId,
}

impl ChDeleteChar3CancelPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let char_id = CharId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { char_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CancelDeleteCharacter {
            char_id: self.char_id,
        }
    }
}

/// Decoded representation of `CH_CHARLIST_REQ` (0x09a1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChCharListReqPacket;

impl ChCharListReqPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestCharacterList
    }
}

/// Decoded representation of `PING` (0x0187).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingPacket {
    pub account_id: AccountId,
}

impl PingPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { account_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CharPing {
            account_id: self.account_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use ro_contracts::packet::PacketOpcode;

    #[test]
    fn test_decode_ch_enter() {
        let mut payload = vec![0u8; 15];
        payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        payload[4..8].copy_from_slice(&11111u32.to_le_bytes());
        payload[8..12].copy_from_slice(&22222u32.to_le_bytes());
        payload[12..14].copy_from_slice(&20u16.to_le_bytes());
        payload[14] = 1; // Male

        let raw = RawPacket::new(PacketOpcode(0x0065), Bytes::from(payload));
        let decoded = ChEnterPacket::decode(&raw).unwrap();

        assert_eq!(decoded.account_id, AccountId(2000001));
        assert_eq!(decoded.login_id1, 11111);
        assert_eq!(decoded.login_id2, 22222);
        assert_eq!(decoded.client_version, 20);
        assert_eq!(decoded.sex, Sex::Male);
    }

    #[test]
    fn test_decode_ch_select_char() {
        let payload = vec![3u8];
        let raw = RawPacket::new(PacketOpcode(0x0066), Bytes::from(payload));
        let decoded = ChSelectCharPacket::decode(&raw).unwrap();

        assert_eq!(decoded.slot, 3);
    }

    #[test]
    fn test_ch_make_char_invalid_sex() {
        let mut payload = vec![0u8; 34];
        payload[0..4].copy_from_slice(b"Hero");
        payload[24] = 0;
        payload[25..27].copy_from_slice(&1u16.to_le_bytes());
        payload[27..29].copy_from_slice(&2u16.to_le_bytes());
        payload[29..33].copy_from_slice(&0u32.to_le_bytes());
        payload[33] = 99;

        let raw = RawPacket::new(PacketOpcode(0x0a39), Bytes::from(payload));
        match ChMakeCharPacket::decode(&raw) {
            Err(PacketCodecError::InvalidField {
                opcode,
                field,
                value,
            }) => {
                assert_eq!(opcode, PacketOpcode(0x0a39));
                assert_eq!(field, "sex");
                assert_eq!(value, 99);
            }
            other => panic!("Expected InvalidField error, got {:?}", other),
        }
    }

    #[test]
    fn test_truncated_char_payload_errors() {
        let short_payload = Bytes::from_static(&[0u8; 2]);

        let raw_ch_enter = RawPacket::new(PacketOpcode(0x0065), short_payload);
        match ChEnterPacket::decode(&raw_ch_enter) {
            Err(PacketCodecError::TruncatedPayload {
                opcode,
                expected,
                actual,
            }) => {
                assert_eq!(opcode, PacketOpcode(0x0065));
                assert_eq!(expected, 15);
                assert_eq!(actual, 2);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }

        let empty_payload = Bytes::new();
        let raw_select = RawPacket::new(PacketOpcode(0x0066), empty_payload);
        match ChSelectCharPacket::decode(&raw_select) {
            Err(PacketCodecError::TruncatedPayload {
                opcode,
                expected,
                actual,
            }) => {
                assert_eq!(opcode, PacketOpcode(0x0066));
                assert_eq!(expected, 1);
                assert_eq!(actual, 0);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }
    }
}
