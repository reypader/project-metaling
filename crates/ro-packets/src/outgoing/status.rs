//! Character status packet encoders: `0x0acb`, `0x00bd`, `0x0141`, `0x013a`, `0x00be`.
//! Layouts: `reference/catalog/packets/status.md`.

use bytes::{BufMut, Bytes, BytesMut};
use ro_core::StatusWindow;

use crate::PacketOpcode;

/// `0x00bd` frame size.
const STATUS_SIZE: usize = 44;
/// `0x00bd` caps the status-point count at `INT16_MAX`.
const MAX_STATUS_POINT: u16 = 0x7fff;

/// Encodes `ZC_LONGLONGPAR_CHANGE` (0x0acb): `var:u16, value:i64`, 12 bytes.
pub fn encode_zc_longlongpar_change(var_id: u16, value: i64) -> Bytes {
    let mut buf = BytesMut::with_capacity(12);
    buf.put_u16_le(PacketOpcode::ZC_LONGLONGPAR_CHANGE.raw());
    buf.put_u16_le(var_id);
    buf.put_i64_le(value);
    buf.freeze()
}

/// Encodes `ZC_STATUS` (0x00bd), 44 bytes. The status-point count is capped at 32767 and each
/// stat at 255, as rAthena does; a `0x0141` carries the full stat.
pub fn encode_zc_status(status: &StatusWindow) -> Bytes {
    let mut buf = BytesMut::with_capacity(STATUS_SIZE);
    buf.put_u16_le(PacketOpcode::ZC_STATUS.raw());
    buf.put_u16_le(status.status_point.min(MAX_STATUS_POINT));
    for (stat, cost) in status.stats.iter().zip(status.costs) {
        buf.put_u8(u8::try_from(*stat).unwrap_or(u8::MAX));
        buf.put_u8(cost);
    }
    for value in [
        status.atk,
        status.atk_bonus,
        status.matk_bonus,
        status.matk,
        status.soft_def,
        status.hard_def,
        status.soft_mdef,
        status.hard_mdef,
        status.hit,
        status.flee,
        status.perfect_dodge,
        status.critical,
        status.aspd,
        0, // plusASPD
    ] {
        buf.put_i16_le(value);
    }
    buf.freeze()
}

/// Encodes `ZC_COUPLESTATUS` (0x0141): `var:u32, base:i32, bonus:i32`, 14 bytes.
pub fn encode_zc_couplestatus(var_id: u32, base: i32, bonus: i32) -> Bytes {
    let mut buf = BytesMut::with_capacity(14);
    buf.put_u16_le(PacketOpcode::ZC_COUPLESTATUS.raw());
    buf.put_u32_le(var_id);
    buf.put_i32_le(base);
    buf.put_i32_le(bonus);
    buf.freeze()
}

/// Encodes `ZC_ATTACK_RANGE` (0x013a): `range:u16`, 4 bytes.
pub fn encode_zc_attack_range(range: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_ATTACK_RANGE.raw());
    buf.put_u16_le(range);
    buf.freeze()
}

/// Encodes `ZC_STATUS_CHANGE` (0x00be): `var:u16, value:u8`, 5 bytes.
pub fn encode_zc_status_change(var_id: u16, value: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(5);
    buf.put_u16_le(PacketOpcode::ZC_STATUS_CHANGE.raw());
    buf.put_u16_le(var_id);
    buf.put_u8(value);
    buf.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_44_bytes_and_saturates() {
        let status = StatusWindow {
            status_point: 40_000,
            stats: [300, 2, 3, 4, 5, 6],
            costs: [0, 2, 2, 2, 2, 2],
            aspd: 590,
            ..StatusWindow::default()
        };
        let bytes = encode_zc_status(&status);
        assert_eq!(bytes.len(), 44);
        assert_eq!(&bytes[..6], &[0xbd, 0x00, 0xff, 0x7f, 0xff, 0x00]);
        assert_eq!(&bytes[40..44], &[0x4e, 0x02, 0x00, 0x00]);
    }
}
