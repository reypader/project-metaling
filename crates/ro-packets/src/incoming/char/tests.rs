use ro_core::{AccountId, Sex};

use super::*;
use crate::PacketOpcode;
use crate::{PacketCodecError, RawPacket};
use bytes::Bytes;

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
