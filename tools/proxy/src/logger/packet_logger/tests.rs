use super::*;
use crate::logger::recorder::PacketSequenceRecorder;

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
        0x7f, 0x00, 0x01, 0x02, 0x03, 0x04, // Real packet 2: ZC_NOTIFY_TIME (offset 3, len 6)
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

/// A ZC_ACK_REQNAMEALL frame (fixed 106) whose payload bytes count up from 0.
fn reqname_frame() -> Vec<u8> {
    let mut frame = vec![0x30, 0x0a];
    frame.extend(0..104u8);
    frame
}

#[test]
fn test_full_payload_logs_every_byte() {
    let recorder = Arc::new(PacketSequenceRecorder::new().with_full_payload(true));
    let tracker = Arc::new(SessionSequenceTracker::with_global(
        ServerType::Map,
        1,
        recorder.clone(),
    ));
    let mut logger =
        PacketLogger::new(ServerType::Map, 1, PacketDirection::ServerToClient, tracker);
    logger.feed_and_log(&reqname_frame());

    let entries = recorder.entries();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].detail.contains("(+"), "{}", entries[0].detail);
    assert!(
        entries[0].detail.contains("66 67]"),
        "{}",
        entries[0].detail
    );
}

#[test]
fn test_default_payload_is_cut() {
    let recorder = Arc::new(PacketSequenceRecorder::new());
    let tracker = Arc::new(SessionSequenceTracker::with_global(
        ServerType::Map,
        1,
        recorder.clone(),
    ));
    let mut logger =
        PacketLogger::new(ServerType::Map, 1, PacketDirection::ServerToClient, tracker);
    logger.feed_and_log(&reqname_frame());

    let entries = recorder.entries();
    assert!(
        entries[0].detail.contains("(+72 bytes)"),
        "{}",
        entries[0].detail
    );
}

#[test]
fn test_pending_frame_noted_once_after_three_reads() {
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = PacketLogger::new(
        ServerType::Map,
        1,
        PacketDirection::ServerToClient,
        tracker.clone(),
    );
    let frame = reqname_frame();

    logger.feed_and_log(&frame[..10]);
    logger.feed_and_log(&frame[10..20]);
    assert_eq!(tracker.packet_count(), 0, "no note before the third read");
    logger.feed_and_log(&frame[20..30]);
    assert_eq!(tracker.packet_count(), 1, "note after the third read");
    logger.feed_and_log(&frame[30..40]);
    assert_eq!(tracker.packet_count(), 1, "the note is written once");

    let flow = tracker.format_sequence_flow();
    assert!(flow.contains("[PENDING FRAME"), "{flow}");
    assert!(flow.contains("expects 106 bytes, has 30"), "{flow}");

    logger.feed_and_log(&frame[40..]);
    assert_eq!(tracker.packet_count(), 2, "the frame logs once complete");
    assert!(logger.buffer.is_empty());
}

/// A misframe that reads as a big dynamic frame (here `0x0b39` claiming 1000 bytes) would
/// hold the log until that many bytes arrive. Once it has been pending longer than the stall
/// timeout, the next read drops it (logged as unframed bytes with a note) and framing
/// restarts at that read.
#[test]
fn test_stalled_frame_is_dropped_at_the_next_read() {
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = PacketLogger::new(
        ServerType::Map,
        1,
        PacketDirection::ServerToClient,
        tracker.clone(),
    )
    .with_stall_timeout(std::time::Duration::ZERO);
    let mut bogus = vec![0x39, 0x0b];
    bogus.extend_from_slice(&1000u16.to_le_bytes());
    bogus.extend_from_slice(&[0xaa; 20]);
    logger.feed_and_log(&bogus);
    assert_eq!(tracker.packet_count(), 0, "still pending");

    logger.feed_and_log(&reqname_frame());
    assert!(logger.buffer.is_empty());
    let flow = tracker.format_sequence_flow();
    assert!(flow.contains("[STALLED FRAME DROPPED]"), "{flow}");
    assert!(flow.contains("24 bytes"), "{flow}");
    assert!(
        flow.contains("0x0a30"),
        "the next read frames again: {flow}"
    );
}

/// With the default timeout a frame split over quick reads still completes.
#[test]
fn test_split_frame_within_timeout_is_kept() {
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = PacketLogger::new(
        ServerType::Map,
        1,
        PacketDirection::ServerToClient,
        tracker.clone(),
    );
    let frame = reqname_frame();
    logger.feed_and_log(&frame[..50]);
    logger.feed_and_log(&frame[50..]);
    let flow = tracker.format_sequence_flow();
    assert!(!flow.contains("DROPPED"), "{flow}");
    assert!(flow.contains("0x0a30"), "{flow}");
}

/// One skill point as rAthena sends it (`equip_sequence_3.txt` lines 2535-2546): `0x010e`,
/// the `0x010f` list, `SP_SKILLPOINT`. Before `0x010e` had a row, each one misframed and a
/// run of them stalled the log.
#[test]
fn test_skill_up_burst_frames_cleanly() {
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = PacketLogger::new(
        ServerType::Map,
        1,
        PacketDirection::ServerToClient,
        tracker.clone(),
    );
    let mut burst = vec![
        0x0e, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];
    let mut list = vec![0x0f, 0x01, 41, 0x00];
    list.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
    list.extend_from_slice(&[0x00, 0x00]);
    let mut name = [0u8; 24];
    name[..8].copy_from_slice(b"NV_BASIC");
    list.extend_from_slice(&name);
    list.push(0x01);
    assert_eq!(list.len(), 41);
    burst.extend_from_slice(&list);
    burst.extend_from_slice(&[0xb0, 0x00, 0x0c, 0x00, 0x3b, 0x00, 0x00, 0x00]);
    for _ in 0..50 {
        logger.feed_and_log(&burst);
    }
    let flow = tracker.format_sequence_flow();
    assert!(!flow.contains("UNKNOWN"), "{flow}");
    assert_eq!(tracker.packet_count(), 150);
    assert!(logger.buffer.is_empty());
}

#[test]
fn test_record_note() {
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = PacketLogger::new(
        ServerType::Map,
        1,
        PacketDirection::ServerToClient,
        tracker.clone(),
    );
    logger.record_note("forward to client took 1500 ms");
    let flow = tracker.format_sequence_flow();
    assert!(
        flow.contains("[PROXY NOTE] forward to client took 1500 ms"),
        "{flow}"
    );
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
