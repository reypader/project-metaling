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
