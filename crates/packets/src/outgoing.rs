//! Phase 0 outgoing server packet encoders and serializers.
//!
//! Covers:
//! - `AC_ACCEPT_LOGIN` (0x0069)
//! - `AC_REFUSE_LOGIN` (0x006a)
//! - `HC_ACCEPT_ENTER` (0x006b / 0x099d / 0x082d)
//! - `HC_NOTIFY_ZONESVR` (0x0071)
//! - `ZC_ACCEPT_ENTER` (0x0073)
//! - `ZC_NOTIFY_TIME` (0x007f)
//! - `ZC_NOTIFY_NEWENTRY` (0x009d)
//! - `ZC_PAR_CHANGE` (0x00be)
//! - `ZC_NOTIFY_VANISH` (0x0080)
//! - `ZC_NOTIFY_PLAYERMOVE` (0x0087)

use bytes::{BufMut, Bytes, BytesMut};
use ro_contracts::ids::{
    AccountId, CharId, CharServerInfo, CharacterSummary, MapPosition, Sex,
};
use ro_contracts::packet::{EventEncoder, PacketCodecError, PacketOpcode, ServerEvent};

use crate::coords::{pack_pos2, pack_pos_dir};

/// Writes a string into a fixed-size byte buffer, padding with null bytes.
pub fn write_fixed_string<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let bytes = s.as_bytes();
    let len = bytes.len().min(N);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}

/// Information describing a hosted character server sent in `AC_ACCEPT_LOGIN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharServerEntry {
    pub ip: [u8; 4],
    pub port: u16,
    pub name: String,
    pub users: u16,
    pub server_type: u16,
    pub new_server: u16,
}

impl From<&CharServerInfo> for CharServerEntry {
    fn from(info: &CharServerInfo) -> Self {
        Self {
            ip: info.ip,
            port: info.port,
            name: info.name.clone(),
            users: info.users,
            server_type: info.server_type,
            new_server: info.new_server,
        }
    }
}

impl From<CharServerInfo> for CharServerEntry {
    fn from(info: CharServerInfo) -> Self {
        Self {
            ip: info.ip,
            port: info.port,
            name: info.name,
            users: info.users,
            server_type: info.server_type,
            new_server: info.new_server,
        }
    }
}

/// Encodes `AC_ACCEPT_LOGIN` (0x0069).
///
/// Dynamic packet structure:
/// - 0..2: opcode (0x0069)
/// - 2..4: packet length (47 + 32 * servers.len())
/// - 4..8: login_id1
/// - 8..12: account_id
/// - 12..16: login_id2
/// - 16..20: last_ip
/// - 20..46: last_login (26 bytes)
/// - 46..47: sex (1 byte)
/// - 47..: char server entries (32 bytes each)
pub fn encode_ac_accept_login(
    login_id1: u32,
    account_id: AccountId,
    login_id2: u32,
    last_ip: [u8; 4],
    last_login: &str,
    sex: Sex,
    servers: &[CharServerEntry],
) -> Bytes {
    let total_len = 47 + servers.len() * 32;
    let mut buf = BytesMut::with_capacity(total_len);

    buf.put_u16_le(PacketOpcode::AC_ACCEPT_LOGIN.raw());
    buf.put_u16_le(total_len as u16);
    buf.put_u32_le(login_id1);
    buf.put_u32_le(account_id.0);
    buf.put_u32_le(login_id2);
    buf.put_slice(&last_ip);
    buf.put_slice(&write_fixed_string::<26>(last_login));
    buf.put_u8(sex as u8);

    for s in servers {
        buf.put_slice(&s.ip);
        buf.put_u16_le(s.port);
        buf.put_slice(&write_fixed_string::<20>(&s.name));
        buf.put_u16_le(s.users);
        buf.put_u16_le(s.server_type);
        buf.put_u16_le(s.new_server);
    }

    buf.freeze()
}

