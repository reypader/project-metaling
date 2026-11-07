use super::*;

fn map_logger(tracker: &Arc<SessionSequenceTracker>) -> PacketLogger {
    PacketLogger::new(
        ServerType::Map,
        1,
        PacketDirection::ServerToClient,
        tracker.clone(),
    )
}

#[tokio::test(start_paused = true)]
async fn stuck_forward_is_noted_while_still_blocked() {
    // The far end never reads, so writing more than the pipe holds blocks forever.
    let (mut writer, _never_read) = tokio::io::duplex(8);
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = map_logger(&tracker);

    let result = tokio::time::timeout(
        SLOW_FORWARD * 2,
        forward_chunk(&mut writer, &[0u8; 64], &mut logger, "client"),
    )
    .await;

    assert!(result.is_err(), "the write is still blocked");
    let flow = tracker.format_sequence_flow();
    assert!(
        flow.contains("forwarding 64 bytes to the client has been blocked for over 1000 ms"),
        "{flow}"
    );
}

#[tokio::test]
async fn quick_forward_writes_no_note() {
    let (mut writer, mut reader) = tokio::io::duplex(64);
    let tracker = Arc::new(SessionSequenceTracker::new());
    let mut logger = map_logger(&tracker);

    forward_chunk(&mut writer, &[1, 2, 3], &mut logger, "client")
        .await
        .expect("forward succeeds");

    let mut got = [0u8; 3];
    reader.read_exact(&mut got).await.expect("bytes arrive");
    assert_eq!(got, [1, 2, 3]);
    assert_eq!(tracker.packet_count(), 0);
}
