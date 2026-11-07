use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use ro_core::ServerType;
use ro_packets::PacketOpcode;

use crate::logger::{
    recorder::PacketSequenceRecorder,
    resync::DEFAULT_PREVIEW_BYTES,
    sequence_dto::{PacketDirection, PacketSequenceEntry},
};

pub const DEFAULT_MAX_SESSION_HISTORY: usize = 10_000;

/// Tracks the chronological sequence of packets for an individual connection session.
#[derive(Debug)]
pub struct SessionSequenceTracker {
    seq_counter: AtomicUsize,
    packet_counter: AtomicUsize,
    max_history: usize,
    history: Mutex<VecDeque<PacketSequenceEntry>>,
    global_sink: Option<(ServerType, usize, Arc<PacketSequenceRecorder>)>,
}

impl Default for SessionSequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionSequenceTracker {
    pub fn new() -> Self {
        Self::with_history_limit(DEFAULT_MAX_SESSION_HISTORY)
    }

    pub fn with_history_limit(max_history: usize) -> Self {
        Self {
            seq_counter: AtomicUsize::new(1),
            packet_counter: AtomicUsize::new(0),
            max_history: max_history.max(1),
            history: Mutex::new(VecDeque::new()),
            global_sink: None,
        }
    }

    pub fn with_global(
        server_type: ServerType,
        session_id: usize,
        recorder: Arc<PacketSequenceRecorder>,
    ) -> Self {
        Self::with_global_and_limit(
            server_type,
            session_id,
            recorder,
            DEFAULT_MAX_SESSION_HISTORY,
        )
    }

    pub fn with_global_and_limit(
        server_type: ServerType,
        session_id: usize,
        recorder: Arc<PacketSequenceRecorder>,
        max_history: usize,
    ) -> Self {
        Self {
            seq_counter: AtomicUsize::new(1),
            packet_counter: AtomicUsize::new(0),
            max_history: max_history.max(1),
            history: Mutex::new(VecDeque::new()),
            global_sink: Some((server_type, session_id, recorder)),
        }
    }

    pub fn max_history(&self) -> usize {
        self.max_history
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
            if hist.len() >= self.max_history {
                hist.pop_front();
            }
            hist.push_back(entry);
        }
    }

    pub fn packet_count(&self) -> usize {
        self.packet_counter.load(Ordering::SeqCst)
    }

    /// The most payload bytes a log line shows (the global recorder's setting).
    pub fn preview_limit(&self) -> usize {
        match self.global_sink {
            Some((_, _, ref recorder)) => recorder.preview_limit(),
            None => DEFAULT_PREVIEW_BYTES,
        }
    }

    /// Records a proxy-side note (not a packet) in the sequence, e.g. a slow forward or a closed
    /// session. It shows as opcode `0x0000` with a `[PROXY NOTE]` detail.
    pub fn record_note(&self, direction: PacketDirection, note: &str) {
        self.record(PacketSequenceEntry {
            seq: self.next_seq(),
            direction,
            opcode: PacketOpcode(0),
            length: 0,
            detail: format!("[PROXY NOTE] {note}"),
        });
    }

    /// Formats the complete chronological sequence flow observed during this session.
    pub fn format_sequence_flow(&self) -> String {
        let hist: Vec<PacketSequenceEntry> = match self.history.lock() {
            Ok(guard) => guard.iter().cloned().collect(),
            Err(_) => return "Unable to format sequence history (lock poisoned)".to_string(),
        };

        if hist.is_empty() {
            return "No packets recorded in this session.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "Chronological Packet Sequence ({} packets):",
            hist.len()
        ));
        for entry in &hist {
            lines.push(format!(
                "  Seq #{:<3} [{}] {} (len={}) => {}",
                entry.seq, entry.direction, entry.opcode, entry.length, entry.detail
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests;
