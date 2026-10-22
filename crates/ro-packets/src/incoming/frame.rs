//! High-level client frame decoding and intent parsing.

use ro_contracts::packet::{
    ClientIntent, PacketCodecError, PacketIntentParser, PacketLength, PacketOpcode, RawPacket,
};

use crate::table::lookup_packet_length;

use super::char::{
    ChCharListReqPacket, ChDeleteChar3CancelPacket, ChDeleteChar3ReservedPacket, ChEnterPacket,
    ChMakeCharPacket, ChSelectCharPacket, PingPacket,
};
use super::login::CaLoginPacket;
use super::map::{
    CzChangeDirectionPacket, CzEnterPacket, CzNotifyActorInitPacket, CzReqNamePacket,
    CzRequestActPacket, CzRequestMovePacket, CzRequestTimePacket, CzRestartPacket,
};

/// Enumeration of all recognized decoded client frames across login, char, and map servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedFrame {
    Intent(ClientIntent),
    Unhandled { opcode: PacketOpcode, len: usize },
}

/// Decodes any incoming `RawPacket` into a high-level `DecodedFrame`.
///
/// If the opcode is recognized by packetdb but not handled in Phase 0 domain logic,
/// returns `DecodedFrame::Unhandled { opcode, len }` to allow connection persistence.
pub fn decode_frame(raw: &RawPacket, packetver: u32) -> Result<DecodedFrame, PacketCodecError> {
    match raw.opcode.raw() {
        0x0064 | 0x0277 => {
            let p = CaLoginPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0065 => {
            let p = ChEnterPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0066 => {
            let p = ChSelectCharPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0a39 => {
            let p = ChMakeCharPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0827 => {
            let p = ChDeleteChar3ReservedPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x082b => {
            let p = ChDeleteChar3CancelPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x09a1 => {
            let p = ChCharListReqPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0187 => {
            let p = PingPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x00b2 => {
            let p = CzRestartPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0436 => {
            let p = CzEnterPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x007d => {
            let p = CzNotifyActorInitPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0360 => {
            let p = CzRequestTimePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x035f => {
            let p = CzRequestMovePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0361 => {
            let p = CzChangeDirectionPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0437 => {
            let p = CzRequestActPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0368 => {
            let p = CzReqNamePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        _ => {
            let total_len = match lookup_packet_length(raw.opcode, packetver) {
                Some(PacketLength::Fixed(len)) => len,
                Some(PacketLength::Dynamic) => raw.payload.len() + 4,
                None => raw.payload.len() + 2,
            };
            Ok(DecodedFrame::Unhandled {
                opcode: raw.opcode,
                len: total_len,
            })
        }
    }
}

/// Domain intent parser implementing contracts trait.
#[derive(Debug, Default, Clone, Copy)]
pub struct Phase0IntentParser;

impl PacketIntentParser for Phase0IntentParser {
    fn parse_intent(
        &self,
        raw: &RawPacket,
        packetver: u32,
    ) -> Result<Option<ClientIntent>, PacketCodecError> {
        match decode_frame(raw, packetver)? {
            DecodedFrame::Intent(intent) => Ok(Some(intent)),
            DecodedFrame::Unhandled { .. } => Ok(None),
        }
    }
}

/// Modern alias for the intent parser.
pub type ModernIntentParser = Phase0IntentParser;

/// Rathena-compatible alias for the intent parser.
pub type RathenaIntentParser = Phase0IntentParser;

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use ro_contracts::ids::{AccountId, CharId, Sex};
    use ro_contracts::TARGET_PACKETVER;

    use crate::coords::pack_pos_dir;

    #[test]
    fn test_phase0_intent_parser() {
        let parser = Phase0IntentParser;
        let ver = TARGET_PACKETVER;

        let raw_time = RawPacket::new(
            PacketOpcode(0x0360),
            Bytes::copy_from_slice(&55555u32.to_le_bytes()),
        );
        let intent = parser.parse_intent(&raw_time, ver).unwrap().unwrap();
        assert_eq!(intent, ClientIntent::RequestTime { client_tick: 55555 });

        let raw_unknown =
            RawPacket::new(PacketOpcode(0x009f), Bytes::copy_from_slice(&[1, 2, 3, 4]));
        assert_eq!(parser.parse_intent(&raw_unknown, ver).unwrap(), None);

        let mut login_payload = vec![0u8; 53];
        login_payload[0..4].copy_from_slice(&20211103u32.to_le_bytes());
        login_payload[4..12].copy_from_slice(b"alice123");
        login_payload[28..36].copy_from_slice(b"password");
        login_payload[52] = 1;
        let raw_login = RawPacket::new(PacketOpcode(0x0064), Bytes::from(login_payload));
        let login_intent = parser.parse_intent(&raw_login, ver).unwrap().unwrap();
        assert_eq!(
            login_intent,
            ClientIntent::Login {
                username: "alice123".to_string(),
                password: "password".to_string(),
                client_version: 20211103,
                client_type: 1,
            }
        );
        let frame_login = decode_frame(&raw_login, ver).unwrap();
        assert_eq!(frame_login, DecodedFrame::Intent(login_intent));

        let mut enter_payload = vec![0u8; 15];
        enter_payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        enter_payload[4..8].copy_from_slice(&11111u32.to_le_bytes());
        enter_payload[8..12].copy_from_slice(&22222u32.to_le_bytes());
        enter_payload[12..14].copy_from_slice(&20u16.to_le_bytes());
        enter_payload[14] = 1;
        let raw_enter = RawPacket::new(PacketOpcode(0x0065), Bytes::from(enter_payload));
        let enter_intent = parser.parse_intent(&raw_enter, ver).unwrap().unwrap();
        assert_eq!(
            enter_intent,
            ClientIntent::EnterCharServer {
                account_id: AccountId(2000001),
                login_id1: 11111,
                login_id2: 22222,
                client_version: 20,
                sex: Sex::Male,
            }
        );
        let frame_enter = decode_frame(&raw_enter, ver).unwrap();
        assert_eq!(frame_enter, DecodedFrame::Intent(enter_intent));

        let select_payload = vec![2u8];
        let raw_select = RawPacket::new(PacketOpcode(0x0066), Bytes::from(select_payload));
        let select_intent = parser.parse_intent(&raw_select, ver).unwrap().unwrap();
        assert_eq!(select_intent, ClientIntent::SelectCharacter { slot: 2 });
        let frame_select = decode_frame(&raw_select, ver).unwrap();
        assert_eq!(frame_select, DecodedFrame::Intent(select_intent));
    }

    #[test]
    fn test_modern_client_intent_decoding() {
        let parser = Phase0IntentParser;
        let ver = TARGET_PACKETVER;

        let pos = pack_pos_dir(250, 300, 2);
        let raw_move = RawPacket::new(PacketOpcode(0x035f), Bytes::copy_from_slice(&pos));
        let move_intent = parser.parse_intent(&raw_move, ver).unwrap().unwrap();
        assert_eq!(
            move_intent,
            ClientIntent::RequestMove {
                dest_x: 250,
                dest_y: 300,
            }
        );
        let frame_move = decode_frame(&raw_move, ver).unwrap();
        assert_eq!(frame_move, DecodedFrame::Intent(move_intent));

        let raw_time = RawPacket::new(
            PacketOpcode(0x0360),
            Bytes::copy_from_slice(&77777u32.to_le_bytes()),
        );
        let time_intent = parser.parse_intent(&raw_time, ver).unwrap().unwrap();
        assert_eq!(
            time_intent,
            ClientIntent::RequestTime { client_tick: 77777 }
        );
        let frame_time = decode_frame(&raw_time, ver).unwrap();
        assert_eq!(frame_time, DecodedFrame::Intent(time_intent));

        let mut dir_payload = vec![0u8; 3];
        dir_payload[0..2].copy_from_slice(&6u16.to_le_bytes());
        dir_payload[2] = 4;
        let raw_dir = RawPacket::new(PacketOpcode(0x0361), Bytes::from(dir_payload));
        let dir_intent = parser.parse_intent(&raw_dir, ver).unwrap().unwrap();
        assert_eq!(
            dir_intent,
            ClientIntent::ChangeDirection {
                head_dir: 6,
                body_dir: 4,
            }
        );
        let frame_dir = decode_frame(&raw_dir, ver).unwrap();
        assert_eq!(frame_dir, DecodedFrame::Intent(dir_intent));

        let mut act_payload = vec![0u8; 5];
        act_payload[0..4].copy_from_slice(&100050u32.to_le_bytes());
        act_payload[4] = 2;
        let raw_act = RawPacket::new(PacketOpcode(0x0437), Bytes::from(act_payload));
        let act_intent = parser.parse_intent(&raw_act, ver).unwrap().unwrap();
        assert_eq!(
            act_intent,
            ClientIntent::RequestAction {
                target_id: 100050,
                action_type: 2,
            }
        );
        let frame_act = decode_frame(&raw_act, ver).unwrap();
        assert_eq!(frame_act, DecodedFrame::Intent(act_intent));

        let raw_reqname = RawPacket::new(
            PacketOpcode(0x0368),
            Bytes::copy_from_slice(&1002u32.to_le_bytes()),
        );
        let reqname_intent = parser.parse_intent(&raw_reqname, ver).unwrap().unwrap();
        assert_eq!(
            reqname_intent,
            ClientIntent::RequestEntityName { entity_id: 1002 }
        );
        let frame_reqname = decode_frame(&raw_reqname, ver).unwrap();
        assert_eq!(frame_reqname, DecodedFrame::Intent(reqname_intent));

        let mut cz_enter_payload = vec![0u8; 21];
        cz_enter_payload[0..4].copy_from_slice(&2000002u32.to_le_bytes());
        cz_enter_payload[4..8].copy_from_slice(&150002u32.to_le_bytes());
        cz_enter_payload[8..12].copy_from_slice(&33333u32.to_le_bytes());
        cz_enter_payload[12..16].copy_from_slice(&65432u32.to_le_bytes());
        cz_enter_payload[20] = 1;
        let raw_enter = RawPacket::new(PacketOpcode(0x0436), Bytes::from(cz_enter_payload));
        let enter_intent = parser.parse_intent(&raw_enter, ver).unwrap().unwrap();
        assert_eq!(
            enter_intent,
            ClientIntent::EnterMap {
                account_id: AccountId(2000002),
                char_id: CharId(150002),
                login_id1: 33333,
                client_tick: 65432,
                sex: Sex::Male,
            }
        );
    }

    #[test]
    fn test_character_and_session_incoming_packets() {
        let parser = Phase0IntentParser;
        let ver = TARGET_PACKETVER;

        let make_char_bytes: [u8; 36] = [
            0x39, 0x0a, 0x74, 0x65, 0x73, 0x74, 0x35, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let raw_make = RawPacket::new(
            PacketOpcode(0x0a39),
            Bytes::copy_from_slice(&make_char_bytes[2..]),
        );
        let intent_make = parser.parse_intent(&raw_make, ver).unwrap().unwrap();
        assert_eq!(
            intent_make,
            ClientIntent::CreateCharacter {
                name: "test5".to_string(),
                slot: 4,
                hair_color: 0,
                hair_style: 22,
                starting_job: 0,
                sex: Sex::Female,
            }
        );
        let frame_make = decode_frame(&raw_make, ver).unwrap();
        assert_eq!(frame_make, DecodedFrame::Intent(intent_make));

        let del_res_bytes: [u8; 6] = [0x27, 0x08, 0xf4, 0x49, 0x02, 0x00];
        let raw_del_res = RawPacket::new(
            PacketOpcode(0x0827),
            Bytes::copy_from_slice(&del_res_bytes[2..]),
        );
        let intent_del_res = parser.parse_intent(&raw_del_res, ver).unwrap().unwrap();
        assert_eq!(
            intent_del_res,
            ClientIntent::ReserveDeleteCharacter {
                char_id: CharId(150004),
            }
        );
        let frame_del_res = decode_frame(&raw_del_res, ver).unwrap();
        assert_eq!(frame_del_res, DecodedFrame::Intent(intent_del_res));

        let del_cancel_bytes: [u8; 6] = [0x2b, 0x08, 0xf4, 0x49, 0x02, 0x00];
        let raw_del_cancel = RawPacket::new(
            PacketOpcode(0x082b),
            Bytes::copy_from_slice(&del_cancel_bytes[2..]),
        );
        let intent_del_cancel = parser.parse_intent(&raw_del_cancel, ver).unwrap().unwrap();
        assert_eq!(
            intent_del_cancel,
            ClientIntent::CancelDeleteCharacter {
                char_id: CharId(150004),
            }
        );
        let frame_del_cancel = decode_frame(&raw_del_cancel, ver).unwrap();
        assert_eq!(frame_del_cancel, DecodedFrame::Intent(intent_del_cancel));

        let raw_charlist = RawPacket::new(PacketOpcode(0x09a1), Bytes::new());
        let intent_charlist = parser.parse_intent(&raw_charlist, ver).unwrap().unwrap();
        assert_eq!(intent_charlist, ClientIntent::RequestCharacterList);
        let frame_charlist = decode_frame(&raw_charlist, ver).unwrap();
        assert_eq!(frame_charlist, DecodedFrame::Intent(intent_charlist));

        let ping_bytes: [u8; 6] = [0x87, 0x01, 0x01, 0x00, 0x1f, 0x00];
        let raw_ping = RawPacket::new(
            PacketOpcode(0x0187),
            Bytes::copy_from_slice(&ping_bytes[2..]),
        );
        let intent_ping = parser.parse_intent(&raw_ping, ver).unwrap().unwrap();
        assert_eq!(
            intent_ping,
            ClientIntent::CharPing {
                account_id: AccountId(2031617),
            }
        );
        let frame_ping = decode_frame(&raw_ping, ver).unwrap();
        assert_eq!(frame_ping, DecodedFrame::Intent(intent_ping));

        let restart_bytes: [u8; 3] = [0xb2, 0x00, 0x01];
        let raw_restart = RawPacket::new(
            PacketOpcode(0x00b2),
            Bytes::copy_from_slice(&restart_bytes[2..]),
        );
        let intent_restart = parser.parse_intent(&raw_restart, ver).unwrap().unwrap();
        assert_eq!(intent_restart, ClientIntent::Restart { restart_type: 1 });
        let frame_restart = decode_frame(&raw_restart, ver).unwrap();
        assert_eq!(frame_restart, DecodedFrame::Intent(intent_restart));
    }
}
