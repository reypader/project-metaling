//! Map-entry settings packet encoders: friends list, skill list, hotkeys, config flags, reputation,
//! map property, mail icon, achievements, overweight percent and the load confirm. Layouts:
//! `reference/catalog/packets/settings.md` and `packets/map.md` (`0x099b`).

use bytes::{BufMut, Bytes, BytesMut};

use ro_core::{AchievementSummary, SkillInfo};

use super::helpers::write_fixed_string;
use crate::PacketOpcode;

/// Hotkey slots per `0x0b20` bank (`MAX_HOTKEYS`).
const HOTKEYS_PER_TAB: usize = 38;
/// One hotkey slot: `isSkill:u8, id:u32, count:u16`.
const HOTKEY_SIZE: usize = 7;
/// `0x0b20` frame size: opcode, rotate, tab, 38 slots.
const SHORTCUT_KEY_LIST_SIZE: usize = 2 + 1 + 2 + HOTKEYS_PER_TAB * HOTKEY_SIZE;
/// One `0x010f` entry: `id:u16, inf:i32, level:u16, sp:u16, range:u16, name:char[24], upFlag:u8`.
const SKILLINFO_ENTRY_SIZE: usize = 37;
/// Entries that fit rAthena's 8192-byte `0x010f` cap.
const MAX_SKILLINFO_ENTRIES: usize = (8192 - 4) / SKILLINFO_ENTRY_SIZE;
/// Reputation types a fresh character is sent (all at 0 points).
const REPUTATION_TYPES: [u64; 4] = [1, 2, 3, 4];
/// `0x0b8d` entry: `type:u64, points:i64`.
const REPUTATION_ENTRY_SIZE: usize = 16;

/// Encodes `ZC_FRIENDS_LIST` (0x0201) with no friends: a 4-byte dynamic frame.
pub fn encode_zc_friends_list_empty() -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_FRIENDS_LIST.raw());
    buf.put_u16_le(4);
    buf.freeze()
}

/// Encodes `ZC_SKILLINFO_LIST` (0x010f): `len:u16`, then 37 bytes per skill. rAthena caps the
/// frame at 8192 bytes; skills past that are dropped here.
pub fn encode_zc_skillinfo_list(skills: &[SkillInfo]) -> Bytes {
    let count = skills.len().min(MAX_SKILLINFO_ENTRIES);
    let len = 4 + count * SKILLINFO_ENTRY_SIZE;
    let mut buf = BytesMut::with_capacity(len);
    buf.put_u16_le(PacketOpcode::ZC_SKILLINFO_LIST.raw());
    buf.put_u16_le(u16::try_from(len).unwrap_or(u16::MAX));
    for skill in &skills[..count] {
        buf.put_u16_le(skill.id);
        buf.put_i32_le(skill.inf);
        buf.put_u16_le(skill.level);
        buf.put_u16_le(skill.sp);
        buf.put_u16_le(skill.range);
        buf.put_slice(&write_fixed_string::<24>(&skill.name));
        buf.put_u8(u8::from(skill.upgradable));
    }
    buf.freeze()
}

/// Encodes `ZC_SKILLINFO_UPDATE` (0x010e): `id:u16, level:u16, sp:u16, range:u16, upFlag:u8`,
/// 11 bytes.
pub fn encode_zc_skillinfo_update(
    id: u16,
    level: u16,
    sp: u16,
    range: u16,
    upgradable: bool,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(11);
    buf.put_u16_le(PacketOpcode::ZC_SKILLINFO_UPDATE.raw());
    buf.put_u16_le(id);
    buf.put_u16_le(level);
    buf.put_u16_le(sp);
    buf.put_u16_le(range);
    buf.put_u8(u8::from(upgradable));
    buf.freeze()
}

/// Encodes `ZC_SKILLINFO_DELETE` (0x0441): `id:u16`, 4 bytes.
pub fn encode_zc_skillinfo_delete(id: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_SKILLINFO_DELETE.raw());
    buf.put_u16_le(id);
    buf.freeze()
}

/// Encodes `ZC_SHORTCUT_KEY_LIST_V4` (0x0b20) for hotkey bank `tab` with every slot empty and no row
/// shift: 271 bytes.
pub fn encode_zc_shortcut_key_list_empty(tab: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(SHORTCUT_KEY_LIST_SIZE);
    buf.put_u16_le(PacketOpcode::ZC_SHORTCUT_KEY_LIST_V4.raw());
    buf.put_u8(0);
    buf.put_u16_le(u16::from(tab));
    buf.put_bytes(0, HOTKEYS_PER_TAB * HOTKEY_SIZE);
    buf.freeze()
}