/// Encodes `AC_REFUSE_LOGIN` (0x006a).
///
/// Fixed packet structure (23 bytes):
/// - 0..2: opcode (0x006a)
/// - 2..3: error code
/// - 3..23: unblock time (20 bytes)
pub fn encode_ac_refuse_login(error_code: u8, unblock_time: &str) -> Bytes {
    let mut buf = BytesMut::with_capacity(23);
    buf.put_u16_le(PacketOpcode::AC_REFUSE_LOGIN.raw());
    buf.put_u8(error_code);
    buf.put_slice(&write_fixed_string::<20>(unblock_time));
    buf.freeze()
}

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
    buf.put_i32_le(0); // DelRevDate
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
) -> Bytes {
    let total_len = 27 + characters.len() * 175;
    let mut buf = BytesMut::with_capacity(total_len);

    buf.put_u16_le(PacketOpcode::HC_ACCEPT_ENTER.raw());
    buf.put_u16_le(total_len as u16);
    buf.put_u8(total_slots);
    buf.put_u8(premium_start);
    buf.put_u8(premium_end);
    buf.put_bytes(0, 20);

    for c in characters {
        encode_character_info(&mut buf, c);
    }

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
    buf.put_u16_le(0x082d);
    buf.put_u16_le(29);
    buf.put_u8(normal_slots);
    buf.put_u8(premium_slots);
    buf.put_u8(billing_slots);
    buf.put_u8(producible_slots);
    buf.put_u8(total_slots);
    buf.put_bytes(0, 20);
    buf.freeze()
}

/// Encodes `HC_ACK_CHARINFO_PER_PAGE` (0x099d).
///
/// Dynamic packet structure: 4 + 175 * count.
pub fn encode_hc_ack_charinfo_per_page(characters: &[CharacterSummary]) -> Bytes {
    let total_len = 4 + characters.len() * 175;
    let mut buf = BytesMut::with_capacity(total_len);
    buf.put_u16_le(0x099d);
    buf.put_u16_le(total_len as u16);

    for c in characters {
        encode_character_info(&mut buf, c);
    }

    buf.freeze()
}

/// Encodes `HC_NOTIFY_ZONESVR` (0x0071).
///
/// Fixed packet structure (28 bytes):
/// - 0..2: opcode (0x0071)
/// - 2..6: char_id (u32 LE)
/// - 6..22: map name (16 bytes)
/// - 22..26: ip (4 bytes)
/// - 26..28: port (u16 LE)
pub fn encode_hc_notify_zonesvr(
    char_id: CharId,
    map_name: &str,
    ip: [u8; 4],
    port: u16,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(28);
    buf.put_u16_le(PacketOpcode::HC_NOTIFY_ZONESVR.raw());
    buf.put_u32_le(char_id.0);
    buf.put_slice(&write_fixed_string::<16>(map_name));
    buf.put_slice(&ip);
    buf.put_u16_le(port);
    buf.freeze()
}

/// Encodes `ZC_ACCEPT_ENTER` (0x0073).
///
/// Fixed packet structure (11 bytes):
/// - 0..2: opcode (0x0073)
/// - 2..6: start time (u32 LE)
/// - 6..9: pos_dir (3 bytes packed)
/// - 9..10: x_size (u8)
/// - 10..11: y_size (u8)
pub fn encode_zc_accept_enter(
    start_time: u32,
    pos: MapPosition,
    x_size: u8,
    y_size: u8,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(11);
    buf.put_u16_le(PacketOpcode::ZC_ACCEPT_ENTER.raw());
    buf.put_u32_le(start_time);
    buf.put_slice(&pack_pos_dir(pos.x, pos.y, pos.dir));
    buf.put_u8(x_size);
    buf.put_u8(y_size);
    buf.freeze()
}

