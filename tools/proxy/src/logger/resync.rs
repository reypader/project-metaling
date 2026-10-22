use std::cmp::min;

use ro_core::packet::{PacketLength, PacketLengthTable, PacketOpcode};

/// Formats a byte slice as a hex dump preview with ASCII representation.
pub fn format_hex_preview(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "[]".to_string();
    }
    const MAX_PREVIEW_BYTES: usize = 32;
    let take_count = min(bytes.len(), MAX_PREVIEW_BYTES);
    let hex_part: Vec<String> = bytes[..take_count]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    let ascii_part: String = bytes[..take_count]
        .iter()
        .map(|&b| {
            if (0x20..=0x7e).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect();

    if bytes.len() > MAX_PREVIEW_BYTES {
        format!(
            "[{} ... (+{} bytes)] |{}|",
            hex_part.join(" "),
            bytes.len() - MAX_PREVIEW_BYTES,
            ascii_part
        )
    } else {
        format!("[{}] |{}|", hex_part.join(" "), ascii_part)
    }
}

/// Verifies if a candidate next offset forms a valid packet boundary.
///
/// Returns true if the next offset points to a known opcode in the table,
/// or if it matches the exact end of the buffer.
pub fn is_valid_resync_boundary(
    buffer: &[u8],
    next_offset: usize,
    table: &impl PacketLengthTable,
    packetver: u32,
) -> bool {
    if buffer.len() >= next_offset + 2 {
        let follow_op = u16::from_le_bytes([buffer[next_offset], buffer[next_offset + 1]]);
        table
            .packet_length(PacketOpcode(follow_op), packetver)
            .is_some()
    } else {
        next_offset == buffer.len()
    }
}

/// Attempts to find the next valid frame boundary in the buffer when an unmapped opcode is encountered.
///
/// Uses multi-packet chain validation to prevent phantom opcodes (random payload bytes
/// matching a known opcode by coincidence) from falsely triggering a frame boundary.
pub fn find_resync_offset(buffer: &[u8], table: &impl PacketLengthTable, packetver: u32) -> usize {
    // 1. High-confidence dynamic length check:
    // If bytes 2..4 specify a length L, verify if L points directly to another recognized opcode.
    if buffer.len() >= 4 {
        let cand_len = u16::from_le_bytes([buffer[2], buffer[3]]) as usize;
        if cand_len >= 4
            && cand_len <= buffer.len()
            && is_valid_resync_boundary(buffer, cand_len, table, packetver)
        {
            return cand_len;
        }
    }

    // 2. Chain validation: Scan forward for an offset K whose predicted packet length
    // points to a second valid opcode (or exact buffer end), confirming a multi-packet sequence.
    for offset in 1..buffer.len().saturating_sub(1) {
        let op = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
        if let Some(pkt_len) = table.packet_length(PacketOpcode(op), packetver) {
            match pkt_len {
                PacketLength::Fixed(sz) => {
                    let next_offset = offset + sz;
                    if is_valid_resync_boundary(buffer, next_offset, table, packetver) {
                        return offset;
                    }
                }
                PacketLength::Dynamic => {
                    if buffer.len() >= offset + 4 {
                        let dyn_sz =
                            u16::from_le_bytes([buffer[offset + 2], buffer[offset + 3]]) as usize;
                        if dyn_sz >= 4 {
                            let next_offset = offset + dyn_sz;
                            if is_valid_resync_boundary(buffer, next_offset, table, packetver) {
                                return offset;
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback: single opcode match if chain confirmation was not possible.
    for offset in 1..buffer.len().saturating_sub(1) {
        let next_op = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
        if table
            .packet_length(PacketOpcode(next_op), packetver)
            .is_some()
        {
            return offset;
        }
    }

    // 4. Fallback: consume 1 byte (or all if buffer <= 4).
    if buffer.len() <= 4 {
        buffer.len()
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use ro_core::TARGET_PACKETVER;
    use ro_packets::RathenaPacketLengthTable;

    use super::*;

    #[test]
    fn test_format_hex_preview() {
        assert_eq!(format_hex_preview(&[]), "[]");
        assert_eq!(format_hex_preview(b"RO"), "[52 4f] |RO|");
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
            0x7f, 0x00, 0x01, 0x02, 0x03,
            0x04, // Real packet 2: ZC_NOTIFY_TIME (offset 3, len 6)
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
}
