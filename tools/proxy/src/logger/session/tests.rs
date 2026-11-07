use super::*;

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

#[test]
fn test_session_sequence_tracker_bounded_capacity() {
    let tracker = SessionSequenceTracker::with_history_limit(2);
    assert_eq!(tracker.max_history(), 2);

    for i in 1..=4 {
        tracker.record(PacketSequenceEntry {
            seq: tracker.next_seq(),
            direction: PacketDirection::ClientToServer,
            opcode: PacketOpcode(0x0064),
            length: 55,
            detail: format!("Session Packet {}", i),
        });
    }

    assert_eq!(tracker.packet_count(), 4);

    let flow = tracker.format_sequence_flow();
    assert!(!flow.contains("Session Packet 1"));
    assert!(!flow.contains("Session Packet 2"));
    assert!(flow.contains("Session Packet 3"));
    assert!(flow.contains("Session Packet 4"));
}
