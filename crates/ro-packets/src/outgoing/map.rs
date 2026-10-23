//! Outgoing map server packet encoders supporting ZC_* opcodes.

use crate::PacketOpcode;
use bytes::{BufMut, Bytes, BytesMut};
use ro_core::{AccountId, MapPosition, UnitAppearance, UnitObjectType};

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

/// Total lengths rAthena writes for the dynamic unit entry packets.
const MOVEENTRY11_LEN: u16 = 114;
const NEWENTRY11_LEN: u16 = 107;
const STANDENTRY11_LEN: u16 = 108;

/// `object_type` on the wire (`clif_bl_type`, `PACKETVER >= 20170726`).
fn object_type(t: UnitObjectType) -> u8 {
    match t {
        UnitObjectType::Pc => 0x0,
        UnitObjectType::Npc => 0x6,
        UnitObjectType::MobSpriteNpc => 0xC,
    }
}

/// `x_size`/`y_size`: rAthena sends 5 for players, 0 for every other unit.
fn cell_size(t: UnitObjectType) -> u8 {
    match t {
        UnitObjectType::Pc => 5,
        UnitObjectType::Npc | UnitObjectType::MobSpriteNpc => 0,
    }
}

/// Writes offsets `0..37`, shared by `0x09fd/0x09fe/0x09ff`: header, ids, speed, status
/// words, job, hair, gear up to `accessory`.
fn put_unit_head(buf: &mut BytesMut, opcode: PacketOpcode, len: u16, unit: &UnitAppearance) {
    buf.put_u16_le(opcode.raw());
    buf.put_u16_le(len);
    buf.put_u8(object_type(unit.object_type));
    buf.put_u32_le(unit.gid); // AID: account id for players, NPC id for NPCs
    buf.put_u32_le(unit.char_id.map_or(0, |c| c.0)); // GID: char id, 0 for non-players
    buf.put_u16_le(unit.speed);
    buf.put_i16_le(0); // body_state (sc.opt1); NOT dead_sit
    buf.put_i16_le(0); // health_state (sc.opt2)
    buf.put_i32_le(unit.effect_state); // effect_state (sc.option)
    buf.put_u16_le(unit.job);
    buf.put_u16_le(unit.hair_style); // head
    buf.put_u32_le(0); // weapon
    buf.put_u32_le(0); // shield
    buf.put_u16_le(0); // accessory (head bottom)
}

/// Writes `accessory2` through `sex` (26 bytes), shared by all three entry packets.
fn put_unit_middle(buf: &mut BytesMut, unit: &UnitAppearance) {
    buf.put_u16_le(0); // accessory2 (head top)
    buf.put_u16_le(0); // accessory3 (head mid)
    buf.put_u16_le(unit.hair_color); // head_palette
    buf.put_i16_le(0); // body_palette
    buf.put_u16_le(unit.head_dir);
    buf.put_u16_le(0); // robe
    buf.put_u32_le(0); // guild_id
    buf.put_i16_le(0); // emblem_ver
    buf.put_i16_le(0); // honor
    buf.put_i32_le(0); // virtue (sc.opt3)
    buf.put_u8(0); // is_pk_mode_on
    buf.put_u8(unit.sex as u8);
}

/// Writes `clevel` through `name` (32 bytes), shared by all three entry packets.
fn put_unit_tail(buf: &mut BytesMut, unit: &UnitAppearance) {
    buf.put_u16_le(unit.clevel);
    buf.put_i16_le(0); // font
    buf.put_i32_le(-1); // max_hp: -1 unless a damaged mob
    buf.put_i32_le(-1); // hp
    buf.put_u8(0); // is_boss
    buf.put_u16_le(0); // body (LOOK_BODY2)
    buf.put_slice(&write_fixed_string::<24>(&unit.name));
}

/// Encodes `ZC_NOTIFY_MOVEENTRY11` (0x09fd): a walking unit, 114 bytes.
/// Cells come from `start`/`dest`; `unit.pos` is not on the wire.
pub fn encode_zc_notify_moveentry11(
    unit: &UnitAppearance,
    start: (u16, u16),
    dest: (u16, u16),
    move_start_time: u32,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(usize::from(MOVEENTRY11_LEN));
    put_unit_head(
        &mut buf,
        PacketOpcode::ZC_NOTIFY_MOVEENTRY11,
        MOVEENTRY11_LEN,
        unit,
    );
    buf.put_u32_le(move_start_time);
    put_unit_middle(&mut buf, unit);
    buf.put_slice(&pack_pos2(start.0, start.1, dest.0, dest.1, 8, 8));
    buf.put_u8(cell_size(unit.object_type));
    buf.put_u8(cell_size(unit.object_type));
    put_unit_tail(&mut buf, unit);
    buf.freeze()
}

