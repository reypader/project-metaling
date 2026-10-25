//! Incoming map server client packet decoders supporting CZ_* opcodes.

use crate::{PacketCodecError, RawPacket};
use ro_core::{AccountId, CharId, ClientIntent, Sex};

use crate::coords::unpack_pos_dir;
use crate::incoming::helpers::{parse_sex, read_u16_le, read_u32_le};

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

/// Decoded representation of `CZ_NOTIFY_ACTORINIT` (0x007d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzNotifyActorInitPacket;

impl CzNotifyActorInitPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::NotifyActorInit
    }
}

/// Decoded representation of `CZ_BLOCKING_PLAY_CANCEL` (0x0447): no fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzBlockingPlayCancelPacket;

impl CzBlockingPlayCancelPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::BlockingPlayCancel
    }
}

/// Decoded representation of `CZ_REQUEST_TIME` (0x0360).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestTimePacket {
    pub client_tick: u32,
}

impl CzRequestTimePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let client_tick = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { client_tick })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestTime {
            client_tick: self.client_tick,
        }
    }
}

/// Decoded representation of `CZ_REQUEST_MOVE` (0x035f).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestMovePacket {
    pub dest_x: u16,
    pub dest_y: u16,
}

impl CzRequestMovePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 3 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 3,
                actual: raw.payload.len(),
            });
        }

        let buf = [raw.payload[0], raw.payload[1], raw.payload[2]];
        let (dest_x, dest_y, _) = unpack_pos_dir(&buf);
        Ok(Self { dest_x, dest_y })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestMove {
            dest_x: self.dest_x,
            dest_y: self.dest_y,
        }
    }
}

/// Head direction: 0 straight, 1 clockwise, 2 counter-clockwise.
const MAX_HEAD_DIR: u16 = 2;
/// Body direction: 0 (N) through 7 (NE).
const MAX_BODY_DIR: u8 = 7;

/// Decoded representation of `CZ_CHANGE_DIRECTION` (0x0361). Out-of-range directions are
/// `InvalidField`, which closes the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzChangeDirectionPacket {
    pub head_dir: u16,
    pub body_dir: u8,
}

impl CzChangeDirectionPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 3 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 3,
                actual: raw.payload.len(),
            });
        }

        // rAthena reads head_dir as one byte at frame offset 2 and never reads offset 3.
        let head_dir = u16::from(raw.payload[0]);
        let body_dir = raw.payload[2];
        if head_dir > MAX_HEAD_DIR {
            return Err(PacketCodecError::InvalidField {
                opcode: raw.opcode,
                field: "head_dir",
                value: head_dir.into(),
            });
        }
        if body_dir > MAX_BODY_DIR {
            return Err(PacketCodecError::InvalidField {
                opcode: raw.opcode,
                field: "body_dir",
                value: body_dir.into(),
            });
        }
        Ok(Self { head_dir, body_dir })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::ChangeDirection {
            head_dir: self.head_dir,
            body_dir: self.body_dir,
        }
    }
}

/// Decoded representation of `CZ_REQUEST_ACT` (0x0437).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestActPacket {
    pub target_id: u32,
    pub action_type: u8,
}

impl CzRequestActPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 5 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 5,
                actual: raw.payload.len(),
            });
        }

        let target_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
        let action_type = raw.payload[4];
        Ok(Self {
            target_id,
            action_type,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestAction {
            target_id: self.target_id,
            action_type: self.action_type,
        }
    }
}

/// Decoded representation of `CZ_REQNAME` (0x0368).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzReqNamePacket {
    pub entity_id: u32,
}

impl CzReqNamePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let entity_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { entity_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestEntityName {
            entity_id: self.entity_id,
        }
    }
}

/// Decoded representation of `CZ_RESTART` (0x00b2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRestartPacket {
    pub restart_type: u8,
}

impl CzRestartPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.is_empty() {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 1,
                actual: raw.payload.len(),
            });
        }

        Ok(Self {
            restart_type: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::Restart {
            restart_type: self.restart_type,
        }
    }
}

/// Decoded representation of `CZ_REQ_DISCONNECT` (0x018a), sent by "Exit to Windows".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzReqDisconnectPacket {
    pub disconnect_type: u16,
}

