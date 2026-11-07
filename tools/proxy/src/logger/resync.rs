use std::cmp::min;

use ro_packets::{PacketLength, PacketLengthTable, PacketOpcode};

/// How many payload bytes a log line shows unless `--full-payload` is on.
pub const DEFAULT_PREVIEW_BYTES: usize = 32;

/// Formats a byte slice as a hex dump preview with ASCII representation.
pub fn format_hex_preview(bytes: &[u8]) -> String {
    format_hex_preview_limited(bytes, DEFAULT_PREVIEW_BYTES)
}

/// Like [`format_hex_preview`], but shows at most `max_bytes` bytes (`usize::MAX` shows all).
pub fn format_hex_preview_limited(bytes: &[u8], max_bytes: usize) -> String {
    if bytes.is_empty() {
        return "[]".to_string();
    }
    let take_count = min(bytes.len(), max_bytes);
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

    if bytes.len() > take_count {
        format!(
            "[{} ... (+{} bytes)] |{}|",
            hex_part.join(" "),
            bytes.len() - take_count,
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
mod tests;
