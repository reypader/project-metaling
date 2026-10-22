//! Outgoing map server packet encoders supporting ZC_* opcodes.

use bytes::{BufMut, Bytes, BytesMut};
use ro_contracts::ids::{AccountId, CharId, MapPosition, Sex};
use ro_contracts::packet::PacketOpcode;

use crate::coords::{pack_pos2, pack_pos_dir};
use crate::outgoing::helpers::write_fixed_string;

/// Encodes `ZC_AID` (0x0283), fixed 6 bytes.
///
/// Packet structure:
/// - 0..2: opcode (0x0283)
/// - 2..6: account_id (u32 LE)
pub fn encode_zc_aid(account_id: AccountId) -> Bytes {
    let mut buf = BytesMut::with_capacity(6);
    buf.put_u16_le(PacketOpcode::ZC_AID.raw());
    buf.put_u32_le(account_id.0);
    buf.freeze()
}

/// Encodes `ZC_INVENTORY_EXPANSION_INFO` (0x0b18), fixed 4 bytes.
///
/// Packet structure:
/// - 0..2: opcode (0x0b18)
/// - 2..4: expansion_size (i16 LE)
pub fn encode_zc_inventory_expansion_info(expansion_size: i16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_INVENTORY_EXPANSION_INFO.raw());
    buf.put_i16_le(expansion_size);
    buf.freeze()
}

/// Encodes `ZC_ACCEPT_ENTER` (0x02eb), fixed 13 bytes.
pub fn encode_zc_accept_enter(start_time: u32, pos: MapPosition, x_size: u8, y_size: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(13);
    buf.put_u16_le(PacketOpcode::ZC_ACCEPT_ENTER.raw());
    buf.put_u32_le(start_time);
    buf.put_slice(&pack_pos_dir(pos.x, pos.y, pos.dir));
    buf.put_u8(x_size);
    buf.put_u8(y_size);
    buf.put_u16_le(0); // font
    buf.freeze()
}

/// Legacy alias forwarding to canonical [`encode_zc_accept_enter`].
pub fn encode_zc_accept_enter2(
    start_time: u32,
    pos: MapPosition,
    x_size: u8,
    y_size: u8,
    _font: u16,
) -> Bytes {
    encode_zc_accept_enter(start_time, pos, x_size, y_size)
}

/// Encodes `ZC_NOTIFY_TIME` (0x007f).
///
/// Fixed packet structure (6 bytes):
/// - 0..2: opcode (0x007f)
/// - 2..6: server tick (u32 LE)
pub fn encode_zc_notify_time(server_tick: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(6);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_TIME.raw());
    buf.put_u32_le(server_tick);
    buf.freeze()
}

/// Parameters required to construct `ZC_NOTIFY_NEWENTRY` (0x009d) and `ZC_NOTIFY_STANDENTRY` (0x09ff).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntryParams {
    pub gid: u32,
    pub speed: i16,
    pub body_state: i16,
    pub health_state: i16,
    pub effect_state: i16,
    pub job: i16,
    pub head: i16,
    pub weapon: i16,
    pub accessory: i16,
    pub shield: i16,
    pub accessory2: i16,
    pub accessory3: i16,
    pub headpalette: i16,
    pub bodypalette: i16,
    pub head_dir: i16,
    pub guid: u32,
    pub gemblem_ver: i16,
    pub honor: i16,
    pub virtue: i16,
    pub is_pkmode_on: u8,
    pub sex: Sex,
    pub pos: MapPosition,
    pub x_size: u8,
    pub y_size: u8,
    pub clevel: i16,
}