/// Encodes `ZC_NOTIFY_NEWENTRY11` (0x09fe): a unit spawned in view, 107 bytes.
pub fn encode_zc_notify_newentry11(unit: &UnitAppearance) -> Bytes {
    let mut buf = BytesMut::with_capacity(usize::from(NEWENTRY11_LEN));
    put_unit_head(
        &mut buf,
        PacketOpcode::ZC_NOTIFY_NEWENTRY11,
        NEWENTRY11_LEN,
        unit,
    );
    put_unit_middle(&mut buf, unit);
    buf.put_slice(&pack_pos_dir(unit.pos.x, unit.pos.y, unit.pos.dir));
    buf.put_u8(cell_size(unit.object_type));
    buf.put_u8(cell_size(unit.object_type));
    put_unit_tail(&mut buf, unit);
    buf.freeze()
}

/// Encodes `ZC_NOTIFY_STANDENTRY11` (0x09ff): an idle unit came into view, 108 bytes.
/// The `state` byte at offset 68 carries `dead_sit`.
pub fn encode_zc_notify_standentry11(unit: &UnitAppearance) -> Bytes {
    let mut buf = BytesMut::with_capacity(usize::from(STANDENTRY11_LEN));
    put_unit_head(
        &mut buf,
        PacketOpcode::ZC_NOTIFY_STANDENTRY11,
        STANDENTRY11_LEN,
        unit,
    );
    put_unit_middle(&mut buf, unit);
    buf.put_slice(&pack_pos_dir(unit.pos.x, unit.pos.y, unit.pos.dir));
    buf.put_u8(cell_size(unit.object_type));
    buf.put_u8(cell_size(unit.object_type));
    buf.put_u8(unit.dead_sit);
    put_unit_tail(&mut buf, unit);
    buf.freeze()
}

/// Encodes `ZC_STOPMOVE` (0x0088), fixed 10 bytes: gid, x, y.
pub fn encode_zc_stopmove(gid: u32, x: u16, y: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(10);
    buf.put_u16_le(PacketOpcode::ZC_STOPMOVE.raw());
    buf.put_u32_le(gid);
    buf.put_u16_le(x);
    buf.put_u16_le(y);
    buf.freeze()
}

/// Encodes `ZC_CHANGE_DIRECTION` (0x009c), fixed 9 bytes: src, head dir (u16), body dir (u8).
pub fn encode_zc_change_direction(gid: u32, head_dir: u16, body_dir: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(9);
    buf.put_u16_le(PacketOpcode::ZC_CHANGE_DIRECTION.raw());
    buf.put_u32_le(gid);
    buf.put_u16_le(head_dir);
    buf.put_u8(body_dir);
    buf.freeze()
}

/// Encodes `ZC_ACK_REQNAMEALL` (0x0a30), fixed 106 bytes: the `CZ_REQNAME` reply for players.
pub fn encode_zc_ack_reqnameall(
    gid: u32,
    name: &str,
    party_name: &str,
    guild_name: &str,
    position_name: &str,
    title_id: i32,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(106);
    buf.put_u16_le(PacketOpcode::ZC_ACK_REQNAMEALL.raw());
    buf.put_u32_le(gid);
    buf.put_slice(&write_fixed_string::<24>(name));
    buf.put_slice(&write_fixed_string::<24>(party_name));
    buf.put_slice(&write_fixed_string::<24>(guild_name));
    buf.put_slice(&write_fixed_string::<24>(position_name));
    buf.put_i32_le(title_id);
    buf.freeze()
}

/// Encodes `ZC_ACK_REQNAMEALL_NPC` (0x0adf), fixed 58 bytes: the `CZ_REQNAME` reply for NPCs.
/// `group_id` and `title` are always empty for static NPCs.
pub fn encode_zc_ack_reqnameall_npc(gid: u32, name: &str) -> Bytes {
    let mut buf = BytesMut::with_capacity(58);
    buf.put_u16_le(PacketOpcode::ZC_ACK_REQNAMEALL_NPC.raw());
    buf.put_u32_le(gid);
    buf.put_i32_le(0); // group_id
    buf.put_slice(&write_fixed_string::<24>(name));
    buf.put_slice(&[0u8; 24]); // title
    buf.freeze()
}

/// Encodes `ZC_NPCACK_MAPMOVE` (0x0091), fixed 22 bytes: 16-byte map name, x, y.
pub fn encode_zc_npcack_mapmove(map_name: &str, x: u16, y: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(22);
    buf.put_u16_le(PacketOpcode::ZC_NPCACK_MAPMOVE.raw());
    buf.put_slice(&write_fixed_string::<16>(map_name));
    buf.put_u16_le(x);
    buf.put_u16_le(y);
    buf.freeze()
}

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

/// Encodes `ZC_ACK_REQ_DISCONNECT` (0x018b), fixed 4 bytes. Result 0 lets the client exit.
pub fn encode_zc_ack_req_disconnect(result: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_ACK_REQ_DISCONNECT.raw());
    buf.put_u16_le(result);
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
    fn test_encode_zc_par_change() {
        let bytes = encode_zc_par_change(10, 50);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &0x00b0u16.to_le_bytes());
        assert_eq!(&bytes[2..4], &10u16.to_le_bytes());
        assert_eq!(&bytes[4..8], &50i32.to_le_bytes());
    }
}
