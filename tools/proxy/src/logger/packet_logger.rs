use std::{
    cmp::min,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::{Buf, BytesMut};
use ro_core::{ServerType, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, is_implemented_outbound_opcode, DecodedFrame, PacketDecoder, Phase0IntentParser,
    RathenaPacketLengthTable,
};
use ro_packets::{
    PacketCodecError, PacketIntentParser, PacketLength, PacketLengthTable, PacketOpcode, RawPacket,
};
use tokio_util::codec::Decoder;
use tracing::{debug, warn};

use crate::logger::{
    resync::{find_resync_offset, format_hex_preview, format_hex_preview_limited},
    sequence_dto::{PacketDirection, PacketSequenceEntry},
    session::SessionSequenceTracker,
};

pub struct PacketLogger {
    decoder: PacketDecoder<RathenaPacketLengthTable>,
    parser: Phase0IntentParser,
    server_type: ServerType,
    session_id: usize,
    direction: PacketDirection,
    tracker: Arc<SessionSequenceTracker>,
    buffer: BytesMut,
    /// Consecutive reads that ended with the same frame still incomplete.
    pending_reads: usize,
    /// Whether that incomplete frame already has its `[PENDING FRAME]` note.
    pending_noted: bool,
    /// When the head frame was first seen incomplete.
    pending_since: Option<Instant>,
    /// How long a frame may stay incomplete before the next read drops it.
    stall_timeout: Duration,
}

/// Reads a frame may stay incomplete before the logger writes a `[PENDING FRAME]` note.
const PENDING_NOTE_AFTER_READS: usize = 3;
/// Bytes of a pending frame's head shown in its note.
const PENDING_NOTE_HEAD_BYTES: usize = 64;
/// A frame still incomplete this long after it started is a misframe: the servers write each
/// packet whole, so a real frame's bytes arrive within milliseconds.
const DEFAULT_STALL_TIMEOUT: Duration = Duration::from_secs(1);

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
            pending_reads: 0,
            pending_noted: false,
            pending_since: None,
            stall_timeout: DEFAULT_STALL_TIMEOUT,
        }
    }

    /// Overrides how long a frame may stay incomplete (tests).
    pub fn with_stall_timeout(mut self, timeout: Duration) -> Self {
        self.stall_timeout = timeout;
        self
    }

    /// Feed incoming raw wire bytes into the logger's framing buffer and log all complete packets.
    /// Note: Bytes should already have been forwarded across the network socket.
    pub fn feed_and_log(&mut self, chunk: &[u8]) {
        if self
            .pending_since
            .is_some_and(|since| since.elapsed() >= self.stall_timeout)
        {
            self.drop_stalled_frame();
        }
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
                    self.clear_pending();
                    self.log_raw_packet(&raw_packet);
                }
                Ok(None) => {
                    // Frame incomplete, wait for more data from TCP stream
                    self.note_pending_frame();
                    break;
                }
                Err(err) => {
                    self.clear_pending();
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
                        let payload_preview =
                            format_hex_preview_limited(raw_payload, self.tracker.preview_limit());
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

    /// Records a proxy-side note in this logger's direction (see
    /// [`SessionSequenceTracker::record_note`]).
    pub fn record_note(&mut self, note: &str) {
        self.tracker.record_note(self.direction, note);
    }

    fn clear_pending(&mut self) {
        self.pending_reads = 0;
        self.pending_noted = false;
        self.pending_since = None;
    }

    /// Logs the buffered bytes of a frame that stayed incomplete past the stall timeout as one
    /// unframed entry with a note, and empties the buffer, so framing restarts at the read that
    /// comes next (a new server write, so most likely a packet start). Without this a misframe
    /// that reads as a large dynamic frame holds the log until that many bytes arrive.
    fn drop_stalled_frame(&mut self) {
        let waited = self
            .pending_since
            .map_or(Duration::ZERO, |since| since.elapsed());
        let opcode = PacketOpcode(u16::from_le_bytes([self.buffer[0], self.buffer[1]]));
        let note = format!(
            "[STALLED FRAME DROPPED] {} incomplete for {} ms; {} bytes logged as unframed, framing restarts at the next read",
            opcode,
            waited.as_millis(),
            self.buffer.len()
        );
        warn!(
            target: "proxy",
            "[{:?} Session #{}] [{}] {}",
            self.server_type, self.session_id, self.direction, note
        );
        self.record_note(&note);
        let seq = self.tracker.next_seq();
        let payload = format_hex_preview_limited(&self.buffer[2..], self.tracker.preview_limit());
        self.tracker.record(PacketSequenceEntry {
            seq,
            direction: self.direction,
            opcode,
            length: self.buffer.len(),
            detail: format!(
                "[UNFRAMED, {} bytes of a stalled frame] payload: {}",
                self.buffer.len(),
                payload
            ),
        });
        self.buffer.clear();
        self.clear_pending();
    }

    /// Counts a read that left the head frame incomplete. After a few, writes one note with the
    /// frame's expected length and head bytes, so a stalled log shows what it is waiting on.
    fn note_pending_frame(&mut self) {
        self.pending_reads += 1;
        self.pending_since.get_or_insert_with(Instant::now);
        if self.pending_noted || self.pending_reads < PENDING_NOTE_AFTER_READS {
            return;
        }
        self.pending_noted = true;
        let opcode = PacketOpcode(u16::from_le_bytes([self.buffer[0], self.buffer[1]]));
        let expected = match RathenaPacketLengthTable.packet_length(opcode, TARGET_PACKETVER) {
            Some(PacketLength::Fixed(n)) => n.to_string(),
            Some(PacketLength::Dynamic) if self.buffer.len() >= 4 => {
                u16::from_le_bytes([self.buffer[2], self.buffer[3]]).to_string()
            }
            _ => "an unknown number of".to_string(),
        };
        let head = format_hex_preview_limited(
            &self.buffer,
            PENDING_NOTE_HEAD_BYTES.max(self.tracker.preview_limit()),
        );
        let note = format!(
            "[PENDING FRAME] {} expects {} bytes, has {} after {} reads; head: {}",
            opcode,
            expected,
            self.buffer.len(),
            self.pending_reads,
            head
        );
        warn!(
            target: "proxy",
            "[{:?} Session #{}] [{}] {}",
            self.server_type, self.session_id, self.direction, note
        );
        self.record_note(&note);
    }

    fn record_unmapped_packet(&mut self, err: &PacketCodecError, consumed: usize) {
        let head_opcode = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
        let opcode = PacketOpcode(head_opcode);

        let raw_payload = if consumed > 2 {
            &self.buffer[2..consumed]
        } else {
            &[]
        };
        let payload_preview = format_hex_preview_limited(raw_payload, self.tracker.preview_limit());

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
        let payload_preview =
            format_hex_preview_limited(&raw.payload, self.tracker.preview_limit());

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
mod tests;
