use std::{
    fmt,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::SystemTime,
};
use bytes::{Buf, BytesMut};
use ro_contracts::{
    http::ServerType,
    packet::{PacketIntentParser, PacketLength, PacketLengthTable, PacketOpcode, RawPacket},
    TARGET_PACKETVER,
};
use ro_packets::{
    incoming::{decode_frame, DecodedFrame},
    is_implemented_outbound_opcode, PacketDecoder, Phase0IntentParser, RathenaPacketLengthTable,
};
use tokio_util::codec::Decoder;
use tracing::{debug, warn};

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

/// A recorded packet entry in the global sequence across all sessions and servers.
#[derive(Debug, Clone)]
pub struct GlobalSequenceEntry {
    pub global_seq: usize,
    pub session_id: usize,
    pub server_type: ServerType,
    pub direction: PacketDirection,
    pub opcode: PacketOpcode,
    pub length: usize,
    pub detail: String,
    pub timestamp: SystemTime,
}

/// Thread-safe recorder capturing packet flow across all proxy sessions and servers.
#[derive(Debug, Default)]
pub struct PacketSequenceRecorder {
    entries: Mutex<Vec<GlobalSequenceEntry>>,
}

impl PacketSequenceRecorder {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn record(
        &self,
        session_id: usize,
        server_type: ServerType,
        direction: PacketDirection,
        opcode: PacketOpcode,
        length: usize,
        detail: String,
    ) -> usize {
        if let Ok(mut entries) = self.entries.lock() {
            let global_seq = entries.len() + 1;
            let entry = GlobalSequenceEntry {
                global_seq,
                session_id,
                server_type,
                direction,
                opcode,
                length,
                detail,
                timestamp: SystemTime::now(),
            };
            entries.push(entry);
            global_seq
        } else {
            0
        }
    }

    pub fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> Vec<GlobalSequenceEntry> {
        let mut list = self.entries.lock().map(|e| e.clone()).unwrap_or_default();
        list.sort_by_key(|e| e.global_seq);
        list
    }

    pub fn format_log(&self, generated_at: SystemTime) -> String {
        let entries = self.entries();
        self.format_entries(&entries, generated_at)
    }

    pub fn format_entries(&self, entries: &[GlobalSequenceEntry], generated_at: SystemTime) -> String {
        let (gy, gm, gd, gh, gmin, gs, gms) = civil_from_timestamp(generated_at);
        let timestamp_str = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} UTC",
            gy, gm, gd, gh, gmin, gs, gms
        );

        let mut login_count = 0usize;
        let mut char_count = 0usize;
        let mut map_count = 0usize;

        for entry in entries {
            match entry.server_type {
                ServerType::Login => login_count += 1,
                ServerType::Char => char_count += 1,
                ServerType::Map => map_count += 1,
            }
        }

        let mut output = String::new();
        output.push_str("================================================================================\n");
        output.push_str("Ragnarok Online Packet Proxy - Global Sequence Log\n");
        output.push_str(&format!("Target PACKETVER: {}\n", TARGET_PACKETVER));
        output.push_str(&format!("Generated: {}\n", timestamp_str));
        output.push_str(&format!("Total Packets Recorded: {}\n", entries.len()));
        output.push_str("================================================================================\n\n");

        output.push_str("Chronological Packet Sequence:\n");
        if entries.is_empty() {
            output.push_str("No packets recorded during this session.\n\n");
        } else {
            output.push_str(&format!(
                "{:<6} {:<23} {:<7} {:<8} {:<18} {:<24} {:<6} {}\n",
                "#", "Timestamp (UTC)", "Server", "Session", "Direction", "Opcode", "Len", "Details"
            ));
            output.push_str(&"-".repeat(110));
            output.push('\n');

            for entry in entries {
                let (ey, em, ed, eh, emin, es, ems) = civil_from_timestamp(entry.timestamp);
                let entry_time = format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
                    ey, em, ed, eh, emin, es, ems
                );
                let srv = match entry.server_type {
                    ServerType::Login => "Login",
                    ServerType::Char => "Char",
                    ServerType::Map => "Map",
                };
                let sess = format!("#{}", entry.session_id);
                let dir = match entry.direction {
                    PacketDirection::ClientToServer => "CLIENT -> SERVER",
                    PacketDirection::ServerToClient => "SERVER -> CLIENT",
                };
                let op = format!("{}", entry.opcode);

                output.push_str(&format!(
                    "{:<6} {:<23} {:<7} {:<8} {:<18} {:<24} {:<6} {}\n",
                    entry.global_seq, entry_time, srv, sess, dir, op, entry.length, entry.detail
                ));
            }
            output.push('\n');
        }

        output.push_str("================================================================================\n");
        output.push_str("Summary by Server:\n");
        output.push_str(&format!("  Login Server: {} packets\n", login_count));
        output.push_str(&format!("  Char Server:  {} packets\n", char_count));
        output.push_str(&format!("  Map Server:   {} packets\n", map_count));
        output.push_str(&format!("  Total:        {} packets\n", entries.len()));
        output.push_str("================================================================================\n");

        output
    }

    pub fn write_to_file(
        &self,
        path: impl AsRef<Path>,
        generated_at: SystemTime,
    ) -> std::io::Result<usize> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let entries = self.entries();
        let count = entries.len();
        let content = self.format_entries(&entries, generated_at);
        std::fs::write(p, content)?;
        Ok(count)
    }
}