/// Encodes `ZC_ACCEPT_ENTER2` (0x02eb).
///
/// Fixed packet structure with font (13 bytes).
pub fn encode_zc_accept_enter2(
    start_time: u32,
    pos: MapPosition,
    x_size: u8,
    y_size: u8,
    font: u16,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(13);
    buf.put_u16_le(0x02eb);
    buf.put_u32_le(start_time);
    buf.put_slice(&pack_pos_dir(pos.x, pos.y, pos.dir));
    buf.put_u8(x_size);
    buf.put_u8(y_size);
    buf.put_u16_le(font);
    buf.freeze()
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

/// Parameters required to construct `ZC_NOTIFY_NEWENTRY` (0x009d).
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

/// Encodes `ZC_NOTIFY_NEWENTRY` (0x009d).
///
/// Fixed packet structure (53 bytes).
pub fn encode_zc_notify_newentry(params: &NewEntryParams) -> Bytes {
    let mut buf = BytesMut::with_capacity(53);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_NEWENTRY.raw());
    buf.put_u32_le(params.gid);
    buf.put_i16_le(params.speed);
    buf.put_i16_le(params.body_state);
    buf.put_i16_le(params.health_state);
    buf.put_i16_le(params.effect_state);
    buf.put_i16_le(params.job);
    buf.put_i16_le(params.head);
    buf.put_i16_le(params.weapon);
    buf.put_i16_le(params.accessory);
    buf.put_i16_le(params.shield);
    buf.put_i16_le(params.accessory2);
    buf.put_i16_le(params.accessory3);
    buf.put_i16_le(params.headpalette);
    buf.put_i16_le(params.bodypalette);
    buf.put_i16_le(params.head_dir);
    buf.put_u32_le(params.guid);
    buf.put_i16_le(params.gemblem_ver);
    buf.put_i16_le(params.honor);
    buf.put_i16_le(params.virtue);
    buf.put_u8(params.is_pkmode_on);
    buf.put_u8(params.sex as u8);
    buf.put_slice(&pack_pos_dir(params.pos.x, params.pos.y, params.pos.dir));
    buf.put_u8(params.x_size);
    buf.put_u8(params.y_size);
    buf.put_i16_le(params.clevel);
    buf.freeze()
}

/// Encodes `ZC_PAR_CHANGE` (0x00be / STATUS_CHANGE).
///
/// Fixed packet structure (5 bytes):
/// - 0..2: opcode (0x00be)
/// - 2..4: status/var id (u16 LE)
/// - 4..5: value (u8)
pub fn encode_zc_par_change(var_id: u16, value: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(5);
    buf.put_u16_le(PacketOpcode::ZC_PAR_CHANGE.raw());
    buf.put_u16_le(var_id);
    buf.put_u8(value);
    buf.freeze()
}

/// Encodes `ZC_PAR_CHANGE` with 32-bit count (0x00b0).
///
/// Fixed packet structure (8 bytes):
/// - 0..2: opcode (0x00b0)
/// - 2..4: var id (u16 LE)
/// - 4..8: count (i32 LE)
pub fn encode_zc_par_change_i32(var_id: u16, count: i32) -> Bytes {
    let mut buf = BytesMut::with_capacity(8);
    buf.put_u16_le(0x00b0);
    buf.put_u16_le(var_id);
    buf.put_i32_le(count);
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

/// Parameters for encoding `ZC_NOTIFY_ACT` (0x008a).
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

/// Encodes `ZC_NOTIFY_ACT` (0x008a).
///
/// Fixed packet structure (29 bytes).
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
    let mut buf = BytesMut::with_capacity(29);
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_ACT.raw());
    buf.put_u32_le(source_id);
    buf.put_u32_le(target_id);
    buf.put_u32_le(server_tick);
    buf.put_i32_le(attack_speed);
    buf.put_i32_le(damage_delay);
    buf.put_i16_le(damage);
    buf.put_i16_le(div_count);
    buf.put_u8(action_type);
    buf.put_i16_le(left_damage);
    buf.freeze()
}

/// Encodes `ZC_NOTIFY_ACT` (0x008a) using [`ActParams`].
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

