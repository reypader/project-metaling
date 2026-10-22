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

use ro_core::{http::ServerType, TARGET_PACKETVER};
use ro_packets::PacketOpcode;

use crate::logger::session::PacketDirection;

pub const DEFAULT_MAX_RECORDED_PACKETS: usize = 50_000;

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
#[derive(Debug)]
pub struct PacketSequenceRecorder {
    max_capacity: usize,
    seq_counter: AtomicUsize,
    entries: Mutex<VecDeque<GlobalSequenceEntry>>,
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
        }
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
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_generate_log_filename() {
        // 2026-10-17 13:05:00 UTC
        // 20743 days * 86400 + 13 * 3600 + 5 * 60 = 1792195200 + 47100 = 1792242300
        let time = UNIX_EPOCH + Duration::from_secs(1792242300);
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

        let now = UNIX_EPOCH + Duration::from_secs(1792242300);
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

        let now = SystemTime::now();
        let write_res = recorder.write_to_file(&path, now);
        assert!(write_res.is_ok());
        assert_eq!(write_res.unwrap(), 1);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Global Sequence Log"));
        assert!(content.contains("CA_LOGIN"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_packet_sequence_recorder_bounded_capacity() {
        let recorder = PacketSequenceRecorder::with_capacity(3);
        assert_eq!(recorder.max_capacity(), 3);

        for i in 1..=5 {
            let seq = recorder.record(
                1,
                ServerType::Login,
                PacketDirection::ClientToServer,
                PacketOpcode(0x0064),
                55,
                format!("Packet {}", i),
            );
            assert_eq!(seq, i);
        }

        assert_eq!(recorder.len(), 3);
        assert_eq!(recorder.total_recorded(), 5);

        let entries = recorder.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].global_seq, 3);
        assert_eq!(entries[0].detail, "Packet 3");
        assert_eq!(entries[1].global_seq, 4);
        assert_eq!(entries[1].detail, "Packet 4");
        assert_eq!(entries[2].global_seq, 5);
        assert_eq!(entries[2].detail, "Packet 5");
    }
}
