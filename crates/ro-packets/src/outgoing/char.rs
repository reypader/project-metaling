//! Outgoing character server packet encoders supporting HC_* opcodes.

use std::time::{SystemTime, UNIX_EPOCH};

use bytes::{BufMut, Bytes, BytesMut};
use ro_core::ids::{CharId, CharacterSummary};
use ro_core::packet::{PacketCodecError, PacketOpcode};

use crate::outgoing::helpers::write_fixed_string;

/// Serializes a single `CharacterSummary` into the 175-byte `CHARACTER_INFO` binary struct
/// according to rAthena `PACKETVER_RE_NUM = 20211103`.
pub fn encode_character_info(buf: &mut BytesMut, c: &CharacterSummary) {
    buf.put_u32_le(c.char_id.0); // GID
    buf.put_i64_le(0); // exp (int64 for >= 20170830)
    buf.put_i32_le(0); // money
    buf.put_i64_le(0); // jobexp (int64 for >= 20170830)
    buf.put_i32_le(c.job_level as i32); // joblevel
    buf.put_i32_le(0); // bodystate
    buf.put_i32_le(0); // healthstate
    buf.put_i32_le(0); // effectstate
    buf.put_i32_le(0); // virtue
    buf.put_i32_le(0); // honor
    buf.put_i16_le(0); // jobpoint
    buf.put_i64_le(100); // hp (int64 for >= 20211103)
    buf.put_i64_le(100); // maxhp (int64 for >= 20211103)
    buf.put_i64_le(50); // sp (int64 for >= 20211103)
    buf.put_i64_le(50); // maxsp (int64 for >= 20211103)
    buf.put_i16_le(150); // speed
    buf.put_i16_le(c.job_class as i16); // job
    buf.put_i16_le(c.hair_style as i16); // head
    buf.put_i16_le(0); // body (since >= 20141022)
    buf.put_i16_le(0); // weapon
    buf.put_i16_le(c.base_level as i16); // level
    buf.put_i16_le(0); // sppoint
    buf.put_i16_le(0); // accessory
    buf.put_i16_le(0); // shield
    buf.put_i16_le(0); // accessory2
    buf.put_i16_le(0); // accessory3
    buf.put_i16_le(c.hair_color as i16); // headpalette
    buf.put_i16_le(0); // bodypalette
    buf.put_slice(&write_fixed_string::<24>(&c.name)); // name
    buf.put_u8(1); // Str
    buf.put_u8(1); // Agi
    buf.put_u8(1); // Vit
    buf.put_u8(1); // Int
    buf.put_u8(1); // Dex
    buf.put_u8(1); // Luk
    buf.put_u8(c.slot); // CharNum
    buf.put_u8(c.hair_color as u8); // hairColor
    buf.put_i16_le(0); // bIsChangedCharName
    buf.put_slice(&write_fixed_string::<16>(&c.last_map)); // mapName
    let del_rev_date = match c.delete_date {
        Some(ts) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if ts > now {
                (ts - now).min(i32::MAX as u64) as i32
            } else {
                -1
            }
        }
        None => 0,
    };
    buf.put_i32_le(del_rev_date); // DelRevDate
    buf.put_i32_le(0); // robePalette
    buf.put_i32_le(0); // chr_slot_changeCnt
    buf.put_i32_le(0); // chr_name_changeCnt
    buf.put_u8(c.sex as u8); // sex
}

/// Encodes `HC_ACCEPT_ENTER` (0x006b).
///
/// Dynamic packet structure:
/// - 0..2: opcode (0x006b)
/// - 2..4: packet length (27 + 175 * count)
/// - 4..5: total slots
/// - 5..6: premium start slot
/// - 6..7: premium end slot
/// - 7..27: extension (20 bytes)
/// - 27..: characters
pub fn encode_hc_accept_enter(
    total_slots: u8,
    premium_start: u8,
    premium_end: u8,
    characters: &[CharacterSummary],
) -> Result<Bytes, PacketCodecError> {
    let total_len = characters
        .len()
        .checked_mul(175)
        .and_then(|len| len.checked_add(27));
    let len_u16 = match total_len {
        Some(len) => u16::try_from(len).map_err(|_| PacketCodecError::PacketTooLarge {
            opcode: PacketOpcode::HC_ACCEPT_ENTER,
            length: len,
            max: u16::MAX as usize,
        })?,
        None => {
            return Err(PacketCodecError::PacketTooLarge {
                opcode: PacketOpcode::HC_ACCEPT_ENTER,
                length: usize::MAX,
                max: u16::MAX as usize,
            });
        }
    };
    let mut buf = BytesMut::with_capacity(len_u16 as usize);

    buf.put_u16_le(PacketOpcode::HC_ACCEPT_ENTER.raw());
    buf.put_u16_le(len_u16);
    buf.put_u8(total_slots);
    buf.put_u8(premium_start);
    buf.put_u8(premium_end);
    buf.put_bytes(0, 20);

    for c in characters {
        encode_character_info(&mut buf, c);
    }

    Ok(buf.freeze())
}

/// Encodes `HC_REFUSE_ENTER` (0x006c), fixed 3 bytes.
pub fn encode_hc_refuse_enter(error_code: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::HC_REFUSE_ENTER.raw());
    buf.put_u8(error_code);
    buf.freeze()
}