/// Tracks the chronological sequence of packets for an individual connection session.
#[derive(Debug, Default)]
pub struct SessionSequenceTracker {
    seq_counter: AtomicUsize,
    packet_counter: AtomicUsize,
    history: Mutex<Vec<PacketSequenceEntry>>,
    global_sink: Option<(ServerType, usize, Arc<PacketSequenceRecorder>)>,
}

impl SessionSequenceTracker {
    pub fn new() -> Self {
        Self {
            seq_counter: AtomicUsize::new(1),
            packet_counter: AtomicUsize::new(0),
            history: Mutex::new(Vec::new()),
            global_sink: None,
        }
    }

    pub fn with_global(
        server_type: ServerType,
        session_id: usize,
        recorder: Arc<PacketSequenceRecorder>,
    ) -> Self {
        Self {
            seq_counter: AtomicUsize::new(1),
            packet_counter: AtomicUsize::new(0),
            history: Mutex::new(Vec::new()),
            global_sink: Some((server_type, session_id, recorder)),
        }
    }

    pub fn next_seq(&self) -> usize {
        self.seq_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn record(&self, entry: PacketSequenceEntry) {
        self.packet_counter.fetch_add(1, Ordering::SeqCst);
        if let Some((server_type, session_id, ref recorder)) = self.global_sink {
            recorder.record(
                session_id,
                server_type,
                entry.direction,
                entry.opcode,
                entry.length,
                entry.detail.clone(),
            );
        }
        if let Ok(mut hist) = self.history.lock() {
            hist.push(entry);
        }
    }

    pub fn packet_count(&self) -> usize {
        self.packet_counter.load(Ordering::SeqCst)
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
                    debug!(
                        target: "proxy",
                        "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} [UNKNOWN / UNMAPPED in table.rs] (reason: {}, consumed {} bytes){}, payload: {}",
                        self.server_type,
                        self.session_id,
                        seq,
                        self.direction,
                        opcode,
                        consumed_bytes,
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
                            "[UNKNOWN / UNMAPPED in table.rs, {} bytes] payload: {}",
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
        let is_dynamic = matches!(
            RathenaPacketLengthTable.packet_length(opcode, TARGET_PACKETVER),
            Some(PacketLength::Dynamic)
        );
        let total_len = if is_dynamic {
            4 + raw.payload.len()
        } else {
            2 + raw.payload.len()
        };
        let payload_preview = format_hex_preview(&raw.payload);

        let (is_implemented, detail_msg) = if self.direction == PacketDirection::ClientToServer {
            match decode_frame(raw, TARGET_PACKETVER) {
                Ok(DecodedFrame::Unhandled { .. }) => {
                    match self.parser.parse_intent(raw, TARGET_PACKETVER) {
                        Ok(Some(intent)) => (true, format!("Decoded Intent: {:?}", intent)),
                        _ => (false, format!("payload: {}", payload_preview)),
                    }
                }
                Ok(frame) => (true, format!("Decoded: {:?}", frame)),
                Err(_) => {
                    match self.parser.parse_intent(raw, TARGET_PACKETVER) {
                        Ok(Some(intent)) => (true, format!("Decoded Intent: {:?}", intent)),
                        _ => (false, format!("payload: {}", payload_preview)),
                    }
                }
            }
        } else {
            let implemented = is_implemented_outbound_opcode(opcode, TARGET_PACKETVER);
            (implemented, format!("payload: {}", payload_preview))
        };

        let status_tag = if is_implemented {
            "[IMPLEMENTED]"
        } else {
            "[KNOWN / UNIMPLEMENTED]"
        };

        let detail = format!("{} {}", status_tag, detail_msg);

        debug!(
            target: "proxy",
            "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} {} => {}",
            self.server_type, self.session_id, seq, self.direction, opcode, total_len, status_tag, detail_msg
        );

        self.tracker.record(PacketSequenceEntry {
            seq,
            direction: self.direction,
            opcode,
            length: total_len,
            detail,
        });
    }
}

/// Converts a `SystemTime` into civil date components (year, month, day, hour, minute, second, millis).
pub fn civil_from_timestamp(time: SystemTime) -> (i64, u32, u32, u32, u32, u32, u32) {
    let (secs, subsec_millis) = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(dur) => (dur.as_secs(), dur.subsec_millis()),
        Err(_) => (0, 0),
    };
    let days = (secs / 86400) as i64;
    let day_secs = (secs % 86400) as u32;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    (year, m as u32, d as u32, hour, minute, second, subsec_millis)
}

/// Generates a standardized log filename: `packet_sequence-%Y%m%d_%H%M%S.log`.
pub fn generate_log_filename(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second, _) = civil_from_timestamp(time);
    format!(
        "packet_sequence-{:04}{:02}{:02}_{:02}{:02}{:02}.log",
        year, month, day, hour, minute, second
    )
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
        assert!(flow.contains("[UNKNOWN / UNMAPPED in table.rs"));
        assert!(flow.contains("Seq #2"));
        assert!(flow.contains("0x007f"));
        assert!(flow.contains("ZC_NOTIFY_TIME"));
        assert!(flow.contains("[IMPLEMENTED]"));
    }

    #[test]
    fn test_packet_logger_implemented_inbound_and_outbound() {
        let tracker = Arc::new(SessionSequenceTracker::new());

        // Implemented Inbound: CA_LOGIN (0x0064, fixed 55 bytes)
        let mut in_logger = PacketLogger::new(
            ServerType::Login,
            1,
            PacketDirection::ClientToServer,
            tracker.clone(),
        );
        let mut ca_login = vec![0x64, 0x00];
        ca_login.extend_from_slice(&20211103u32.to_le_bytes());
        ca_login.extend_from_slice(&[0u8; 49]); // 24 user + 24 pass + 1 client_type
        in_logger.feed_and_log(&ca_login);

        // Implemented Outbound: ZC_NOTIFY_TIME (0x007f, fixed 6 bytes)
        let mut out_logger = PacketLogger::new(
            ServerType::Map,
            1,
            PacketDirection::ServerToClient,
            tracker.clone(),
        );
        let zc_time = [0x7f, 0x00, 0x12, 0x34, 0x56, 0x78];
        out_logger.feed_and_log(&zc_time);

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("CA_LOGIN"));
        assert!(flow.contains("0x0064 (CA_LOGIN)"));
        assert!(flow.contains("[IMPLEMENTED]"));

        assert!(flow.contains("ZC_NOTIFY_TIME"));
        assert!(flow.contains("0x007f (ZC_NOTIFY_TIME)"));
        assert!(flow.contains("[IMPLEMENTED]"));
    }

    #[test]
    fn test_packet_logger_known_unimplemented_inbound_and_outbound() {
        let tracker = Arc::new(SessionSequenceTracker::new());

        // Inbound known in table.rs but unimplemented: CZ_REQUEST_QUIT (0x0082, fixed 2 bytes)
        let mut in_logger = PacketLogger::new(
            ServerType::Map,
            1,
            PacketDirection::ClientToServer,
            tracker.clone(),
        );
        in_logger.feed_and_log(&[0x82, 0x00]);

        // Outbound known in table.rs but unimplemented: ZC_NPCACK_ENABLE (0x0093, fixed 2 bytes)
        let mut out_logger = PacketLogger::new(
            ServerType::Map,
            1,
            PacketDirection::ServerToClient,
            tracker.clone(),
        );
        out_logger.feed_and_log(&[0x93, 0x00]);

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("CZ_REQUEST_QUIT"));
        assert!(flow.contains("0x0082 (CZ_REQUEST_QUIT)"));
        assert!(flow.contains("ZC_NPCACK_ENABLE"));
        assert!(flow.contains("0x0093 (ZC_NPCACK_ENABLE)"));
        assert!(flow.contains("[KNOWN / UNIMPLEMENTED]"));
    }

    #[test]
    fn test_packet_logger_totally_unknown_unmapped() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = PacketLogger::new(
            ServerType::Char,
            1,
            PacketDirection::ClientToServer,
            tracker.clone(),
        );
        logger.feed_and_log(&[0x01, 0x00, 0x00, 0x00]);

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("0x0001"));
        assert!(flow.contains("[UNKNOWN / UNMAPPED in table.rs"));
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

    #[test]
    fn test_generate_log_filename() {
        // 2026-10-17 13:05:00 UTC
        // 20743 days * 86400 + 13 * 3600 + 5 * 60 = 1792195200 + 47100 = 1792242300
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1792242300);
        let filename = generate_log_filename(time);
        assert_eq!(filename, "packet_sequence-20261017_130500.log");
    }

    #[test]
    fn test_packet_sequence_recorder_and_format_log() {
        let recorder = PacketSequenceRecorder::new();
        assert!(recorder.is_empty());

        let seq1 = recorder.record(
            1,
            ServerType::Login,
            PacketDirection::ClientToServer,
            PacketOpcode(0x0064),
            55,
            "[IMPLEMENTED] CA_LOGIN".to_string(),
        );
        assert_eq!(seq1, 1);

        let seq2 = recorder.record(
            1,
            ServerType::Login,
            PacketDirection::ServerToClient,
            PacketOpcode(0x0069),
            10,
            "[IMPLEMENTED] AC_ACCEPT_LOGIN".to_string(),
        );
        assert_eq!(seq2, 2);

        let seq3 = recorder.record(
            2,
            ServerType::Char,
            PacketDirection::ClientToServer,
            PacketOpcode(0x0065),
            20,
            "[IMPLEMENTED] CH_ENTER".to_string(),
        );
        assert_eq!(seq3, 3);

        let seq4 = recorder.record(
            3,
            ServerType::Map,
            PacketDirection::ClientToServer,
            PacketOpcode(0x0072),
            19,
            "[IMPLEMENTED] CZ_ENTER".to_string(),
        );
        assert_eq!(seq4, 4);

        assert_eq!(recorder.len(), 4);

        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1792242300);
        let log = recorder.format_log(now);

        assert!(log.contains("Target PACKETVER:"));
        assert!(log.contains("2026-10-17 13:05:00"));
        assert!(log.contains("Total Packets Recorded: 4"));
        assert!(log.contains("Login Server: 2 packets"));
        assert!(log.contains("Char Server:  1 packets"));
        assert!(log.contains("Map Server:   1 packets"));
        assert!(log.contains("0x0064 (CA_LOGIN)"));
        assert!(log.contains("0x0069"));
        assert!(log.contains("AC_ACCEPT_LOGIN"));
        assert!(log.contains("0x0065 (CH_ENTER)"));
        assert!(log.contains("0x0072"));
    }

    #[test]
    fn test_packet_sequence_recorder_write_to_file() {
        let recorder = PacketSequenceRecorder::new();
        recorder.record(
            1,
            ServerType::Login,
            PacketDirection::ClientToServer,
            PacketOpcode(0x0064),
            55,
            "[IMPLEMENTED] CA_LOGIN".to_string(),
        );

        let mut path = std::env::temp_dir();
        path.push(format!("test_proxy_log_{}.log", std::process::id()));

        let now = std::time::SystemTime::now();
        let write_res = recorder.write_to_file(&path, now);
        assert!(write_res.is_ok());
        assert_eq!(write_res.unwrap(), 1);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Global Sequence Log"));
        assert!(content.contains("CA_LOGIN"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_session_sequence_tracker_with_global() {
        let recorder = Arc::new(PacketSequenceRecorder::new());
        let tracker = SessionSequenceTracker::with_global(ServerType::Login, 10, recorder.clone());

        assert_eq!(tracker.packet_count(), 0);

        let entry = PacketSequenceEntry {
            seq: tracker.next_seq(),
            direction: PacketDirection::ClientToServer,
            opcode: PacketOpcode(0x0064),
            length: 55,
            detail: "[IMPLEMENTED] CA_LOGIN".to_string(),
        };
        tracker.record(entry);

        assert_eq!(tracker.packet_count(), 1);
        assert_eq!(recorder.len(), 1);

        let global_entries = recorder.entries();
        assert_eq!(global_entries.len(), 1);
        assert_eq!(global_entries[0].global_seq, 1);
        assert_eq!(global_entries[0].session_id, 10);
        assert_eq!(global_entries[0].server_type, ServerType::Login);
        assert_eq!(global_entries[0].direction, PacketDirection::ClientToServer);
        assert_eq!(global_entries[0].opcode, PacketOpcode(0x0064));
    }
}
