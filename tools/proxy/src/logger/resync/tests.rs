use ro_core::TARGET_PACKETVER;
use ro_packets::RathenaPacketLengthTable;

use super::*;

#[test]
fn test_format_hex_preview() {
    assert_eq!(format_hex_preview(&[]), "[]");
    assert_eq!(format_hex_preview(b"RO"), "[52 4f] |RO|");
}

#[test]
fn test_format_hex_preview_limited() {
    let bytes = [0xabu8; 40];
    let cut = format_hex_preview_limited(&bytes, DEFAULT_PREVIEW_BYTES);
    assert!(cut.contains("... (+8 bytes)"), "{cut}");
    let full = format_hex_preview_limited(&bytes, usize::MAX);
    assert!(!full.contains("(+"), "{full}");
    assert_eq!(full.matches("ab").count(), 40);
}

#[test]
fn test_find_resync_offset_rejects_phantom_opcode() {
    let table = RathenaPacketLengthTable;
    let ver = TARGET_PACKETVER;

    // Buffer:
    // [0..6]: unmapped 6-byte packet: 0x0AFE with phantom opcode 0x0085 (CZ_REQUEST_MOVE, fixed 6) at bytes [2..4]
    // [6..8]: valid 2-byte packet: 0x007d (CZ_NOTIFY_ACTORINIT, fixed 2)
    // [8..14]: valid 6-byte packet: 0x007f (ZC_NOTIFY_TIME, fixed 6)
    let buffer = [
        0xfe, 0x0a, 0x85, 0x00, 0x99,
        0x99, // Unmapped packet (contains phantom 0x0085 at offset 2)
        0x7d, 0x00, // Real packet 1 (offset 6, len 2)
        0x7f, 0x00, 0x01, 0x02, 0x03, 0x04, // Real packet 2 (offset 8, len 6)
    ];

    // Without chain validation, offset 2 would be chosen because 0x0085 is in table.rs.
    // With chain validation, offset 6 is chosen because offset 6 (len 2) confirms offset 8 (len 6).
    let resync_offset = find_resync_offset(&buffer, &table, ver);
    assert_eq!(resync_offset, 6);
}

#[test]
fn test_find_resync_offset_recovers_at_odd_byte_offset() {
    let table = RathenaPacketLengthTable;
    let ver = TARGET_PACKETVER;

    // Buffer starts with 1 stray byte at offset 0, followed by valid 0x007d and 0x007f packets.
    let buffer = [
        0xAA, // 1 stray byte at offset 0
        0x7d, 0x00, // Real packet 1: CZ_NOTIFY_ACTORINIT (offset 1, len 2)
        0x7f, 0x00, 0x01, 0x02, 0x03, 0x04, // Real packet 2: ZC_NOTIFY_TIME (offset 3, len 6)
    ];

    let resync_offset = find_resync_offset(&buffer, &table, ver);
    assert_eq!(resync_offset, 1);
}

#[test]
fn test_is_valid_resync_boundary() {
    let table = RathenaPacketLengthTable;
    let ver = TARGET_PACKETVER;

    let buffer = [0x7d, 0x00, 0x7f, 0x00, 0x01, 0x02];
    // Offset 0 points to 0x007d (valid)
    assert!(is_valid_resync_boundary(&buffer, 0, &table, ver));
    // Offset 2 points to 0x007f (valid)
    assert!(is_valid_resync_boundary(&buffer, 2, &table, ver));
    // Offset 6 matches buffer.len() (valid EOF boundary)
    assert!(is_valid_resync_boundary(&buffer, 6, &table, ver));
    // Offset 5 is buffer.len() - 1, not matching EOF or having 2 bytes for opcode
    assert!(!is_valid_resync_boundary(&buffer, 5, &table, ver));
    // Offset 7 is beyond buffer.len()
    assert!(!is_valid_resync_boundary(&buffer, 7, &table, ver));
}
