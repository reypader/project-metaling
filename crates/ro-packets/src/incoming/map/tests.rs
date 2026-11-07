use ro_core::{AccountId, CharId, ClientIntent, Sex};

use super::*;
use crate::coords::pack_pos_dir;
use crate::PacketOpcode;
use crate::{PacketCodecError, RawPacket};
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