/// Default event encoder connecting domain `ServerEvent` variants to Phase 0 packet serializers.
#[derive(Debug, Default, Clone, Copy)]
pub struct Phase0EventEncoder;

impl EventEncoder for Phase0EventEncoder {
    fn encode_event(
        &self,
        event: &ServerEvent,
        _packetver: u32,
    ) -> Result<Bytes, PacketCodecError> {
        match event {
            ServerEvent::LoginAccepted {
                account_id,
                login_id1,
                login_id2,
                sex,
                char_servers,
            } => {
                let entries: Vec<CharServerEntry> =
                    char_servers.iter().map(CharServerEntry::from).collect();
                Ok(encode_ac_accept_login(
                    *login_id1,
                    *account_id,
                    *login_id2,
                    [0, 0, 0, 0],
                    "",
                    *sex,
                    &entries,
                ))
            }
            ServerEvent::LoginRefused { error_code } => {
                Ok(encode_ac_refuse_login(*error_code, ""))
            }
            ServerEvent::CharacterListLoaded {
                total_slots,
                characters,
                ..
            } => {
                let mut buf = BytesMut::new();
                buf.extend_from_slice(&encode_hc_accept_enter(*total_slots, 0, 0, &[]));
                buf.extend_from_slice(&encode_hc_ack_charinfo_per_page(characters));
                Ok(buf.freeze())
            }
            ServerEvent::CharacterSelected {
                char_id,
                map_name,
                map_host,
                map_port,
            } => Ok(encode_hc_notify_zonesvr(
                *char_id,
                map_name,
                *map_host,
                *map_port,
            )),
            ServerEvent::AcceptEnter { server_tick, pos } => {
                Ok(encode_zc_accept_enter(*server_tick, *pos, 5, 5))
            }
            ServerEvent::TimeSynchronized { server_tick } => {
                Ok(encode_zc_notify_time(*server_tick))
            }
            ServerEvent::EntityMoved {
                start_pos,
                dest_pos,
                move_start_tick,
                ..
            } => {
                Ok(encode_zc_notify_playermove(
                    start_pos.x,
                    start_pos.y,
                    dest_pos.x,
                    dest_pos.y,
                    *move_start_tick,
                ))
            }
            ServerEvent::EntityAppeared {
                char_id,
                pos,
                hair_style,
                hair_color,
                ..
            } => {
                let params = NewEntryParams {
                    gid: char_id.0,
                    speed: 150,
                    body_state: 0,
                    health_state: 0,
                    effect_state: 0,
                    job: 0,
                    head: *hair_style as i16,
                    weapon: 0,
                    accessory: 0,
                    shield: 0,
                    accessory2: 0,
                    accessory3: 0,
                    headpalette: *hair_color as i16,
                    bodypalette: 0,
                    head_dir: 0,
                    guid: 0,
                    gemblem_ver: 0,
                    honor: 0,
                    virtue: 0,
                    is_pkmode_on: 0,
                    sex: Sex::Male,
                    pos: *pos,
                    x_size: 5,
                    y_size: 5,
                    clevel: 1,
                };
                Ok(encode_zc_notify_newentry(&params))
            }
            ServerEvent::EntityVanished {
                char_id,
                vanish_type,
            } => Ok(encode_zc_notify_vanish(char_id.0, *vanish_type)),
            ServerEvent::ParameterChanged { var_id, value } => {
                Ok(encode_zc_par_change(*var_id, (*value & 0xff) as u8))
            }
            ServerEvent::EntityAction {
                source_id,
                target_id,
                action_type,
                action_tick,
            } => {
                if *action_type <= 8 {
                    Ok(encode_zc_notify_act(
                        *source_id,
                        *target_id,
                        *action_tick,
                        0,
                        0,
                        0,
                        1,
                        *action_type,
                        0,
                    ))
                } else {
                    Err(PacketCodecError::UnsupportedEvent(format!(
                        "Action type {action_type} is not implemented"
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        );

        assert_eq!(bytes.len(), 47 + 32);
        assert_eq!(&bytes[0..2], &0x0069u16.to_le_bytes());
        assert_eq!(&bytes[2..4], &(79u16).to_le_bytes());
        assert_eq!(&bytes[4..8], &1111u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &2000001u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &2222u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &[10, 0, 0, 1]);
        assert_eq!(bytes[46], 1); // Male
        assert_eq!(&bytes[47..51], &[127, 0, 0, 1]);
        assert_eq!(&bytes[51..53], &6121u16.to_le_bytes());
    }

    #[test]
    fn test_encode_ac_refuse_login() {
        let bytes = encode_ac_refuse_login(1, "0");
        assert_eq!(bytes.len(), 23);
        assert_eq!(&bytes[0..2], &0x006au16.to_le_bytes());
        assert_eq!(bytes[2], 1);
    }

    #[test]
    fn test_encode_hc_notify_zonesvr() {
        let bytes = encode_hc_notify_zonesvr(
            CharId(150001),
            "prontera.gat",
            [127, 0, 0, 1],
            5121,
        );

        assert_eq!(bytes.len(), 28);
        assert_eq!(&bytes[0..2], &0x0071u16.to_le_bytes());
        assert_eq!(&bytes[2..6], &150001u32.to_le_bytes());
        assert_eq!(&bytes[22..26], &[127, 0, 0, 1]);
        assert_eq!(&bytes[26..28], &5121u16.to_le_bytes());
    }

    #[test]
    fn test_encode_zc_accept_enter() {
        let pos = MapPosition { x: 156, y: 180, dir: 4 };
        let bytes = encode_zc_accept_enter(99999, pos, 5, 5);

        assert_eq!(bytes.len(), 11);
        assert_eq!(&bytes[0..2], &0x0073u16.to_le_bytes());
        assert_eq!(&bytes[2..6], &99999u32.to_le_bytes());
        assert_eq!(bytes[9], 5);
        assert_eq!(bytes[10], 5);
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
            pos: MapPosition { x: 100, y: 100, dir: 0 },
            x_size: 5,
            y_size: 5,
            clevel: 1,
        };

        let bytes = encode_zc_notify_newentry(&params);
        assert_eq!(bytes.len(), 53);
        assert_eq!(&bytes[0..2], &0x009du16.to_le_bytes());
        assert_eq!(&bytes[2..6], &150001u32.to_le_bytes());
    }

    #[test]
    fn test_encode_zc_par_change() {
        let bytes = encode_zc_par_change(10, 50);
        assert_eq!(bytes.len(), 5);
        assert_eq!(&bytes[0..2], &0x00beu16.to_le_bytes());
        assert_eq!(&bytes[2..4], &10u16.to_le_bytes());
        assert_eq!(bytes[4], 50);
    }

    #[test]
    fn test_phase0_event_encoder_all_variants() {
        let encoder = Phase0EventEncoder;
        let ver = ro_contracts::TARGET_PACKETVER;

        // 1. AcceptEnter
        let event = ServerEvent::AcceptEnter {
            server_tick: 5000,
            pos: MapPosition { x: 100, y: 120, dir: 4 },
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 11);
        assert_eq!(&bytes[0..2], &0x0073u16.to_le_bytes());

        // 2. TimeSynchronized
        let event = ServerEvent::TimeSynchronized { server_tick: 9999 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..2], &0x007fu16.to_le_bytes());

        // 3. EntityMoved
        let event = ServerEvent::EntityMoved {
            char_id: CharId(150001),
            start_pos: MapPosition { x: 10, y: 20, dir: 0 },
            dest_pos: MapPosition { x: 30, y: 40, dir: 0 },
            move_start_tick: 1234,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..2], &0x0087u16.to_le_bytes());
        assert_eq!(&bytes[2..6], &1234u32.to_le_bytes());

        // 4. EntityAppeared
        let event = ServerEvent::EntityAppeared {
            char_id: CharId(150001),
            pos: MapPosition { x: 50, y: 60, dir: 2 },
            name: "Novice".to_string(),
            hair_style: 1,
            hair_color: 2,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 53);
        assert_eq!(&bytes[0..2], &0x009du16.to_le_bytes());

        // 5. EntityVanished
        let event = ServerEvent::EntityVanished {
            char_id: CharId(150001),
            vanish_type: 1,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 7);
        assert_eq!(&bytes[0..2], &0x0080u16.to_le_bytes());

        // 6. ParameterChanged
        let event = ServerEvent::ParameterChanged {
            var_id: 10,
            value: 42,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 5);
        assert_eq!(&bytes[0..2], &0x00beu16.to_le_bytes());

        // 7. EntityAction supported
        let event = ServerEvent::EntityAction {
            source_id: 100,
            target_id: 200,
            action_type: 0,
            action_tick: 4321,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 29);
        assert_eq!(&bytes[0..2], &0x008au16.to_le_bytes());

        // 8. EntityAction unsupported
        let event = ServerEvent::EntityAction {
            source_id: 100,
            target_id: 200,
            action_type: 99,
            action_tick: 4321,
        };
        let err = encoder.encode_event(&event, ver).unwrap_err();
        match err {
            PacketCodecError::UnsupportedEvent(msg) => {
                assert!(msg.contains("99"));
            }
            _ => panic!("Expected UnsupportedEvent error"),
        }

        // 9. LoginAccepted
        let event = ServerEvent::LoginAccepted {
            account_id: AccountId(2000001),
            login_id1: 1111,
            login_id2: 2222,
            sex: Sex::Male,
            char_servers: vec![CharServerInfo {
                ip: [127, 0, 0, 1],
                port: 6121,
                name: "Loki".to_string(),
                users: 10,
                server_type: 0,
                new_server: 0,
            }],
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0069u16.to_le_bytes());
        assert_eq!(bytes.len(), 47 + 32);

        // 10. LoginRefused
        let event = ServerEvent::LoginRefused { error_code: 1 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x006au16.to_le_bytes());
        assert_eq!(bytes.len(), 23);
        assert_eq!(bytes[2], 1);

        // 11. CharacterListLoaded
        let event = ServerEvent::CharacterListLoaded {
            account_id: AccountId(2000001),
            total_slots: 9,
            characters: vec![CharacterSummary {
                char_id: CharId(150001),
                account_id: AccountId(2000001),
                slot: 0,
                name: "Hero".to_string(),
                sex: Sex::Male,
                job_class: 0,
                base_level: 1,
                job_level: 1,
                hair_style: 1,
                hair_color: 2,
                last_map: "prontera.gat".to_string(),
                last_pos: MapPosition { x: 50, y: 50, dir: 0 },
            }],
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        // Starts with HC_ACCEPT_ENTER (27 bytes)
        assert_eq!(&bytes[0..2], &0x006bu16.to_le_bytes());
        assert_eq!(&bytes[2..4], &27u16.to_le_bytes());
        assert_eq!(bytes[4], 9);
        // Followed by HC_ACK_CHARINFO_PER_PAGE (4 + 175 = 179 bytes)
        assert_eq!(&bytes[27..29], &0x099du16.to_le_bytes());
        assert_eq!(&bytes[29..31], &179u16.to_le_bytes());
        assert_eq!(bytes.len(), 27 + 179);

        // 12. CharacterSelected
        let event = ServerEvent::CharacterSelected {
            char_id: CharId(150001),
            map_name: "prontera.gat".to_string(),
            map_host: [127, 0, 0, 1],
            map_port: 5121,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0071u16.to_le_bytes());
        assert_eq!(bytes.len(), 28);
        assert_eq!(&bytes[2..6], &150001u32.to_le_bytes());
    }
}
