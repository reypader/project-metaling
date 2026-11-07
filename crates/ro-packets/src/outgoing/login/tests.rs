use super::*;
use crate::EventEncoder;
use ro_core::{ServerEvent, TARGET_PACKETVER};

use crate::outgoing::encoder::ModernEventEncoder;

#[test]
fn test_encode_ac_accept_login() {
    let servers = [CharServerEntry {
        ip: [127, 0, 0, 1],
        port: 6121,
        name: "Loki".to_string(),
        users: 42,
        server_type: 0,
        new_server: 1,
    }];

    let bytes = encode_ac_accept_login(
        1111,
        AccountId(2000001),
        2222,
        [10, 0, 0, 1],
        "2026-10-16 00:00:00",
        Sex::Male,
        &servers,
    )
    .unwrap();

    assert_eq!(bytes.len(), 64 + 160);
    assert_eq!(&bytes[0..2], &0x0ac4u16.to_le_bytes());
    assert_eq!(&bytes[2..4], &(224u16).to_le_bytes());
    assert_eq!(&bytes[4..8], &1111u32.to_le_bytes());
    assert_eq!(&bytes[8..12], &2000001u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &2222u32.to_le_bytes());
    assert_eq!(&bytes[16..20], &[10, 0, 0, 1]);
    assert_eq!(bytes[46], 1);
    assert_eq!(&bytes[64..68], &[127, 0, 0, 1]);
    assert_eq!(&bytes[68..70], &6121u16.to_le_bytes());
}

#[test]
fn test_encode_ac_refuse_login() {
    let bytes = encode_ac_refuse_login(1, "0");
    assert_eq!(bytes.len(), 26);
    assert_eq!(&bytes[0..2], &0x083eu16.to_le_bytes());
    assert_eq!(&bytes[2..6], &1u32.to_le_bytes());
}

#[test]
fn test_encode_login_refused_captured_trace() {
    let encoder = ModernEventEncoder;
    let event = ServerEvent::LoginRefused { error_code: 1 };
    let bytes = encoder.encode_event(&event, TARGET_PACKETVER).unwrap();

    let expected: [u8; 26] = [
        0x3e, 0x08, 0x01, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    assert_eq!(bytes.len(), 26);
    assert_eq!(bytes.as_ref(), &expected);
}

#[test]
fn test_encode_login_accepted_captured_trace() {
    let encoder = ModernEventEncoder;
    let event = ServerEvent::LoginAccepted {
        account_id: AccountId(2000000),
        login_id1: 2422544335,
        login_id2: 800520755,
        sex: Sex::Male,
        char_servers: vec![CharServerInfo {
            ip: [127, 0, 0, 1],
            port: 6121,
            name: "Loki".to_string(),
            users: 0,
            server_type: 0,
            new_server: 0,
        }],
    };

    let bytes = encoder.encode_event(&event, TARGET_PACKETVER).unwrap();

    assert_eq!(bytes.len(), 224);
    assert_eq!(&bytes[0..2], &[0xc4, 0x0a]);
    assert_eq!(&bytes[2..4], &224u16.to_le_bytes());
    assert_eq!(&bytes[4..8], &2422544335u32.to_le_bytes());
    assert_eq!(&bytes[8..12], &2000000u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &800520755u32.to_le_bytes());
    assert_eq!(&bytes[16..20], &[0, 0, 0, 0]);
    assert_eq!(&bytes[20..46], &[0u8; 26]);
    assert_eq!(bytes[46], 1);
    assert_eq!(&bytes[47..64], &[0u8; 17]);

    assert_eq!(&bytes[64..68], &[127, 0, 0, 1]);
    assert_eq!(&bytes[68..70], &6121u16.to_le_bytes());
    assert_eq!(&bytes[70..74], b"Loki");
    assert_eq!(&bytes[74..90], &[0u8; 16]);
    assert_eq!(&bytes[90..92], &0u16.to_le_bytes());
    assert_eq!(&bytes[92..94], &0u16.to_le_bytes());
    assert_eq!(&bytes[94..96], &0u16.to_le_bytes());
    assert_eq!(&bytes[96..224], &[0u8; 128]);
}

#[test]
fn test_encode_packet_too_large_login() {
    let dummy_servers = vec![
        CharServerEntry {
            ip: [127, 0, 0, 1],
            port: 6121,
            name: "Server".to_string(),
            users: 0,
            server_type: 0,
            new_server: 0,
        };
        410
    ];
    let res_login = encode_ac_accept_login(
        1,
        AccountId(2000000),
        2,
        [0, 0, 0, 0],
        "",
        Sex::Male,
        &dummy_servers,
    );
    match res_login {
        Err(PacketCodecError::PacketTooLarge {
            opcode,
            length,
            max,
        }) => {
            assert_eq!(opcode, PacketOpcode::AC_ACCEPT_LOGIN);
            assert_eq!(length, 64 + 410 * 160);
            assert_eq!(max, u16::MAX as usize);
        }
        other => panic!("Expected PacketTooLarge error, got {:?}", other),
    }
}