/// Encodes `ZC_NOTIFY_NEWENTRY` (0x09fe) for PACKETVER >= 20150513, 107 bytes dynamic.
pub fn encode_zc_notify_newentry(params: &NewEntryParams) -> Bytes {
    let mut buf = BytesMut::with_capacity(107);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_NEWENTRY11.raw());
    buf.put_u16_le(107);
    buf.put_u8(0); // objecttype: BL_PC = 0
    buf.put_u32_le(params.gid); // AID
    buf.put_u32_le(params.gid); // GID
    buf.put_i16_le(params.speed);
    buf.put_i16_le(params.body_state);
    buf.put_i16_le(params.health_state);
    buf.put_i32_le(params.effect_state as i32);
    buf.put_i16_le(params.job);
    buf.put_u16_le(params.head as u16);
    buf.put_u32_le(params.weapon as u32);
    buf.put_u32_le(params.shield as u32);
    buf.put_u16_le(params.accessory as u16);
    buf.put_u16_le(params.accessory2 as u16);
    buf.put_u16_le(params.accessory3 as u16);
    buf.put_i16_le(params.headpalette);
    buf.put_i16_le(params.bodypalette);
    buf.put_i16_le(params.head_dir);
    buf.put_u16_le(0); // robe
    buf.put_u32_le(params.guid);
    buf.put_i16_le(params.gemblem_ver);
    buf.put_i16_le(params.honor);
    buf.put_i32_le(params.virtue as i32);
    buf.put_u8(params.is_pkmode_on);
    buf.put_u8(params.sex as u8);
    buf.put_slice(&pack_pos_dir(params.pos.x, params.pos.y, params.pos.dir));
    buf.put_u8(params.x_size);
    buf.put_u8(params.y_size);
    buf.put_i16_le(params.clevel);
    buf.put_i16_le(0); // font
    buf.put_i32_le(100); // maxHP
    buf.put_i32_le(100); // HP
    buf.put_u8(0); // isBoss
    buf.put_i16_le(0); // body
    buf.put_slice(&write_fixed_string::<24>(""));
    buf.freeze()
}

pub use encode_zc_notify_newentry as encode_zc_notify_newentry_modern;

/// Encodes `ZC_NOTIFY_STANDENTRY` (0x09ff) for PACKETVER >= 20150513, 108 bytes dynamic.
pub fn encode_zc_notify_standentry(params: &NewEntryParams, state: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(108);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_STANDENTRY11.raw());
    buf.put_u16_le(108);
    buf.put_u8(0); // objecttype: BL_PC = 0
    buf.put_u32_le(params.gid); // AID
    buf.put_u32_le(params.gid); // GID
    buf.put_i16_le(params.speed);
    buf.put_i16_le(params.body_state);
    buf.put_i16_le(params.health_state);
    buf.put_i32_le(params.effect_state as i32);
    buf.put_i16_le(params.job);
    buf.put_u16_le(params.head as u16);
    buf.put_u32_le(params.weapon as u32);
    buf.put_u32_le(params.shield as u32);
    buf.put_u16_le(params.accessory as u16);
    buf.put_u16_le(params.accessory2 as u16);
    buf.put_u16_le(params.accessory3 as u16);
    buf.put_i16_le(params.headpalette);
    buf.put_i16_le(params.bodypalette);
    buf.put_i16_le(params.head_dir);
    buf.put_u16_le(0); // robe
    buf.put_u32_le(params.guid);
    buf.put_i16_le(params.gemblem_ver);
    buf.put_i16_le(params.honor);
    buf.put_i32_le(params.virtue as i32);
    buf.put_u8(params.is_pkmode_on);
    buf.put_u8(params.sex as u8);
    buf.put_slice(&pack_pos_dir(params.pos.x, params.pos.y, params.pos.dir));
    buf.put_u8(params.x_size);
    buf.put_u8(params.y_size);
    buf.put_u8(state);
    buf.put_i16_le(params.clevel);
    buf.put_i16_le(0); // font
    buf.put_i32_le(100); // maxHP
    buf.put_i32_le(100); // HP
    buf.put_u8(0); // isBoss
    buf.put_i16_le(0); // body
    buf.put_slice(&write_fixed_string::<24>(""));
    buf.freeze()
}

pub use encode_zc_notify_standentry as encode_zc_notify_standentry_modern;

