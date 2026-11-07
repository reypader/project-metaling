//! Outgoing login server packet encoders supporting AC_* opcodes.

use crate::{PacketCodecError, PacketOpcode};
use bytes::{BufMut, Bytes, BytesMut};
use ro_core::{AccountId, CharServerInfo, Sex};

use crate::outgoing::helpers::write_fixed_string;

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

/// Encodes `AC_ACCEPT_LOGIN` (0x0ac4).
///
/// Dynamic packet structure:
/// - 0..2: opcode (0x0ac4)
/// - 2..4: packet length (64 + 160 * servers.len())
/// - 4..8: login_id1
/// - 8..12: account_id
/// - 12..16: login_id2
/// - 16..20: last_ip
/// - 20..46: last_login (26 bytes)
/// - 46..47: sex (1 byte)
/// - 47..64: token (17 bytes)
/// - 64..: char server entries (160 bytes each)
pub fn encode_ac_accept_login(
    login_id1: u32,
    account_id: AccountId,
    login_id2: u32,
    last_ip: [u8; 4],
    last_login: &str,
    sex: Sex,
    servers: &[CharServerEntry],
) -> Result<Bytes, PacketCodecError> {
    let total_len = servers
        .len()
        .checked_mul(160)
        .and_then(|len| len.checked_add(64));
    let total_len = match total_len {
        Some(len) if len <= u16::MAX as usize => len,
        Some(len) => {
            return Err(PacketCodecError::PacketTooLarge {
                opcode: PacketOpcode::AC_ACCEPT_LOGIN,
                length: len,
                max: u16::MAX as usize,
            });
        }
        None => {
            return Err(PacketCodecError::PacketTooLarge {
                opcode: PacketOpcode::AC_ACCEPT_LOGIN,
                length: usize::MAX,
                max: u16::MAX as usize,
            });
        }
    };
    let mut buf = BytesMut::with_capacity(total_len);

    buf.put_u16_le(PacketOpcode::AC_ACCEPT_LOGIN.raw());
    buf.put_u16_le(total_len as u16);
    buf.put_u32_le(login_id1);
    buf.put_u32_le(account_id.0);
    buf.put_u32_le(login_id2);
    buf.put_slice(&last_ip);
    buf.put_slice(&write_fixed_string::<26>(last_login));
    buf.put_u8(sex as u8);
    buf.put_slice(&write_fixed_string::<17>(""));

    for s in servers {
        buf.put_slice(&s.ip);
        buf.put_u16_le(s.port);
        buf.put_slice(&write_fixed_string::<20>(&s.name));
        buf.put_u16_le(s.users);
        buf.put_u16_le(s.server_type);
        buf.put_u16_le(s.new_server);
        buf.put_bytes(0, 128);
    }

    Ok(buf.freeze())
}

/// Legacy alias forwarding to canonical [`encode_ac_accept_login`].
#[allow(clippy::too_many_arguments)]
pub fn encode_ac_accept_login_modern(
    login_id1: u32,
    account_id: AccountId,
    login_id2: u32,
    last_ip: [u8; 4],
    last_login: &str,
    sex: Sex,
    _token: &str,
    servers: &[CharServerEntry],
) -> Result<Bytes, PacketCodecError> {
    encode_ac_accept_login(
        login_id1, account_id, login_id2, last_ip, last_login, sex, servers,
    )
}

/// Encodes `AC_REFUSE_LOGIN` (0x083e).
///
/// Fixed packet structure (26 bytes):
/// - 0..2: opcode (0x083e)
/// - 2..6: error code (u32 LE)
/// - 6..26: unblock time (20 bytes)
pub fn encode_ac_refuse_login(error_code: u32, unblock_time: &str) -> Bytes {
    let mut buf = BytesMut::with_capacity(26);
    buf.put_u16_le(PacketOpcode::AC_REFUSE_LOGIN.raw());
    buf.put_u32_le(error_code);
    buf.put_slice(&write_fixed_string::<20>(unblock_time));
    buf.freeze()
}

pub use encode_ac_refuse_login as encode_ac_refuse_login_modern;

#[cfg(test)]
mod tests;
