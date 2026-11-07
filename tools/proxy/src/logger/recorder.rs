use std::{
    collections::VecDeque,
    fs, io,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use ro_core::{ServerType, TARGET_PACKETVER};
use ro_packets::PacketOpcode;

use crate::logger::{
    resync::DEFAULT_PREVIEW_BYTES,
    sequence_dto::{GlobalSequenceEntry, PacketDirection},
};

pub const DEFAULT_MAX_RECORDED_PACKETS: usize = 50_000;

/// Thread-safe recorder capturing packet flow across all proxy sessions and servers.
#[derive(Debug)]
pub struct PacketSequenceRecorder {
    max_capacity: usize,
    seq_counter: AtomicUsize,
    entries: Mutex<VecDeque<GlobalSequenceEntry>>,
    preview_limit: usize,
}

impl Default for PacketSequenceRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketSequenceRecorder {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_MAX_RECORDED_PACKETS)
    }

    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            max_capacity: max_capacity.max(1),
            seq_counter: AtomicUsize::new(1),
            entries: Mutex::new(VecDeque::new()),
            preview_limit: DEFAULT_PREVIEW_BYTES,
        }
    }

    /// When `full` is set, log lines show every payload byte instead of the first 32.
    pub fn with_full_payload(mut self, full: bool) -> Self {
        self.preview_limit = if full {
            usize::MAX
        } else {
            DEFAULT_PREVIEW_BYTES
        };
        self
    }

    /// The most payload bytes a log line shows.
    pub fn preview_limit(&self) -> usize {
        self.preview_limit
    }

    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    pub fn total_recorded(&self) -> usize {
        self.seq_counter.load(Ordering::SeqCst).saturating_sub(1)
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
        let global_seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= self.max_capacity {
                entries.pop_front();
            }
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
            entries.push_back(entry);
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
        let mut list: Vec<GlobalSequenceEntry> = self
            .entries
            .lock()
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default();
        list.sort_by_key(|e| e.global_seq);
        list
    }

    pub fn format_log(&self, generated_at: SystemTime) -> String {
        let entries = self.entries();
        self.format_entries(&entries, generated_at)
    }

    pub fn format_entries(
        &self,
        entries: &[GlobalSequenceEntry],
        generated_at: SystemTime,
    ) -> String {
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

        let total = self.total_recorded();
        let total_str = if total > entries.len() {
            format!("{} (Retained: {})", total, entries.len())
        } else {
            format!("{}", entries.len())
        };

        let mut output = String::new();
        output.push_str(
            "================================================================================\n",
        );
        output.push_str("Ragnarok Online Packet Proxy - Global Sequence Log\n");
        output.push_str(&format!("Target PACKETVER: {}\n", TARGET_PACKETVER));
        output.push_str(&format!("Generated: {}\n", timestamp_str));
        output.push_str(&format!("Total Packets Recorded: {}\n", total_str));
        output.push_str(
            "================================================================================\n\n",
        );

        output.push_str("Chronological Packet Sequence:\n");
        if entries.is_empty() {
            output.push_str("No packets recorded during this session.\n\n");
        } else {
            output.push_str(&format!(
                "{:<6} {:<23} {:<7} {:<8} {:<18} {:<24} {:<6} {}\n",
                "#",
                "Timestamp (UTC)",
                "Server",
                "Session",
                "Direction",
                "Opcode",
                "Len",
                "Details"
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

        output.push_str(
            "================================================================================\n",
        );
        output.push_str("Summary by Server:\n");
        output.push_str(&format!("  Login Server: {} packets\n", login_count));
        output.push_str(&format!("  Char Server:  {} packets\n", char_count));
        output.push_str(&format!("  Map Server:   {} packets\n", map_count));
        output.push_str(&format!("  Total:        {} packets\n", total_str));
        output.push_str(
            "================================================================================\n",
        );

        output
    }

    pub fn write_to_file(
        &self,
        path: impl AsRef<Path>,
        generated_at: SystemTime,
    ) -> io::Result<usize> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let entries = self.entries();
        let count = entries.len();
        let content = self.format_entries(&entries, generated_at);
        fs::write(p, content)?;
        Ok(count)
    }
}

/// Converts a `SystemTime` into civil date components (year, month, day, hour, minute, second, millis).
pub fn civil_from_timestamp(time: SystemTime) -> (i64, u32, u32, u32, u32, u32, u32) {
    let (secs, subsec_millis) = match time.duration_since(UNIX_EPOCH) {
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

    (
        year,
        m as u32,
        d as u32,
        hour,
        minute,
        second,
        subsec_millis,
    )
}

/// Generates a standardized log filename: `packet_sequence-%Y%m%d_%H%M%S.log`.
pub fn generate_log_filename(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second, _) = civil_from_timestamp(time);
    format!(
        "packet_sequence-{:04}{:02}{:02}_{:02}{:02}{:02}.log",
        year, month, day, hour, minute, second
    )
}

#[cfg(test)]
mod tests;
