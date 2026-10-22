use std::{cmp::min, sync::Arc};

use bytes::{Buf, BytesMut};
use ro_core::{http::ServerType, TARGET_PACKETVER};
use ro_packets::{
    incoming::{decode_frame, DecodedFrame},
    is_implemented_outbound_opcode, PacketDecoder, Phase0IntentParser, RathenaPacketLengthTable,
};
use ro_packets::{
    PacketCodecError, PacketIntentParser, PacketLength, PacketLengthTable, PacketOpcode, RawPacket,
};
use tokio_util::codec::Decoder;
use tracing::{debug, warn};

use crate::logger::{
    resync::{find_resync_offset, format_hex_preview},
    session::{PacketDirection, PacketSequenceEntry, SessionSequenceTracker},
};

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
                    let advance_bytes = find_resync_offset(
                        &self.buffer,
                        &RathenaPacketLengthTable,
                        TARGET_PACKETVER,
                    );
                    let consumed_bytes = min(self.buffer.len(), advance_bytes);
                    self.record_unmapped_packet(&err, consumed_bytes);
                }
            }
        }
    }

    /// Flushes any remaining bytes in the decode buffer when the connection terminates.
    /// Trailing complete or partial frames are logged so unparsed wire bytes are recorded.
    pub fn flush(&mut self) {
        while !self.buffer.is_empty() {
            if self.buffer.len() >= 2 {
                match self.decoder.decode(&mut self.buffer) {
                    Ok(Some(raw_packet)) => {
                        self.log_raw_packet(&raw_packet);
                    }
                    Ok(None) => {
                        let head_opcode = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
                        let opcode = PacketOpcode(head_opcode);
                        let consumed_bytes = self.buffer.len();
                        let raw_payload = if consumed_bytes > 2 {
                            &self.buffer[2..consumed_bytes]
                        } else {
                            &[]
                        };
                        let payload_preview = format_hex_preview(raw_payload);
                        let seq = self.tracker.next_seq();

                        debug!(
                            target: "proxy",
                            "[{:?} Session #{}] [Seq #{:<3}] [{}] {} len={} [TRAILING PARTIAL FRAME on EOF], payload: {}",
                            self.server_type,
                            self.session_id,
                            seq,
                            self.direction,
                            opcode,
                            consumed_bytes,
                            payload_preview
                        );

                        self.tracker.record(PacketSequenceEntry {
                            seq,
                            direction: self.direction,
                            opcode,
                            length: consumed_bytes,
                            detail: format!(
                                "[TRAILING PARTIAL FRAME on EOF, {} bytes] payload: {}",
                                consumed_bytes, payload_preview
                            ),
                        });

                        self.buffer.clear();
                    }
                    Err(err) => {
                        let advance_bytes = find_resync_offset(
                            &self.buffer,
                            &RathenaPacketLengthTable,
                            TARGET_PACKETVER,
                        );
                        let consumed_bytes = min(self.buffer.len(), advance_bytes);
                        self.record_unmapped_packet(&err, consumed_bytes);
                    }
                }
            } else {
                let single_byte = self.buffer[0];
                let seq = self.tracker.next_seq();
                let payload_preview = format_hex_preview(&[single_byte]);

                debug!(
                    target: "proxy",
                    "[{:?} Session #{}] [Seq #{:<3}] [{}] Trailing single byte 0x{:02x} on EOF",
                    self.server_type,
                    self.session_id,
                    seq,
                    self.direction,
                    single_byte
                );

                self.tracker.record(PacketSequenceEntry {
                    seq,
                    direction: self.direction,
                    opcode: PacketOpcode(single_byte as u16),
                    length: 1,
                    detail: format!(
                        "[TRAILING BYTE on EOF, 1 byte] payload: {}",
                        payload_preview
                    ),
                });

                self.buffer.clear();
            }
        }
    }

    fn record_unmapped_packet(&mut self, err: &PacketCodecError, consumed: usize) {
        let head_opcode = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
        let opcode = PacketOpcode(head_opcode);

        let raw_payload = if consumed > 2 {
            &self.buffer[2..consumed]
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
            consumed,
            err,
            consumed,
            dyn_hint,
            payload_preview
        );

        self.tracker.record(PacketSequenceEntry {
            seq,
            direction: self.direction,
            opcode,
            length: consumed,
            detail: format!(
                "[UNKNOWN / UNMAPPED in table.rs, {} bytes] payload: {}",
                consumed, payload_preview
            ),
        });

        self.buffer.advance(consumed);
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
                Ok(frame) if !matches!(frame, DecodedFrame::Unhandled { .. }) => {
                    (true, format!("Decoded: {:?}", frame))
                }
                _ => match self.parser.parse_intent(raw, TARGET_PACKETVER) {
                    Ok(Some(intent)) => (true, format!("Decoded Intent: {:?}", intent)),
                    _ => (false, format!("payload: {}", payload_preview)),
                },
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

impl Drop for PacketLogger {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_packet_logger_recovers_at_odd_byte_offset() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = PacketLogger::new(
            ServerType::Map,
            1,
            PacketDirection::ClientToServer,
            tracker.clone(),
        );

        let buffer = [
            0xAA, // 1 stray byte at offset 0
            0x7d, 0x00, // Real packet 1: CZ_NOTIFY_ACTORINIT (offset 1, len 2)
            0x7f, 0x00, 0x01, 0x02, 0x03,
            0x04, // Real packet 2: ZC_NOTIFY_TIME (offset 3, len 6)
        ];

        logger.feed_and_log(&buffer);
        assert!(logger.buffer.is_empty());

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("Seq #1"));
        assert!(flow.contains("0x7daa"));
        assert!(flow.contains("[UNKNOWN / UNMAPPED in table.rs, 1 bytes]"));
        assert!(flow.contains("Seq #2"));
        assert!(flow.contains("0x007d (CZ_NOTIFY_ACTORINIT)"));
        assert!(flow.contains("Seq #3"));
        assert!(flow.contains("0x007f (ZC_NOTIFY_TIME)"));
    }

    #[test]
    fn test_packet_logger_flush_on_connection_close() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = PacketLogger::new(
            ServerType::Login,
            1,
            PacketDirection::ClientToServer,
            tracker.clone(),
        );

        // Feed partial CA_LOGIN frame (10 bytes out of 55 bytes)
        let partial_frame = [0x64, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        logger.feed_and_log(&partial_frame);
        assert_eq!(logger.buffer.len(), 10);
        assert_eq!(tracker.packet_count(), 0);

        logger.flush();
        assert!(logger.buffer.is_empty());
        assert_eq!(tracker.packet_count(), 1);

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("Seq #1"));
        assert!(flow.contains("0x0064 (CA_LOGIN)"));
        assert!(flow.contains("[TRAILING PARTIAL FRAME on EOF, 10 bytes]"));
    }

    #[test]
    fn test_packet_logger_drop_flushes_buffer() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        {
            let mut logger = PacketLogger::new(
                ServerType::Login,
                1,
                PacketDirection::ClientToServer,
                tracker.clone(),
            );
            // Feed partial bytes
            logger.feed_and_log(&[0x64, 0x00, 0xAA, 0xBB]);
            assert_eq!(tracker.packet_count(), 0);
            // logger drops here at end of scope
        }

        assert_eq!(tracker.packet_count(), 1);
        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("[TRAILING PARTIAL FRAME on EOF, 4 bytes]"));
    }

    #[test]
    fn test_packet_logger_flush_single_trailing_byte() {
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = PacketLogger::new(
            ServerType::Char,
            1,
            PacketDirection::ServerToClient,
            tracker.clone(),
        );

        logger.feed_and_log(&[0x42]);
        assert_eq!(logger.buffer.len(), 1);
        assert_eq!(tracker.packet_count(), 0);

        logger.flush();
        assert!(logger.buffer.is_empty());
        assert_eq!(tracker.packet_count(), 1);

        let flow = tracker.format_sequence_flow();
        assert!(flow.contains("[TRAILING BYTE on EOF, 1 byte]"));
    }
}
