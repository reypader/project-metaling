//! Packed cell coordinates (PKT-05, 06).

use super::*;

// ---------- PKT-05/06 ----------

#[test]
fn pkt_05_wbufpos_vector_and_round_trip() {
    assert_eq!(pack_pos_dir(100, 140, 6), [0x19, 0x08, 0xc6]);
    for (x, y, d) in [(0, 0, 0), (1023, 1023, 7), (104, 100, 4)] {
        assert_eq!(unpack_pos_dir(&pack_pos_dir(x, y, d)), (x, y, d));
    }
}

#[test]
fn pkt_06_wbufpos2_round_trip() {
    for v in [
        (0, 0, 0, 0, 8, 8),
        (1023, 1023, 1023, 1023, 15, 15),
        (100, 100, 104, 100, 8, 8),
    ] {
        assert_eq!(unpack_pos2(&pack_pos2(v.0, v.1, v.2, v.3, v.4, v.5)), v);
    }
}
