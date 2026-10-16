use std::{
    fmt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use bytes::{Buf, BytesMut};
use ro_contracts::{
    http::ServerType,
    packet::{PacketIntentParser, PacketLength, PacketLengthTable, PacketOpcode, RawPacket},
    TARGET_PACKETVER,
};
use ro_packets::{
    incoming::{decode_frame, DecodedFrame},
    PacketDecoder, Phase0IntentParser, RathenaPacketLengthTable,
};
use tokio_util::codec::Decoder;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    ClientToServer,
    ServerToClient,
}

impl fmt::Display for PacketDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientToServer => write!(f, "CLIENT -> SERVER"),
            Self::ServerToClient => write!(f, "SERVER -> CLIENT"),
        }
    }
}

/// A recorded packet entry in the chronological sequence of a session.
#[derive(Debug, Clone)]
pub struct PacketSequenceEntry {
    pub seq: usize,
    pub direction: PacketDirection,
    pub opcode: PacketOpcode,
    pub length: usize,
    pub detail: String,
}

/// Tracks the chronological sequence of packets for an individual connection session.
#[derive(Debug, Default)]
pub struct SessionSequenceTracker {
    seq_counter: AtomicUsize,
    history: Mutex<Vec<PacketSequenceEntry>>,
}

impl SessionSequenceTracker {
    pub fn new() -> Self {
        Self {
            seq_counter: AtomicUsize::new(1),
            history: Mutex::new(Vec::new()),
        }
    }

    pub fn next_seq(&self) -> usize {
        self.seq_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn record(&self, entry: PacketSequenceEntry) {
        if let Ok(mut hist) = self.history.lock() {
            hist.push(entry);
        }
    }

    /// Formats the complete chronological sequence flow observed during this session.
    pub fn format_sequence_flow(&self) -> String {
        let hist = match self.history.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return "Unable to format sequence history (lock poisoned)".to_string(),
        };

        if hist.is_empty() {
            return "No packets recorded in this session.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("Chronological Packet Sequence ({} packets):", hist.len()));
        for entry in &hist {
            lines.push(format!(
                "  Seq #{:<3} [{}] {} (len={}) => {}",
                entry.seq, entry.direction, entry.opcode, entry.length, entry.detail
            ));
        }

        lines.join("\n")
    }
}

pub struct PacketLogger {
    decoder: PacketDecoder<RathenaPacketLengthTable>,
    parser: Phase0IntentParser,
    server_type: ServerType,
    session_id: usize,
    direction: PacketDirection,
    tracker: Arc<SessionSequenceTracker>,
    buffer: BytesMut,
}

impl PacketLogger {
    pub fn new(
        server_type: ServerType,
        session_id: usize,
        direction: PacketDirection,
        tracker: Arc<SessionSequenceTracker>,
    ) -> Self {
        Self {
            decoder: PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER),
            parser: Phase0IntentParser,
            server_type,
            session_id,
            direction,
            tracker,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    /// Feed incoming raw wire bytes into the logger's framing buffer and log all complete packets.
    /// Note: Bytes should already have been forwarded across the network socket.
    pub fn feed_and_log(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);

        // Guard against runaway buffer if completely desynced
        const MAX_BUFFER_LIMIT: usize = 128 * 1024;
        if self.buffer.len() > MAX_BUFFER_LIMIT {
            warn!(
                target: "proxy",
                "[{:?} Session #{}] [{}] Decode buffer exceeded {} bytes; clearing buffer to prevent memory exhaustion",
                self.server_type, self.session_id, self.direction, MAX_BUFFER_LIMIT
            );
            self.buffer.clear();
            return;
        }

        while self.buffer.len() >= 2 {
            match self.decoder.decode(&mut self.buffer) {
                Ok(Some(raw_packet)) => {
                    self.log_raw_packet(&raw_packet);
                }
                Ok(None) => {
                    // Frame incomplete, wait for more data from TCP stream
                    break;
                }
                Err(err) => {
                    let head_opcode = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
                    let opcode = PacketOpcode(head_opcode);
                    let opcode_hex = format!("{:#06x}", head_opcode);

                    let advance_bytes = find_resync_offset(
                        &self.buffer,
                        &RathenaPacketLengthTable,
                        TARGET_PACKETVER,
                    );
                    let consumed_bytes = std::cmp::min(self.buffer.len(), advance_bytes);
                    let raw_payload = if consumed_bytes > 2 {
                        &self.buffer[2..consumed_bytes]
                    } else {
                        &[]
                    };
                    let payload_preview = format_hex_preview(raw_payload);

                    let dyn_hint = if self.buffer.len() >= 4 {
                        let cand_len = u16::from_le_bytes([self.buffer[2], self.buffer[3]]);
                        format!(" (candidate dynamic len={})", cand_len)
                    } else {
                        String::new()
                    };

                    let seq = self.tracker.next_seq();
                    warn!(
                        target: "proxy",
                        "[{:?} Session #{}] [Seq #{:<3}] [{}] UNMAPPED OPCODE: {} (reason: {}, consumed {} bytes){}, payload: {}",
                        self.server_type,
                        self.session_id,
                        seq,
                        self.direction,
                        opcode_hex,
                        err,
                        consumed_bytes,
                        dyn_hint,
                        payload_preview
                    );

                    self.tracker.record(PacketSequenceEntry {
                        seq,
                        direction: self.direction,
                        opcode,
                        length: consumed_bytes,
                        detail: format!(
                            "[UNMAPPED in table.rs, {} bytes] payload: {}",
                            consumed_bytes, payload_preview
                        ),
                    });

                    self.buffer.advance(consumed_bytes);
                }
            }
        }
    }