/// Encodes `ZC_NOTIFY_MOVE` (0x09fd) for PACKETVER >= 20150513, 114 bytes dynamic.
pub fn encode_zc_notify_move(
    char_id: CharId,
    x0: u16,
    y0: u16,
    x1: u16,
    y1: u16,
    move_start_time: u32,
    name: &str,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(114);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_MOVEENTRY11.raw());
    buf.put_u16_le(114);
    buf.put_u8(0); // objecttype: BL_PC = 0
    buf.put_u32_le(char_id.0); // AID
    buf.put_u32_le(char_id.0); // GID
    buf.put_i16_le(150); // speed
    buf.put_i16_le(0); // body_state
    buf.put_i16_le(0); // health_state
    buf.put_i32_le(0); // effect_state
    buf.put_i16_le(0); // job
    buf.put_u16_le(0); // head
    buf.put_u32_le(0); // weapon
    buf.put_u32_le(0); // shield
    buf.put_u16_le(0); // accessory
    buf.put_u32_le(move_start_time);
    buf.put_u16_le(0); // accessory2
    buf.put_u16_le(0); // accessory3
    buf.put_i16_le(0); // headpalette
    buf.put_i16_le(0); // bodypalette
    buf.put_i16_le(0); // head_dir
    buf.put_u16_le(0); // robe
    buf.put_u32_le(0); // guid
    buf.put_i16_le(0); // gemblem_ver
    buf.put_i16_le(0); // honor
    buf.put_i32_le(0); // virtue
    buf.put_u8(0); // is_pkmode_on
    buf.put_u8(1); // sex: Male
    buf.put_slice(&pack_pos2(x0, y0, x1, y1, 8, 8));
    buf.put_u8(5); // x_size
    buf.put_u8(5); // y_size
    buf.put_i16_le(1); // clevel
    buf.put_i16_le(0); // font
    buf.put_i32_le(100); // maxHP
    buf.put_i32_le(100); // HP
    buf.put_u8(0); // isBoss
    buf.put_i16_le(0); // body
    buf.put_slice(&write_fixed_string::<24>(name));
    buf.freeze()
}

pub use encode_zc_notify_move as encode_zc_notify_move_modern;

/// Encodes `ZC_NOTIFY_PLAYERMOVE` (0x0087).
///
/// Fixed packet structure (12 bytes):
/// - 0..2: opcode (0x0087)
/// - 2..6: move_start_time (u32 LE)
/// - 6..12: move_data (6 bytes packed pos2)
pub fn encode_zc_notify_playermove(
    x0: u16,
    y0: u16,
    x1: u16,
    y1: u16,
    move_start_time: u32,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(12);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_PLAYERMOVE.raw());
    buf.put_u32_le(move_start_time);
    buf.put_slice(&pack_pos2(x0, y0, x1, y1, 8, 8));
    buf.freeze()
}

/// Encodes `ZC_NOTIFY_VANISH` (0x0080).
///
/// Fixed packet structure (7 bytes):
/// - 0..2: opcode (0x0080)
/// - 2..6: gid (u32 LE)
/// - 6..7: vanish type (u8)
pub fn encode_zc_notify_vanish(gid: u32, vanish_type: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(7);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_VANISH.raw());
    buf.put_u32_le(gid);
    buf.put_u8(vanish_type);
    buf.freeze()
}

/// Encodes `ZC_PAR_CHANGE` (0x00b0) with 8-bit value.
pub fn encode_zc_par_change(var_id: u16, value: u8) -> Bytes {
    encode_zc_par_change_i32(var_id, value as i32)
}

/// Encodes `ZC_PAR_CHANGE` with 32-bit count (0x00b0).
///
/// Fixed packet structure (8 bytes):
/// - 0..2: opcode (0x00b0)
/// - 2..4: var id (u16 LE)
/// - 4..8: count (i32 LE)
pub fn encode_zc_par_change_i32(var_id: u16, count: i32) -> Bytes {
    let mut buf = BytesMut::with_capacity(8);
    buf.put_u16_le(PacketOpcode::ZC_PAR_CHANGE.raw());
    buf.put_u16_le(var_id);
    buf.put_i32_le(count);
    buf.freeze()
}

/// Parameters for encoding `ZC_NOTIFY_ACT` (0x08c8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActParams {
    pub source_id: u32,
    pub target_id: u32,
    pub server_tick: u32,
    pub attack_speed: i32,
    pub damage_delay: i32,
    pub damage: i16,
    pub div_count: i16,
    pub action_type: u8,
    pub left_damage: i16,
}

/// Encodes canonical `ZC_NOTIFY_ACT` (0x08c8), fixed 34 bytes.
#[allow(clippy::too_many_arguments)]
pub fn encode_zc_notify_act(
    source_id: u32,
    target_id: u32,
    server_tick: u32,
    attack_speed: i32,
    damage_delay: i32,
    damage: i16,
    div_count: i16,
    action_type: u8,
    left_damage: i16,
) -> Bytes {
    encode_zc_notify_act_modern(
        source_id,
        target_id,
        server_tick,
        attack_speed,
        damage_delay,
        damage as i32,
        0,
        div_count as u16,
        action_type,
        left_damage as i32,
    )
}