/// Encodes `HC_ACCEPT_ENTER2` (0x082d).
///
/// Fixed packet structure (29 bytes).
pub fn encode_hc_accept_enter2(
    normal_slots: u8,
    premium_slots: u8,
    billing_slots: u8,
    producible_slots: u8,
    total_slots: u8,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(29);
    buf.put_u16_le(PacketOpcode::HC_ACCEPT_ENTER2.raw());
    buf.put_u16_le(29);
    buf.put_u8(normal_slots);
    buf.put_u8(premium_slots);
    buf.put_u8(billing_slots);
    buf.put_u8(producible_slots);
    buf.put_u8(total_slots);
    buf.put_bytes(0, 20);
    buf.freeze()
}

/// Encodes `HC_ACK_CHARINFO_PER_PAGE` (0x0b72).
///
/// Dynamic packet structure: 4 + 175 * count.
/// When count is 3, appends a second 4-byte empty frame per rAthena termination rules.
pub fn encode_hc_ack_charinfo_per_page(
    characters: &[CharacterSummary],
) -> Result<Bytes, PacketCodecError> {
    let base_len = characters
        .len()
        .checked_mul(175)
        .and_then(|len| len.checked_add(4));
    let len_u16 = match base_len {
        Some(len) => u16::try_from(len).map_err(|_| PacketCodecError::PacketTooLarge {
            opcode: PacketOpcode::HC_ACK_CHARINFO_PER_PAGE,
            length: len,
            max: u16::MAX as usize,
        })?,
        None => {
            return Err(PacketCodecError::PacketTooLarge {
                opcode: PacketOpcode::HC_ACK_CHARINFO_PER_PAGE,
                length: usize::MAX,
                max: u16::MAX as usize,
            });
        }
    };
    let extra_term = if characters.len() == 3 { 4 } else { 0 };
    let mut buf = BytesMut::with_capacity((len_u16 as usize) + extra_term);
    buf.put_u16_le(PacketOpcode::HC_ACK_CHARINFO_PER_PAGE.raw());
    buf.put_u16_le(len_u16);

    for c in characters {
        encode_character_info(&mut buf, c);
    }

    if characters.len() == 3 {
        buf.put_u16_le(PacketOpcode::HC_ACK_CHARINFO_PER_PAGE.raw());
        buf.put_u16_le(4);
    }

    Ok(buf.freeze())
}

/// Encodes `HC_ACCEPT_MAKECHAR` (0x0b6f), fixed 177 bytes.
pub fn encode_hc_accept_makechar(character: &CharacterSummary) -> Bytes {
    let mut buf = BytesMut::with_capacity(177);
    buf.put_u16_le(PacketOpcode::HC_ACCEPT_MAKECHAR.raw());
    encode_character_info(&mut buf, character);
    buf.freeze()
}

/// Encodes `HC_REFUSE_MAKECHAR` (0x006e), fixed 3 bytes.
pub fn encode_hc_refuse_makechar(error_code: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::HC_REFUSE_MAKECHAR.raw());
    buf.put_u8(error_code);
    buf.freeze()
}

/// Encodes `HC_DELETE_CHAR3_RESERVED` (0x0828), fixed 14 bytes.
pub fn encode_hc_delete_char3_reserved(
    char_id: CharId,
    result: u32,
    delete_delay_seconds: u32,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(14);
    buf.put_u16_le(PacketOpcode::HC_DELETE_CHAR3_RESERVED.raw());
    buf.put_u32_le(char_id.0);
    buf.put_u32_le(result);
    buf.put_u32_le(delete_delay_seconds);
    buf.freeze()
}

/// Encodes `HC_DELETE_CHAR3_CANCEL` (0x082c), fixed 10 bytes.
pub fn encode_hc_delete_char3_cancel(char_id: CharId, result: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(10);
    buf.put_u16_le(PacketOpcode::HC_DELETE_CHAR3_CANCEL.raw());
    buf.put_u32_le(char_id.0);
    buf.put_u32_le(result);
    buf.freeze()
}

/// Encodes `HC_NOTIFY_ZONESVR` (0x0ac5 for modern PACKETVER >= 20170315, 156 bytes).
///
/// Packet structure:
/// - 0..2: opcode (0x0ac5)
/// - 2..6: char_id (u32 LE)
/// - 6..22: map name (16 bytes)
/// - 22..26: ip (4 bytes)
/// - 26..28: port (u16 LE)
/// - 28..156: unknown padding (128 bytes)
pub fn encode_hc_notify_zonesvr(char_id: CharId, map_name: &str, ip: [u8; 4], port: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(156);
    buf.put_u16_le(PacketOpcode::HC_NOTIFY_ZONESVR.raw());
    buf.put_u32_le(char_id.0);
    buf.put_slice(&write_fixed_string::<16>(map_name));
    buf.put_slice(&ip);
    buf.put_u16_le(port);
    buf.put_bytes(0, 128);
    buf.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outgoing::encoder::ModernEventEncoder;
    use ro_core::ids::{AccountId, MapPosition, Sex};
    use ro_core::packet::{EventEncoder, ServerEvent};

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
}
