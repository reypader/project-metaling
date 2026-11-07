//! Outgoing character server packet encoders supporting HC_* opcodes.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{PacketCodecError, PacketOpcode};
use bytes::{BufMut, Bytes, BytesMut};
use ro_core::{CharId, CharacterSummary};

use crate::outgoing::helpers::write_fixed_string;

/// The `CHARACTER_INFO` cap of points, SP and MaxSP (catalog `packets/char.md` "Status fields in
/// `CHARACTER_INFO`").
const SHORT_CAP: u32 = 32_767;
/// The cap of each STR..LUK byte.
const STAT_CAP: u16 = 255;

fn capped_short(value: u32) -> i16 {
    i16::try_from(value.min(SHORT_CAP)).unwrap_or(i16::MAX)
}

fn exp(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Serializes a single `CharacterSummary` into the 175-byte `CHARACTER_INFO` binary struct
/// according to rAthena `PACKETVER_RE_NUM = 20211103`. The status values are the last saved
/// ones; there are no trait or AP fields.
pub fn encode_character_info(buf: &mut BytesMut, c: &CharacterSummary) {
    let st = &c.select_stats;
    buf.put_u32_le(c.char_id.0); // GID
    buf.put_i64_le(exp(st.base_exp)); // exp (int64 for >= 20170830)
    buf.put_i32_le(i32::try_from(st.zeny).unwrap_or(i32::MAX)); // money
    buf.put_i64_le(exp(st.job_exp)); // jobexp (int64 for >= 20170830)
    buf.put_i32_le(c.job_level as i32); // joblevel
    buf.put_i32_le(0); // bodystate
    buf.put_i32_le(0); // healthstate
    buf.put_i32_le(0); // effectstate
    buf.put_i32_le(0); // virtue
    buf.put_i32_le(0); // honor
    buf.put_i16_le(capped_short(st.status_point.into())); // jobpoint: status points
    buf.put_i64_le(st.hp.into()); // hp (int64 for >= 20211103, uncapped)
    buf.put_i64_le(st.max_hp.into()); // maxhp
    buf.put_i64_le(capped_short(st.sp).into()); // sp (int64, but capped at 32767)
    buf.put_i64_le(capped_short(st.max_sp).into()); // maxsp, likewise
    buf.put_i16_le(150); // speed
    buf.put_i16_le(c.job_class as i16); // job
    buf.put_i16_le(c.hair_style as i16); // head
    buf.put_i16_le(c.job_class as i16); // body: the client draws the body sprite from it (= job)
    buf.put_i16_le(c.looks.weapon_type as i16); // weapon: the W_* type, not an item id
    buf.put_i16_le(c.base_level as i16); // level
    buf.put_i16_le(capped_short(st.skill_point.into())); // sppoint: skill points
    buf.put_u16_le(c.looks.head_bottom); // accessory
    buf.put_i16_le(c.looks.shield as i16); // shield: the left armor's View
    buf.put_u16_le(c.looks.head_top); // accessory2
    buf.put_u16_le(c.looks.head_mid); // accessory3
    buf.put_i16_le(c.hair_color as i16); // headpalette
    buf.put_i16_le(0); // bodypalette
    buf.put_slice(&write_fixed_string::<24>(&c.name)); // name
    for stat in st.stats {
        buf.put_u8(u8::try_from(stat.min(STAT_CAP)).unwrap_or(u8::MAX)); // Str..Luk, allocated
    }
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
    buf.put_i32_le(i32::from(c.looks.robe)); // robePalette: the robe view
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
mod tests;