/// Encodes `ZC_NOTIFY_ACT` (0x08c8) using [`ActParams`].
pub fn encode_zc_notify_act_params(params: &ActParams) -> Bytes {
    encode_zc_notify_act(
        params.source_id,
        params.target_id,
        params.server_tick,
        params.attack_speed,
        params.damage_delay,
        params.damage,
        params.div_count,
        params.action_type,
        params.left_damage,
    )
}

/// Encodes modern `ZC_NOTIFY_ACT` (0x08c8) for PACKETVER >= 20131223.
#[allow(clippy::too_many_arguments)]
pub fn encode_zc_notify_act_modern(
    source_id: u32,
    target_id: u32,
    server_tick: u32,
    src_speed: i32,
    dmg_speed: i32,
    damage: i32,
    is_sp_damage: u8,
    div_count: u16,
    action_type: u8,
    damage2: i32,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(34);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_ACT.raw());
    buf.put_u32_le(source_id);
    buf.put_u32_le(target_id);
    buf.put_u32_le(server_tick);
    buf.put_i32_le(src_speed);
    buf.put_i32_le(dmg_speed);
    buf.put_i32_le(damage);
    buf.put_u8(is_sp_damage);
    buf.put_u16_le(div_count);
    buf.put_u8(action_type);
    buf.put_i32_le(damage2);
    buf.freeze()
}

/// Encodes `ZC_RESTART_ACK` (0x00b3), fixed 3 bytes.
pub fn encode_zc_restart_ack(result: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(3);
    buf.put_u16_le(PacketOpcode::ZC_RESTART_ACK.raw());
    buf.put_u8(result);
    buf.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_zc_accept_enter() {
        let pos = MapPosition {
            x: 156,
            y: 180,
            dir: 4,
        };
        let bytes = encode_zc_accept_enter(99999, pos, 5, 5);

        assert_eq!(bytes.len(), 13);
        assert_eq!(&bytes[0..2], &0x02ebu16.to_le_bytes());
        assert_eq!(&bytes[2..6], &99999u32.to_le_bytes());
        assert_eq!(bytes[9], 5);
        assert_eq!(bytes[10], 5);
        assert_eq!(&bytes[11..13], &0u16.to_le_bytes());
    }

    #[test]
    fn test_encode_zc_notify_time() {
        let bytes = encode_zc_notify_time(123456);
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..2], &0x007fu16.to_le_bytes());
        assert_eq!(&bytes[2..6], &123456u32.to_le_bytes());
    }

    #[test]
    fn test_encode_zc_notify_newentry() {
        let params = NewEntryParams {
            gid: 150001,
            speed: 150,
            body_state: 0,
            health_state: 0,
            effect_state: 0,
            job: 0,
            head: 1,
            weapon: 0,
            accessory: 0,
            shield: 0,
            accessory2: 0,
            accessory3: 0,
            headpalette: 0,
            bodypalette: 0,
            head_dir: 0,
            guid: 0,
            gemblem_ver: 0,
            honor: 0,
            virtue: 0,
            is_pkmode_on: 0,
            sex: Sex::Male,
            pos: MapPosition {
                x: 100,
                y: 100,
                dir: 0,
            },
            x_size: 5,
            y_size: 5,
            clevel: 1,
        };

        let bytes = encode_zc_notify_newentry(&params);
        assert_eq!(bytes.len(), 107);
        assert_eq!(&bytes[0..2], &0x09feu16.to_le_bytes());
        assert_eq!(&bytes[2..4], &107u16.to_le_bytes());
        assert_eq!(&bytes[5..9], &150001u32.to_le_bytes());
    }

    #[test]
    fn test_encode_zc_par_change() {
        let bytes = encode_zc_par_change(10, 50);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &0x00b0u16.to_le_bytes());
        assert_eq!(&bytes[2..4], &10u16.to_le_bytes());
        assert_eq!(&bytes[4..8], &50i32.to_le_bytes());
    }
}