impl CzReqDisconnectPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self {
            disconnect_type: read_u16_le(&raw.payload, 0, raw.opcode)?,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestDisconnect {
            disconnect_type: self.disconnect_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::pack_pos_dir;
    use crate::PacketOpcode;
    use bytes::Bytes;

    #[test]
    fn pkt_quit_01_decode_cz_req_disconnect() {
        let raw = RawPacket::new(PacketOpcode(0x018a), Bytes::from_static(&[0x00, 0x00]));
        let decoded = CzReqDisconnectPacket::decode(&raw).expect("2-byte payload decodes");
        assert_eq!(
            decoded.to_intent(),
            ClientIntent::RequestDisconnect { disconnect_type: 0 }
        );

        let short = RawPacket::new(PacketOpcode(0x018a), Bytes::from_static(&[0x00]));
        assert!(matches!(
            CzReqDisconnectPacket::decode(&short),
            Err(PacketCodecError::TruncatedPayload {
                expected: 2,
                actual: 1,
                ..
            })
        ));
    }

    #[test]
    fn test_decode_cz_enter() {
        let mut payload = vec![0u8; 21];
        payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        payload[4..8].copy_from_slice(&150001u32.to_le_bytes());
        payload[8..12].copy_from_slice(&11111u32.to_le_bytes());
        payload[12..16].copy_from_slice(&54321u32.to_le_bytes());
        payload[20] = 0; // Female

        let raw = RawPacket::new(PacketOpcode(0x0436), Bytes::from(payload));
        let decoded = CzEnterPacket::decode(&raw).unwrap();

        assert_eq!(decoded.account_id, AccountId(2000001));
        assert_eq!(decoded.char_id, CharId(150001));
        assert_eq!(decoded.login_id1, 11111);
        assert_eq!(decoded.client_tick, 54321);
        assert_eq!(decoded.sex, Sex::Female);

        let intent = decoded.to_intent();
        assert_eq!(
            intent,
            ClientIntent::EnterMap {
                account_id: AccountId(2000001),
                char_id: CharId(150001),
                login_id1: 11111,
                client_tick: 54321,
                sex: Sex::Female,
            }
        );
    }

    #[test]
    fn test_decode_cz_request_time() {
        let payload = 98765u32.to_le_bytes().to_vec();
        let raw = RawPacket::new(PacketOpcode(0x0360), Bytes::from(payload));
        let decoded = CzRequestTimePacket::decode(&raw).unwrap();

        assert_eq!(decoded.client_tick, 98765);
    }

    #[test]
    fn test_decode_cz_request_move() {
        let pos = pack_pos_dir(120, 240, 4);
        let raw = RawPacket::new(PacketOpcode(0x035f), Bytes::copy_from_slice(&pos));
        let decoded = CzRequestMovePacket::decode(&raw).unwrap();

        assert_eq!(decoded.dest_x, 120);
        assert_eq!(decoded.dest_y, 240);
    }

    #[test]
    fn test_truncated_map_payload_errors() {
        let short_payload = Bytes::from_static(&[0u8; 2]);

        let raw_cz_enter = RawPacket::new(PacketOpcode(0x0436), short_payload.clone());
        match CzEnterPacket::decode(&raw_cz_enter) {
            Err(PacketCodecError::TruncatedPayload {
                opcode,
                expected,
                actual,
            }) => {
                assert_eq!(opcode, PacketOpcode(0x0436));
                assert_eq!(expected, 17);
                assert_eq!(actual, 2);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }

        let raw_time = RawPacket::new(PacketOpcode(0x0360), short_payload);
        match CzRequestTimePacket::decode(&raw_time) {
            Err(PacketCodecError::TruncatedPayload {
                opcode,
                expected,
                actual,
            }) => {
                assert_eq!(opcode, PacketOpcode(0x0360));
                assert_eq!(expected, 4);
                assert_eq!(actual, 2);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }
    }
}

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

/// Decoded representation of `CZ_ITEM_THROW` (0x0363): wire index at offset 2, amount at 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzItemThrowPacket {
    pub index: u16,
    pub amount: u16,
}

impl CzItemThrowPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let index = read_u16_le(&raw.payload, 0, raw.opcode)?;
        let amount = read_u16_le(&raw.payload, 2, raw.opcode)?;
        Ok(Self { index, amount })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::DropItem {
            index: self.index,
            amount: self.amount,
        }
    }
}

/// Decoded representation of `CZ_REQUEST_CHAT` (0x00f3): `"<name> : <message>"` with no NUL on
/// the wire. A client that sends one anyway has its text end there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestChatPacket {
    pub text: Vec<u8>,
}

impl CzRequestChatPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let bytes = raw.payload.as_ref();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(Self {
            text: bytes[..end].to_vec(),
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::Chat {
            text: self.text.clone(),
        }
    }
}

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