    fn log_raw_packet(&mut self, raw: &RawPacket) {
        let seq = self.tracker.next_seq();
        let opcode = raw.opcode;
        let total_len = 2 + raw.payload.len();
        let payload_preview = format_hex_preview(&raw.payload);

        let mut detail = format!("payload: {}", payload_preview);

        // Attempt high-level intent parsing using ro-packets
        if self.direction == PacketDirection::ClientToServer {
            match decode_frame(raw, TARGET_PACKETVER) {
                Ok(DecodedFrame::Unhandled { opcode: unhandled_op, len }) => {
                    detail = format!("[Unhandled Intent] payload: {}", payload_preview);
                    info!(
                        target: "proxy",
                        "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} [Unhandled Intent] => payload: {}",
                        self.server_type, self.session_id, seq, self.direction, unhandled_op, len, payload_preview
                    );
                }
                Ok(frame) => {
                    detail = format!("Decoded: {:?}", frame);
                    info!(
                        target: "proxy",
                        "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} => Decoded: {:?}",
                        self.server_type, self.session_id, seq, self.direction, opcode, total_len, frame
                    );
                }
                Err(err) => {
                    // Try fallback parser
                    match self.parser.parse_intent(raw, TARGET_PACKETVER) {
                        Ok(Some(intent)) => {
                            detail = format!("Decoded Intent: {:?}", intent);
                            info!(
                                target: "proxy",
                                "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} => Decoded Intent: {:?}",
                                self.server_type, self.session_id, seq, self.direction, opcode, total_len, intent
                            );
                        }
                        Ok(None) => {
                            info!(
                                target: "proxy",
                                "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} => payload: {}",
                                self.server_type, self.session_id, seq, self.direction, opcode, total_len, payload_preview
                            );
                        }
                        Err(_) => {
                            debug!(
                                target: "proxy",
                                "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} => decode error: {}, payload: {}",
                                self.server_type, self.session_id, seq, self.direction, opcode, total_len, err, payload_preview
                            );
                            info!(
                                target: "proxy",
                                "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} => payload: {}",
                                self.server_type, self.session_id, seq, self.direction, opcode, total_len, payload_preview
                            );
                        }
                    }
                }
            }
        } else {
            // Server to Client packets
            info!(
                target: "proxy",
                "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} => payload: {}",
                self.server_type, self.session_id, seq, self.direction, opcode, total_len, payload_preview
            );
        }

        self.tracker.record(PacketSequenceEntry {
            seq,
            direction: self.direction,
            opcode,
            length: total_len,
            detail,
        });
    }
}

pub fn format_hex_preview(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "[]".to_string();
    }
    const MAX_PREVIEW_BYTES: usize = 32;
    let take_count = std::cmp::min(bytes.len(), MAX_PREVIEW_BYTES);
    let hex_part: Vec<String> = bytes[..take_count].iter().map(|b| format!("{:02x}", b)).collect();
    let ascii_part: String = bytes[..take_count]
        .iter()
        .map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '.' })
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

