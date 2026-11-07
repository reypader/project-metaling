//! Coordinate bit-packing helpers for Ragnarok Online protocol.

use ro_core::MapPosition;

/// Packs coordinate (x, y) and direction into 3 bytes.
///
/// Bit layout:
/// - Byte 0: x[9..2] (8 bits)
/// - Byte 1: x[1..0] (2 bits) | y[9..4] (6 bits)
/// - Byte 2: y[3..0] (4 bits) | dir[3..0] (4 bits)
pub fn pack_pos_dir(x: u16, y: u16, dir: u8) -> [u8; 3] {
    let mut buf = [0u8; 3];
    buf[0] = (x >> 2) as u8;
    buf[1] = (((x << 6) & 0xc0) as u8) | (((y >> 4) & 0x3f) as u8);
    buf[2] = (((y << 4) & 0xf0) as u8) | (dir & 0x0f);
    buf
}

/// Unpacks 3-byte coordinate into (x, y, dir).
pub fn unpack_pos_dir(buf: &[u8; 3]) -> (u16, u16, u8) {
    let x = (((buf[0] as u16) & 0xff) << 2) | ((buf[1] as u16) >> 6);
    let y = (((buf[1] as u16) & 0x3f) << 4) | ((buf[2] as u16) >> 4);
    let dir = buf[2] & 0x0f;
    (x, y, dir)
}

/// Packs MapPosition into 3 bytes.
pub fn pack_map_position(pos: &MapPosition) -> [u8; 3] {
    pack_pos_dir(pos.x, pos.y, pos.dir)
}

/// Unpacks 3-byte buffer into MapPosition.
pub fn unpack_map_position(buf: &[u8; 3]) -> MapPosition {
    let (x, y, dir) = unpack_pos_dir(buf);
    MapPosition { x, y, dir }
}

/// Packs movement trajectory (x0, y0) -> (x1, y1) with sub-coordinates (sx0, sy0) into 6 bytes.
pub fn pack_pos2(x0: u16, y0: u16, x1: u16, y1: u16, sx0: u8, sy0: u8) -> [u8; 6] {
    let mut buf = [0u8; 6];
    buf[0] = (x0 >> 2) as u8;
    buf[1] = (((x0 << 6) & 0xc0) | ((y0 >> 4) & 0x3f)) as u8;
    buf[2] = (((y0 << 4) & 0xf0) | ((x1 >> 6) & 0x0f)) as u8;
    buf[3] = (((x1 << 2) & 0xfc) | ((y1 >> 8) & 0x03)) as u8;
    buf[4] = (y1 & 0xff) as u8;
    buf[5] = ((sx0 << 4) & 0xf0) | (sy0 & 0x0f);
    buf
}

/// Unpacks 6-byte movement trajectory into (x0, y0, x1, y1, sx0, sy0).
pub fn unpack_pos2(buf: &[u8; 6]) -> (u16, u16, u16, u16, u8, u8) {
    let x0 = (((buf[0] as u16) & 0xff) << 2) | ((buf[1] as u16) >> 6);
    let y0 = (((buf[1] as u16) & 0x3f) << 4) | ((buf[2] as u16) >> 4);
    let x1 = (((buf[2] as u16) & 0x0f) << 6) | ((buf[3] as u16) >> 2);
    let y1 = (((buf[3] as u16) & 0x03) << 8) | (buf[4] as u16);
    let sx0 = (buf[5] & 0xf0) >> 4;
    let sy0 = buf[5] & 0x0f;
    (x0, y0, x1, y1, sx0, sy0)
}

#[cfg(test)]
mod tests;
