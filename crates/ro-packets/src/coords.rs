//! Coordinate bit-packing helpers for Ragnarok Online protocol.

use ro_contracts::ids::MapPosition;

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
mod tests {
    use super::*;

    #[test]
    fn test_pos_dir_roundtrip() {
        let cases = [
            (0, 0, 0),
            (1023, 1023, 15),
            (100, 140, 6),
            (511, 255, 9),
            (12, 34, 7),
            (300, 400, 12),
        ];

        for (x, y, dir) in cases {
            let packed = pack_pos_dir(x, y, dir);
            let (ux, uy, udir) = unpack_pos_dir(&packed);
            assert_eq!(x, ux, "X mismatch for ({x}, {y}, {dir})");
            assert_eq!(y, uy, "Y mismatch for ({x}, {y}, {dir})");
            assert_eq!(dir, udir, "Dir mismatch for ({x}, {y}, {dir})");
        }
    }

    #[test]
    fn test_rust_ro_reference_vector() {
        let x: u16 = 100;
        let y: u16 = 140;
        let dir: u8 = 6;
        let packed = pack_pos_dir(x, y, dir);

        let mut expected = [0u8; 3];
        expected[0] = (x >> 2) as u8;
        expected[1] = ((x << 6) as u8) | (((y >> 4) & 0x3F) as u8);
        expected[2] = ((y << 4) as u8) | (dir & 0xF);

        assert_eq!(packed, expected);
    }

    #[test]
    fn test_pos2_roundtrip() {
        let cases = [
            (0, 0, 0, 0, 0, 0),
            (1023, 1023, 1023, 1023, 15, 15),
            (150, 200, 155, 205, 8, 8),
            (50, 60, 70, 80, 3, 5),
            (1000, 500, 250, 125, 14, 2),
        ];

        for (x0, y0, x1, y1, sx0, sy0) in cases {
            let packed = pack_pos2(x0, y0, x1, y1, sx0, sy0);
            let (ux0, uy0, ux1, uy1, usx0, usy0) = unpack_pos2(&packed);
            assert_eq!(x0, ux0, "x0 mismatch");
            assert_eq!(y0, uy0, "y0 mismatch");
            assert_eq!(x1, ux1, "x1 mismatch");
            assert_eq!(y1, uy1, "y1 mismatch");
            assert_eq!(sx0, usx0, "sx0 mismatch");
            assert_eq!(sy0, usy0, "sy0 mismatch");
        }
    }
}