/// Attempts to find the next valid frame boundary in the buffer when an unmapped opcode is encountered.
///
/// Uses multi-packet chain validation to prevent "phantom opcodes" (random payload bytes
/// matching a known opcode by coincidence) from falsely triggering a frame boundary.
pub fn find_resync_offset(
    buffer: &[u8],
    table: &impl PacketLengthTable,
    packetver: u32,
) -> usize {
    // 1. High-confidence dynamic length check:
    // If bytes 2..4 specify a length L, verify if L points directly to another recognized opcode.
    if buffer.len() >= 4 {
        let cand_len = u16::from_le_bytes([buffer[2], buffer[3]]) as usize;
        if cand_len >= 4 && cand_len <= buffer.len() {
            if buffer.len() >= cand_len + 2 {
                let next_op = u16::from_le_bytes([buffer[cand_len], buffer[cand_len + 1]]);
                if table.packet_length(PacketOpcode(next_op), packetver).is_some() {
                    return cand_len;
                }
            } else if cand_len == buffer.len() {
                return cand_len;
            }
        }
    }

    // 2. Chain validation: Scan forward for an offset K whose predicted packet length
    // points to a second valid opcode (or exact buffer end), confirming a multi-packet sequence.
    for offset in 2..buffer.len().saturating_sub(1) {
        let op = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
        if let Some(pkt_len) = table.packet_length(PacketOpcode(op), packetver) {
            match pkt_len {
                PacketLength::Fixed(sz) => {
                    let next_offset = offset + sz;
                    if buffer.len() >= next_offset + 2 {
                        let follow_op = u16::from_le_bytes([buffer[next_offset], buffer[next_offset + 1]]);
                        if table.packet_length(PacketOpcode(follow_op), packetver).is_some() {
                            return offset; // Validated by two consecutive packets!
                        }
                    } else if next_offset == buffer.len() {
                        return offset; // Packet cleanly completes the buffer
                    }
                }
                PacketLength::Dynamic => {
                    if buffer.len() >= offset + 4 {
                        let dyn_sz = u16::from_le_bytes([buffer[offset + 2], buffer[offset + 3]]) as usize;
                        if dyn_sz >= 4 {
                            let next_offset = offset + dyn_sz;
                            if buffer.len() >= next_offset + 2 {
                                let follow_op = u16::from_le_bytes([buffer[next_offset], buffer[next_offset + 1]]);
                                if table.packet_length(PacketOpcode(follow_op), packetver).is_some() {
                                    return offset;
                                }
                            } else if next_offset == buffer.len() {
                                return offset;
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback: single opcode match if chain confirmation wasn't possible
    for offset in 2..buffer.len().saturating_sub(1) {
        let next_op = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
        if table.packet_length(PacketOpcode(next_op), packetver).is_some() {
            return offset;
        }
    }

    // 4. Fallback: consume 1 byte (or all if buffer <= 4)
    if buffer.len() <= 4 {
        buffer.len()
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hex_preview() {
        assert_eq!(format_hex_preview(&[]), "[]");
        assert_eq!(format_hex_preview(b"RO"), "[52 4f] |RO|");
    }

    #[test]
    fn test_packet_logger_feed_valid_packet_and_sequence() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = PacketLogger::new(
            ServerType::Login,
            1,
            PacketDirection::ClientToServer,
            tracker.clone(),
        );

        // CA_LOGIN frame (55 bytes)
        let mut frame = vec![0x64, 0x00];
        frame.extend_from_slice(&20211103u32.to_le_bytes());
        let mut user = [0u8; 24];
        user[..4].copy_from_slice(b"test");
        frame.extend_from_slice(&user);
        let mut pass = [0u8; 24];
        pass[..4].copy_from_slice(b"pass");
        frame.extend_from_slice(&pass);
        frame.push(1); // client_type

        logger.feed_and_log(&frame[..20]);
        logger.feed_and_log(&frame[20..]);
        assert!(logger.buffer.is_empty());

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("Seq #1"));
        assert!(flow.contains("0x0064"));
    }

    #[test]
    fn test_packet_logger_feed_unknown_and_resync() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = PacketLogger::new(
            ServerType::Char,
            1,
            PacketDirection::ServerToClient,
            tracker.clone(),
        );

        // Unknown 4-byte header e.g. raw AccountId echo
        let raw_head = [0x01, 0x00, 0x00, 0x00];
        logger.feed_and_log(&raw_head);

        // Followed by valid ZC_NOTIFY_TIME (6 bytes: 0x7f, 0x00, tick...)
        let time_packet = [0x7f, 0x00, 0x39, 0x30, 0x00, 0x00];
        logger.feed_and_log(&time_packet);
        assert!(logger.buffer.is_empty());

        // Both the unknown packet and valid packet should be recorded in chronological sequence!
        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("Seq #1"));
        assert!(flow.contains("0x0001"));
        assert!(flow.contains("UNMAPPED in table.rs"));
        assert!(flow.contains("Seq #2"));
        assert!(flow.contains("0x007f"));
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
            0xfe, 0x0a, 0x85, 0x00, 0x99, 0x99, // Unmapped packet (contains phantom 0x0085 at offset 2)
            0x7d, 0x00,                         // Real packet 1 (offset 6, len 2)
            0x7f, 0x00, 0x01, 0x02, 0x03, 0x04, // Real packet 2 (offset 8, len 6)
        ];

        // Without chain validation, offset 2 would be chosen because 0x0085 is in table.rs.
        // With chain validation, offset 6 is chosen because offset 6 (len 2) confirms offset 8 (len 6)!
        let resync_offset = find_resync_offset(&buffer, &table, ver);
        assert_eq!(resync_offset, 6);
    }
}
