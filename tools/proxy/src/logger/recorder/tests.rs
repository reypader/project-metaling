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
