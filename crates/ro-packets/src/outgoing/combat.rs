//! Combat packet encoders: the mob HP bar `0x0977`, the out-of-range reply `0x0139`, the
//! skill/attack refusal `0x0110`, the exp gain `0x0acc` and the ammo deletion `0x07fa`.
//! Layouts: `reference/catalog/packets/combat.md`, `mechanics/mobs.md` §2,
//! `mechanics/experience.md` and `packets/inventory.md`. The swing `0x08c8` is in `map.rs`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::PacketOpcode;

/// Encodes `ZC_HP_INFO` (0x0977): `gid:u32, hp:u32, max_hp:u32`, 14 bytes.
pub fn encode_zc_hp_info(gid: u32, hp: u32, max_hp: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(14);
    buf.put_u16_le(PacketOpcode::ZC_HP_INFO.raw());
    buf.put_u32_le(gid);
    buf.put_u32_le(hp);
    buf.put_u32_le(max_hp);
    buf.freeze()
}

/// Encodes `ZC_ATTACK_FAILURE_FOR_DISTANCE` (0x0139): `target_id:u32, target_x, target_y, x, y,
/// range` (all i16 on the wire), 16 bytes.
pub fn encode_zc_attack_failure_for_distance(
    target_id: u32,
    target: (u16, u16),
    own: (u16, u16),
    range: u16,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(16);
    buf.put_u16_le(PacketOpcode::ZC_ATTACK_FAILURE_FOR_DISTANCE.raw());
    buf.put_u32_le(target_id);
    buf.put_u16_le(target.0);
    buf.put_u16_le(target.1);
    buf.put_u16_le(own.0);
    buf.put_u16_le(own.1);
    buf.put_u16_le(range);
    buf.freeze()
}

/// Encodes `ZC_ACK_TOUSESKILL` (0x0110): `skill_id:u16, btype:i32, item_id:u32, flag:u8,
/// cause:u8`, 14 bytes. `btype`, `item_id` and `flag` (failed) are always 0 here.
pub fn encode_zc_ack_touseskill(skill_id: u16, cause: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(14);
    buf.put_u16_le(PacketOpcode::ZC_ACK_TOUSESKILL.raw());
    buf.put_u16_le(skill_id);
    buf.put_i32_le(0);
    buf.put_u32_le(0);
    buf.put_u8(0);
    buf.put_u8(cause);
    buf.freeze()
}

/// Encodes `ZC_NOTIFY_EXP2` (0x0acc): `aid:u32, amount:i64, var_id:u16, exp_type:i16` (0 normal),
/// 18 bytes.
pub fn encode_zc_notify_exp2(gid: u32, var_id: u16, amount: i64) -> Bytes {
    let mut buf = BytesMut::with_capacity(18);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_EXP2.raw());
    buf.put_u32_le(gid);
    buf.put_i64_le(amount);
    buf.put_u16_le(var_id);
    buf.put_i16_le(0);
    buf.freeze()
}

/// Encodes `ZC_DELETE_ITEM_FROM_BODY` (0x07fa): `delete_type:i16, index:u16, count:i16`, 8 bytes.
pub fn encode_zc_delete_item_from_body(reason: u16, index: u16, amount: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(8);
    buf.put_u16_le(PacketOpcode::ZC_DELETE_ITEM_FROM_BODY.raw());
    buf.put_u16_le(reason);
    buf.put_u16_le(index);
    buf.put_u16_le(amount);
    buf.freeze()
}
