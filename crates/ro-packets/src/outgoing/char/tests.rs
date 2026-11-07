use super::*;
use crate::outgoing::encoder::ModernEventEncoder;
use crate::EventEncoder;
use ro_core::{AccountId, CharacterLooks, MapPosition, ServerEvent, Sex};

#[test]
fn test_encode_hc_notify_zonesvr() {
    let bytes = encode_hc_notify_zonesvr(CharId(150001), "prontera.gat", [127, 0, 0, 1], 5121);

    assert_eq!(bytes.len(), 156);
    assert_eq!(&bytes[0..2], &0x0ac5u16.to_le_bytes());
    assert_eq!(&bytes[2..6], &150001u32.to_le_bytes());
    assert_eq!(&bytes[22..26], &[127, 0, 0, 1]);
    assert_eq!(&bytes[26..28], &5121u16.to_le_bytes());
}

#[test]
fn test_encode_character_info_with_delete_date() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let delete_ts = now + 86400;
    let mut c = CharacterSummary {
        char_id: CharId(150004),
        account_id: AccountId(2000001),
        slot: 3,
        name: "test5".to_string(),
        sex: Sex::Female,
        job_class: 0,
        base_level: 1,
        job_level: 1,
        hair_style: 22,
        hair_color: 0,
        last_map: "prontera.gat".to_string(),
        last_pos: MapPosition {
            x: 50,
            y: 50,
            dir: 0,
        },
        delete_date: Some(delete_ts),
        looks: CharacterLooks::default(),
        select_stats: Default::default(),
    };

    let mut buf = BytesMut::new();
    encode_character_info(&mut buf, &c);
    assert_eq!(buf.len(), 175);
    let remaining_secs = i32::from_le_bytes(buf[158..162].try_into().unwrap());
    assert!((86395..=86400).contains(&remaining_secs));

    c.delete_date = Some(now - 100);
    let mut buf_expired = BytesMut::new();
    encode_character_info(&mut buf_expired, &c);
    let expired_secs = i32::from_le_bytes(buf_expired[158..162].try_into().unwrap());
    assert_eq!(expired_secs, -1);

    c.delete_date = None;
    let mut buf_none = BytesMut::new();
    encode_character_info(&mut buf_none, &c);
    let none_secs = i32::from_le_bytes(buf_none[158..162].try_into().unwrap());
    assert_eq!(none_secs, 0);
}

#[test]
fn test_encode_hc_ack_charinfo_per_page_three_chars_termination() {
    let make_char = |slot: u8| CharacterSummary {
        char_id: CharId(150000 + slot as u32),
        account_id: AccountId(2000001),
        slot,
        name: format!("char{}", slot),
        sex: Sex::Male,
        job_class: 0,
        base_level: 1,
        job_level: 1,
        hair_style: 1,
        hair_color: 1,
        last_map: "prontera.gat".to_string(),
        last_pos: MapPosition {
            x: 10,
            y: 10,
            dir: 0,
        },
        delete_date: None,
        looks: CharacterLooks::default(),
        select_stats: Default::default(),
    };
    let chars = vec![make_char(0), make_char(1), make_char(2)];
    let bytes = encode_hc_ack_charinfo_per_page(&chars).unwrap();
    assert_eq!(bytes.len(), 533);
    assert_eq!(&bytes[0..2], &0x0b72u16.to_le_bytes());
    assert_eq!(&bytes[2..4], &529u16.to_le_bytes());
    assert_eq!(&bytes[529..533], &[0x72, 0x0b, 0x04, 0x00]);
}

#[test]
fn test_encode_character_list_refused() {
    let error_code = 3u8;
    let bytes = encode_hc_refuse_enter(error_code);
    assert_eq!(bytes.len(), 3);
    assert_eq!(&bytes[0..2], &0x006cu16.to_le_bytes());
    assert_eq!(bytes[2], 3);

    let encoder = ModernEventEncoder;
    let event = ServerEvent::CharacterListRefused { error_code };
    let encoded = encoder.encode_event(&event, 20260219).unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn test_encode_packet_too_large_char() {
    let dummy_char = CharacterSummary {
        char_id: CharId(150000),
        account_id: AccountId(2000000),
        slot: 0,
        name: "Hero".to_string(),
        sex: Sex::Male,
        job_class: 0,
        base_level: 1,
        job_level: 1,
        hair_style: 1,
        hair_color: 1,
        last_map: "prontera.gat".to_string(),
        last_pos: MapPosition {
            x: 50,
            y: 50,
            dir: 0,
        },
        delete_date: None,
        looks: CharacterLooks::default(),
        select_stats: Default::default(),
    };
    let dummy_chars = vec![dummy_char; 375];
    let res_char_list = encode_hc_accept_enter(9, 0, 0, &dummy_chars);
    match res_char_list {
        Err(PacketCodecError::PacketTooLarge {
            opcode,
            length,
            max,
        }) => {
            assert_eq!(opcode, PacketOpcode::HC_ACCEPT_ENTER);
            assert_eq!(length, 27 + 375 * 175);
            assert_eq!(max, u16::MAX as usize);
        }
        other => panic!("Expected PacketTooLarge error, got {:?}", other),
    }

    let res_page = encode_hc_ack_charinfo_per_page(&dummy_chars);
    match res_page {
        Err(PacketCodecError::PacketTooLarge {
            opcode,
            length,
            max,
        }) => {
            assert_eq!(opcode, PacketOpcode::HC_ACK_CHARINFO_PER_PAGE);
            assert_eq!(length, 4 + 375 * 175);
            assert_eq!(max, u16::MAX as usize);
        }
        other => panic!("Expected PacketTooLarge error, got {:?}", other),
    }
}