/// Encodes `ZC_PARTY_CONFIG` (0x02c9): 1 when party invites are refused automatically.
pub fn encode_zc_party_config(deny_invites: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::ZC_PARTY_CONFIG.raw());
    buf.put_u8(u8::from(deny_invites));
    buf.freeze()
}

/// Encodes `ZC_CONFIG_NOTIFY` (0x02da): 1 when others may open the equipment window.
pub fn encode_zc_config_notify(show_equip: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::ZC_CONFIG_NOTIFY.raw());
    buf.put_u8(u8::from(show_equip));
    buf.freeze()
}

/// Encodes `ZC_CONFIG` (0x02d9): `config_type:u32, value:u32`, 10 bytes.
pub fn encode_zc_config(config_type: u32, value: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(10);
    buf.put_u16_le(PacketOpcode::ZC_CONFIG.raw());
    buf.put_u32_le(config_type);
    buf.put_u32_le(value);
    buf.freeze()
}

/// Encodes `ZC_REPUTE_INFO` (0x0b8d): success 1 and every reputation type at 0 points (69 bytes).
pub fn encode_zc_repute_info_default() -> Bytes {
    let len = 5 + REPUTATION_TYPES.len() * REPUTATION_ENTRY_SIZE;
    let mut buf = BytesMut::with_capacity(len);
    buf.put_u16_le(PacketOpcode::ZC_REPUTE_INFO.raw());
    buf.put_u16_le(u16::try_from(len).unwrap_or(u16::MAX));
    buf.put_u8(1);
    for repute_type in REPUTATION_TYPES {
        buf.put_u64_le(repute_type);
        buf.put_i64_le(0);
    }
    buf.freeze()
}

/// `0x0a24` frame size: opcode, the totals (14 bytes) and one achievement entry (50 bytes).
const ACH_UPDATE_SIZE: usize = 66;

/// Encodes `ZC_NOTIFY_MAPPROPERTY2` (0x099b): `property:u16, flags:u32`, 8 bytes.
pub fn encode_zc_notify_mapproperty2(property: u16, flags: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(8);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_MAPPROPERTY2.raw());
    buf.put_u16_le(property);
    buf.put_u32_le(flags);
    buf.freeze()
}

/// Encodes `ZC_NOTIFY_UNREADMAIL` (0x09e7): 1 when there is unread mail, 3 bytes.
pub fn encode_zc_notify_unreadmail(unread: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_UNREADMAIL.raw());
    buf.put_u8(u8::from(unread));
    buf.freeze()
}

/// Encodes `ZC_ACH_UPDATE` (0x0a24) with the totals and no achievement (its 50-byte entry all
/// zero), 66 bytes.
pub fn encode_zc_ach_update_totals(summary: &AchievementSummary) -> Bytes {
    let mut buf = BytesMut::with_capacity(ACH_UPDATE_SIZE);
    buf.put_u16_le(PacketOpcode::ZC_ACH_UPDATE.raw());
    buf.put_u32_le(summary.total_score);
    buf.put_u16_le(summary.level);
    buf.put_u32_le(summary.exp);
    buf.put_u32_le(summary.exp_to_next);
    buf.put_bytes(0, ACH_UPDATE_SIZE - buf.len());
    buf.freeze()
}

/// Encodes `ZC_OVERWEIGHT_PERCENT` (0x0ade): `percent:u32`, 6 bytes.
pub fn encode_zc_overweight_percent(percent: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(6);
    buf.put_u16_le(PacketOpcode::ZC_OVERWEIGHT_PERCENT.raw());
    buf.put_u32_le(percent);
    buf.freeze()
}

/// Encodes `ZC_LOAD_CONFIRM` (0x0b1b): opcode only. Lets the client unblock input.
pub fn encode_zc_load_confirm() -> Bytes {
    Bytes::copy_from_slice(&PacketOpcode::ZC_LOAD_CONFIRM.raw().to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_key_list_is_271_bytes_with_tab() {
        let tab1 = encode_zc_shortcut_key_list_empty(1);
        assert_eq!(tab1.len(), 271);
        assert_eq!(&tab1[..5], &[0x20, 0x0b, 0x00, 0x01, 0x00]);
        assert!(tab1[5..].iter().all(|b| *b == 0));
    }

    #[test]
    fn repute_info_is_69_bytes() {
        let bytes = encode_zc_repute_info_default();
        assert_eq!(bytes.len(), 69);
        assert_eq!(&bytes[..5], &[0x8d, 0x0b, 0x45, 0x00, 0x01]);
        assert_eq!(&bytes[5..13], &1u64.to_le_bytes());
        assert!(bytes[13..21].iter().all(|b| *b == 0));
    }
}
