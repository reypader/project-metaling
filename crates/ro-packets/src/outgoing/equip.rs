//! Equipment packet encoders: `0x0999`, `0x099a`, `0x0bae`, `0x013c`, `0x013b`, `0x0291`.
//! Layouts: `reference/catalog/packets/equipment.md`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::PacketOpcode;

/// Encodes `ZC_REQ_WEAR_EQUIP_ACK` (0x0999): `index:u16, location:u32, view:u16, result:u8`,
/// 11 bytes.
pub fn encode_zc_req_wear_equip_ack(index: u16, location: u32, view: u16, result: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(11);
    buf.put_u16_le(PacketOpcode::ZC_REQ_WEAR_EQUIP_ACK.raw());
    buf.put_u16_le(index);
    buf.put_u32_le(location);
    buf.put_u16_le(view);
    buf.put_u8(result);
    buf.freeze()
}

/// Encodes `ZC_REQ_TAKEOFF_EQUIP_ACK` (0x099a): `index:u16, location:u32, flag:u8` (0 ok,
/// 1 failed), 9 bytes.
pub fn encode_zc_req_takeoff_equip_ack(index: u16, location: u32, failed: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(9);
    buf.put_u16_le(PacketOpcode::ZC_REQ_TAKEOFF_EQUIP_ACK.raw());
    buf.put_u16_le(index);
    buf.put_u32_le(location);
    buf.put_u8(u8::from(failed));
    buf.freeze()
}

/// Encodes `ZC_ACK_TAKEOFF_EQUIP_ALL` (0x0bae): `result:u8` (0 ok, 1 failed), 3 bytes.
pub fn encode_zc_ack_takeoff_equip_all(failed: bool) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::ZC_ACK_TAKEOFF_EQUIP_ALL.raw());
    buf.put_u8(u8::from(failed));
    buf.freeze()
}

/// Encodes `ZC_EQUIP_ARROW` (0x013c): `index:u16`, 4 bytes.
pub fn encode_zc_equip_arrow(index: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_EQUIP_ARROW.raw());
    buf.put_u16_le(index);
    buf.freeze()
}

/// Encodes `ZC_ACTION_FAILURE` (0x013b): `type:u16`, 4 bytes.
pub fn encode_zc_action_failure(failure_type: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_ACTION_FAILURE.raw());
    buf.put_u16_le(failure_type);
    buf.freeze()
}

/// Encodes `ZC_MSG` (0x0291): `msg_id:u16`, 4 bytes.
pub fn encode_zc_msg(msg_id: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_MSG.raw());
    buf.put_u16_le(msg_id);
    buf.freeze()
}
