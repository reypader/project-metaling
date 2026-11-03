//! Skill and item-use packet encoders: the cast notice `0x0b1a`, the damaging hit `0x01de`, the
//! non-damaging effect `0x09cb`, the item-use ack `0x01c8` and the long status var `0x00b1`.
//! Layouts: catalog `scripting/skills.md` §5; frames checked against `scripting_1.txt`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::PacketOpcode;

/// Encodes `ZC_USESKILL_ACK` (0x0b1a), 29 bytes: `src:u32, dst:u32, x:u16, y:u16, skill:u16,
/// element:u32, delay:u32, disposable:u8, attackMT:u32`. `disposable` and `attackMT` are 0.
pub fn encode_zc_useskill_ack(
    src: u32,
    dst: u32,
    pos: (u16, u16),
    skill_id: u16,
    element: u32,
    delay_ms: u32,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(29);
    buf.put_u16_le(PacketOpcode::ZC_USESKILL_ACK.raw());
    buf.put_u32_le(src);
    buf.put_u32_le(dst);
    buf.put_u16_le(pos.0);
    buf.put_u16_le(pos.1);
    buf.put_u16_le(skill_id);
    buf.put_u32_le(element);
    buf.put_u32_le(delay_ms);
    buf.put_u8(0);
    buf.put_u32_le(0);
    buf.freeze()
}

/// The fields of one `0x01de` frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillDamageParams {
    pub skill_id: u16,
    pub src: u32,
    pub dst: u32,
    pub tick: u32,
    pub src_speed: i32,
    pub dmg_speed: i32,
    pub damage: i32,
    pub level: i16,
    pub div: i16,
    pub damage_type: u8,
}

/// Encodes `ZC_NOTIFY_SKILL2` (0x01de), 33 bytes: `skill:u16, src:u32, dst:u32, tick:u32,
/// sdelay:i32, ddelay:i32, damage:i32, level:i16, div:i16, type:u8`.
pub fn encode_zc_notify_skill2(p: &SkillDamageParams) -> Bytes {
    let mut buf = BytesMut::with_capacity(33);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_SKILL_0X01DE.raw());
    buf.put_u16_le(p.skill_id);
    buf.put_u32_le(p.src);
    buf.put_u32_le(p.dst);
    buf.put_u32_le(p.tick);
    buf.put_i32_le(p.src_speed);
    buf.put_i32_le(p.dmg_speed);
    buf.put_i32_le(p.damage);
    buf.put_i16_le(p.level);
    buf.put_i16_le(p.div);
    buf.put_u8(p.damage_type);
    buf.freeze()
}

/// Encodes `ZC_USE_SKILL2` (0x09cb), 17 bytes: `skill:u16, value:i32, target:u32, src:u32,
/// result:u8`.
pub fn encode_zc_use_skill2(skill_id: u16, value: i32, target: u32, src: u32, ok: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(17);
    buf.put_u16_le(PacketOpcode::ZC_USE_SKILL2.raw());
    buf.put_u16_le(skill_id);
    buf.put_i32_le(value);
    buf.put_u32_le(target);
    buf.put_u32_le(src);
    buf.put_u8(u8::from(ok));
    buf.freeze()
}

/// Encodes `ZC_USE_ITEM_ACK2` (0x01c8), 15 bytes: `index:u16, item_id:u32, aid:u32, amount:i16,
/// result:u8`.
pub fn encode_zc_use_item_ack2(index: u16, item_id: u32, gid: u32, amount: i16, ok: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(15);
    buf.put_u16_le(PacketOpcode::ZC_USE_ITEM_ACK2.raw());
    buf.put_u16_le(index);
    buf.put_u32_le(item_id);
    buf.put_u32_le(gid);
    buf.put_i16_le(amount);
    buf.put_u8(u8::from(ok));
    buf.freeze()
}

/// Encodes `ZC_LONGPAR_CHANGE` (0x00b1), 8 bytes: `var:u16, value:i32`.
pub fn encode_zc_longpar_change(var_id: u16, value: i32) -> Bytes {
    let mut buf = BytesMut::with_capacity(8);
    buf.put_u16_le(PacketOpcode::ZC_LONGPAR_CHANGE.raw());
    buf.put_u16_le(var_id);
    buf.put_i32_le(value);
    buf.freeze()
}
