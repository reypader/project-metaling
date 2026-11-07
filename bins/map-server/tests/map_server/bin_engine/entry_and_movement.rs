//! Entry, movement, visibility, direction, names and sitting over TCP (BIN-01..09).

use super::*;

// ---------- BIN-01 ----------

#[tokio::test]
async fn bin_01_entry_and_scene_initialization() {
    let server = TestServer::start(&[(1, 100, 100), (2, 103, 100)], &[]).await;
    let mut b = server.join(2).await;

    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(1)).await;
    let burst = a.read_exact_bytes(23).await;
    assert_eq!(&burst[0..2], &[0x83, 0x02]);
    assert_eq!(&burst[2..6], &account(1).to_le_bytes());
    assert_eq!(&burst[6..8], &[0x18, 0x0b]);
    assert_eq!(&burst[10..12], &[0xeb, 0x02]);
    a.send(&[0x7d, 0x00]).await;

    let par = a.wait_opcode(0x00b0, LONG).await.expect("ZC_PAR_CHANGE");
    assert_eq!(frame_len(&par), 8);
    let standing = a
        .wait_opcode(0x09ff, LONG)
        .await
        .expect("0x09ff for B already in view");
    assert_eq!(entry_aid(&standing), account(2));
    assert_eq!(entry_gid(&standing), char_id(2));
    assert_eq!(frame_len(&standing), 108);

    let spawned = b.wait_opcode(0x09fe, LONG).await.expect("0x09fe for A");
    assert_eq!(entry_aid(&spawned), account(1));
    assert_eq!(entry_gid(&spawned), char_id(1));
    assert_eq!(frame_len(&spawned), 107);
    let others = a.collect(SHORT).await;
    assert!(
        others
            .iter()
            .all(|f| !(is_entry(f) && entry_aid(f) == account(1))),
        "A gets no entry packet about itself"
    );
}

#[tokio::test]
async fn bin_01_truncated_enter_closes_without_bytes() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut c = Client::connect(server.addr).await;
    c.send(&cz_enter(1)[..10]).await;
    c.close_write().await;
    let (bytes, eof) = c.read_until_eof(Duration::from_millis(800)).await;
    assert!(eof, "server closes");
    assert!(bytes.is_empty(), "no bytes sent: {bytes:?}");
}

#[tokio::test]
async fn bin_01_unknown_character_closes_without_aid() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut c = Client::connect(server.addr).await;
    c.send(&cz_enter_raw(account(9), char_id(9))).await;
    let (bytes, eof) = c.read_until_eof(LONG).await;
    assert!(eof, "server closes the session");
    assert!(bytes.is_empty(), "no ZC_AID: {bytes:?}");
}

#[tokio::test]
async fn bin_01_character_of_another_account_is_rejected() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut c = Client::connect(server.addr).await;
    c.send(&cz_enter_raw(account(2), char_id(1))).await;
    let (bytes, eof) = c.read_until_eof(LONG).await;
    assert!(eof);
    assert!(bytes.is_empty(), "no ZC_AID: {bytes:?}");
}

#[tokio::test]
async fn bin_01_second_session_for_present_character_is_rejected() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut first = server.join(1).await;
    let mut second = Client::connect(server.addr).await;
    second.send(&cz_enter(1)).await;
    let (bytes, eof) = second.read_until_eof(LONG).await;
    assert!(eof, "duplicate session closed");
    assert!(bytes.is_empty(), "no ZC_AID: {bytes:?}");
    // The first session still works.
    first.send(&cz_request_time(1)).await;
    assert!(first.wait_opcode(0x007f, LONG).await.is_some());
}

// ---------- BIN-02 ----------

#[tokio::test]
async fn bin_02_click_to_move() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(104, 100)).await;
    let mv = a
        .wait_opcode(0x0087, LONG)
        .await
        .expect("ZC_NOTIFY_PLAYERMOVE");
    assert_eq!(frame_len(&mv), 12);
    assert_eq!(playermove(&mv), (100, 100, 104, 100, 8, 8));
    assert!(
        server
            .wait_position(1, (104, 100), Duration::from_millis(1_000))
            .await,
        "arrives within a tick after 600 ms"
    );
    assert!(a.collect(SHORT).await.iter().all(|f| f.opcode.0 != 0x0087));
}

#[tokio::test]
async fn bin_02_click_on_own_cell_resyncs() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(100, 100)).await;
    let mv = a.wait_opcode(0x0087, LONG).await.expect("resync 0x0087");
    assert_eq!(playermove(&mv), (100, 100, 100, 100, 8, 8));
    tokio::time::sleep(SHORT).await;
    assert_eq!(
        server.position(1).await.map(|p| (p.x, p.y)),
        Some((100, 100))
    );
}

#[tokio::test]
async fn bin_02_click_farther_than_17_steps_is_ignored() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(118, 100)).await;
    assert!(a.wait_opcode(0x0087, SHORT).await.is_none());
    assert_eq!(
        server.position(1).await.map(|p| (p.x, p.y)),
        Some((100, 100))
    );
}

// ---------- BIN-03 / BIN-SRV-03 ----------

#[tokio::test]
async fn bin_03_bin_srv_03_movement_multicast_to_aoi_only() {
    let server = TestServer::start(&[(1, 100, 100), (2, 103, 100), (3, 150, 150)], &[]).await;
    let mut a = server.join(1).await;
    let mut b = server.join(2).await;
    let mut c = server.join(3).await;
    a.collect(SHORT).await;
    b.collect(SHORT).await;
    c.collect(SHORT).await;

    a.send(&cz_request_move(100, 106)).await;
    assert!(a.wait_opcode(0x0087, LONG).await.is_some());
    let walk = b.wait_opcode(0x09fd, LONG).await.expect("B sees A walking");
    assert_eq!(entry_aid(&walk), account(1));
    assert_eq!(frame_len(&walk), 114);
    assert_eq!(
        entries_about(&a.collect(SHORT).await, 1),
        0,
        "no 0x09fd to self"
    );
    assert!(c.collect(SHORT).await.is_empty(), "C is out of range");
}

// ---------- BIN-04 ----------

#[tokio::test]
async fn bin_04_visibility_while_walking() {
    // A = 1, O = 2 (observer on A's start cell), D = 3.
    let server = TestServer::start(&[(1, 100, 100), (2, 100, 100), (3, 120, 100)], &[]).await;
    let mut a = server.join(1).await;
    let mut o = server.join(2).await;
    let mut d = server.join(3).await;
    a.collect(SHORT).await;
    o.collect(SHORT).await;
    d.collect(SHORT).await;

    a.send(&cz_request_move(117, 100)).await;
    let d_sees_a = d
        .wait_for(LONG, |f| f.opcode.0 == 0x09fd && entry_aid(f) == account(1))
        .await
        .expect("D gets 0x09fd for A");
    assert_eq!(entry_gid(&d_sees_a), char_id(1));
    let a_sees_d = a
        .wait_for(LONG, |f| f.opcode.0 == 0x09ff && entry_aid(f) == account(3))
        .await
        .expect("A gets 0x09ff for D");
    assert_eq!(entry_gid(&a_sees_d), char_id(3));

    let o_vanish = o
        .wait_for(LONG, |f| {
            f.opcode.0 == 0x0080 && frame_u32(f, 2) == account(1)
        })
        .await
        .expect("O gets 0x0080 for A");
    assert_eq!(frame_u8(&o_vanish, 6), 0);
    let a_vanish = a
        .wait_for(LONG, |f| {
            f.opcode.0 == 0x0080 && frame_u32(f, 2) == account(2)
        })
        .await
        .expect("A gets 0x0080 for O");
    assert_eq!(frame_u8(&a_vanish, 6), 0);

    assert!(server.wait_position(1, (117, 100), LONG).await);
    let rest_d = d.collect(SHORT).await;
    let rest_a = a.collect(SHORT).await;
    assert_eq!(
        entries_about(&rest_d, 1),
        0,
        "no duplicate entry for A at D"
    );
    assert_eq!(
        entries_about(&rest_a, 3),
        0,
        "no duplicate entry for D at A"
    );
}

// ---------- BIN-05 ----------

#[tokio::test]
async fn bin_05_wall_and_boundary_rejection() {
    let server = TestServer::start(&[(1, 100, 100)], &[(105, 100)]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(105, 100)).await;
    a.send(&cz_request_move(250, 100)).await;
    assert!(a.wait_opcode(0x0087, SHORT).await.is_none());
    assert_eq!(
        server.position(1).await.map(|p| (p.x, p.y)),
        Some((100, 100))
    );
    a.send(&cz_request_time(1)).await;
    assert!(a.wait_opcode(0x007f, LONG).await.is_some(), "still running");
}

// ---------- BIN-06 ----------

#[tokio::test]
async fn bin_06_direction_and_name() {
    let server = TestServer::start(&[(1, 100, 100), (2, 102, 100)], &[]).await;
    let mut a = server.join(1).await;
    let mut b = server.join(2).await;
    a.collect(SHORT).await;
    b.collect(SHORT).await;

    a.send(&cz_change_direction(2, 4)).await;
    let dir = b.wait_opcode(0x009c, LONG).await.expect("B gets 0x009c");
    assert_eq!(frame_len(&dir), 9);
    assert_eq!(frame_u32(&dir, 2), account(1));
    assert_eq!(frame_u16(&dir, 6), 2);
    assert_eq!(frame_u8(&dir, 8), 4);
    assert!(a.collect(SHORT).await.iter().all(|f| f.opcode.0 != 0x009c));

    b.send(&cz_reqname(account(1))).await;
    let name = b.wait_opcode(0x0a30, LONG).await.expect("B gets 0x0a30");
    assert_eq!(frame_len(&name), 106);
    assert_eq!(frame_u32(&name, 2), account(1));
    assert_eq!(&name.payload[4..9], b"Char1");
    assert_eq!(name.payload[9], 0);
}

#[tokio::test]
async fn bin_06_name_request_unknown_or_out_of_range_ignored() {
    let server = TestServer::start(&[(1, 100, 100), (2, 120, 100)], &[]).await;
    let _a = server.join(1).await;
    let mut b = server.join(2).await;
    b.send(&cz_reqname(account(1))).await;
    b.send(&cz_reqname(424_242)).await;
    assert!(b.wait_opcode(0x0a30, SHORT).await.is_none());
}

#[tokio::test]
async fn bin_06_out_of_range_direction_closes_sender() {
    let server = TestServer::start(&[(1, 100, 100), (2, 102, 100)], &[]).await;
    let mut a = server.join(1).await;
    let mut b = server.join(2).await;
    b.collect(SHORT).await;
    a.send(&cz_change_direction(0, 8)).await;
    let (_, eof) = a.read_until_eof(LONG).await;
    assert!(eof, "sender's session is closed");
    assert!(b.collect(SHORT).await.iter().all(|f| f.opcode.0 != 0x009c));
}

// ---------- BIN-07 / BIN-SRV-04 ----------

#[tokio::test]
async fn bin_07_bin_srv_04_disconnect_cleanup() {
    let server = TestServer::start(&[(1, 100, 100), (2, 103, 100)], &[]).await;
    let mut a = server.join(1).await;
    let mut b = server.join(2).await;
    a.send(&cz_request_move(102, 101)).await;
    assert!(server.wait_position(1, (102, 101), LONG).await);
    b.collect(SHORT).await;

    a.close().await;
    let vanish = b
        .wait_for(LONG, |f| {
            f.opcode.0 == 0x0080 && frame_u32(f, 2) == account(1)
        })
        .await
        .expect("B gets 0x0080 for A");
    assert_eq!(frame_len(&vanish), 7);
    assert_eq!(frame_u8(&vanish, 6), 2, "logged out");
    assert_eq!(server.aoi(100, 100).await, vec![CharId(char_id(2))]);

    let deadline = tokio::time::Instant::now() + LONG;
    loop {
        if let Some((map, pos)) = server.repo.stored(1) {
            if map == MAP && (pos.x, pos.y) == (102, 101) {
                break;
            }
        }
        assert!(tokio::time::Instant::now() < deadline, "position persisted");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// ---------- BIN-08 ----------

#[tokio::test]
async fn bin_08_clock_sync_uses_server_clock() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_time(50_000)).await;
    let t = a.wait_opcode(0x007f, LONG).await.expect("ZC_NOTIFY_TIME");
    assert_eq!(frame_len(&t), 6);
    let tick = frame_u32(&t, 2);
    assert_ne!(tick, 50_000, "server clock, not an echo");
    assert!(tick > 0);
}

#[tokio::test]
async fn bin_08_bin_srv_02_gameplay_before_enter_closes() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut c = Client::connect(server.addr).await;
    c.send(&cz_request_time(50_000)).await;
    let (bytes, eof) = c.read_until_eof(LONG).await;
    assert!(eof);
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn bin_09_sit_and_stand() {
    let server = TestServer::start(&[(1, 100, 100), (2, 102, 100)], &[]).await;
    let mut a = server.join(1).await;
    let mut b = server.join(2).await;
    a.collect(SHORT).await;
    b.collect(SHORT).await;

    a.send(&cz_request_act(2)).await;
    for client in [&mut a, &mut b] {
        let sit = client.wait_opcode(0x08c8, LONG).await.expect("sit act");
        assert_eq!(frame_len(&sit), 34);
        assert_eq!(frame_u32(&sit, 2), account(1));
        assert_eq!(act_type(&sit), 2);
    }
    a.send(&cz_request_move(103, 100)).await;
    assert!(
        a.wait_opcode(0x0087, SHORT).await.is_none(),
        "no move while sitting"
    );

    a.send(&cz_request_act(3)).await;
    for client in [&mut a, &mut b] {
        let stand = client.wait_opcode(0x08c8, LONG).await.expect("stand act");
        assert_eq!(frame_u32(&stand, 2), account(1));
        assert_eq!(act_type(&stand), 3);
    }
}

#[tokio::test]
async fn bin_09_sit_while_walking_stops_first() {
    let server = TestServer::start(&[(1, 100, 100), (2, 102, 102)], &[]).await;
    let mut a = server.join(1).await;
    let mut b = server.join(2).await;
    a.send(&cz_request_move(110, 100)).await;
    assert!(a.wait_opcode(0x0087, LONG).await.is_some());
    tokio::time::sleep(Duration::from_millis(200)).await;
    b.collect(Duration::from_millis(10)).await;
    a.send(&cz_request_act(2)).await;
    for client in [&mut a, &mut b] {
        let frames = client.collect(Duration::from_millis(300)).await;
        let stop = frames
            .iter()
            .position(|f| f.opcode.0 == 0x0088)
            .expect("ZC_STOPMOVE");
        let sit = frames
            .iter()
            .position(|f| f.opcode.0 == 0x08c8)
            .expect("sit act");
        assert!(stop < sit, "0x0088 comes before the sit");
        assert_eq!(frame_len(&frames[stop]), 10);
        assert_eq!(frame_u32(&frames[stop], 2), account(1));
        assert_eq!(act_type(&frames[sit]), 2);
    }
    let stopped_at = server.position(1).await.expect("present");
    tokio::time::sleep(SHORT).await;
    assert_eq!(server.position(1).await, Some(stopped_at), "walk stopped");
}

#[tokio::test]
async fn bin_01_out_of_bounds_stored_position_falls_back_instead_of_locking_out() {
    let server = TestServer::start(&[(1, 250, 10)], &[]).await;
    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(1)).await;
    let burst = a.read_exact_bytes(23).await;
    assert_eq!(&burst[0..2], &[0x83, 0x02]);
    let (x, y) = burst_cell(&burst);
    assert!(x < 200 && y < 200, "fallback cell is on the map: {x},{y}");
}

#[tokio::test]
async fn bin_01_character_cannot_be_live_on_two_maps() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let _a = server.join(1).await;
    assert!(server
        .service
        .accept_transfer(CharId(char_id(1)), "geffen.gat".to_string(), None));
    let mut second = Client::connect(server.addr).await;
    second.send(&cz_enter(1)).await;
    let (bytes, eof) = second.read_until_eof(LONG).await;
    assert!(eof && bytes.is_empty(), "second copy rejected: {bytes:?}");
    assert_eq!(
        server
            .service
            .character_position("geffen.gat", CharId(char_id(1)))
            .await,
        None
    );
}

#[tokio::test]
async fn bin_01_crowded_entry_does_not_evict_the_newcomer() {
    // 70 characters in one spot: the newcomer gets 69 entries in one tick.
    let chars: Vec<(u32, u16, u16)> = (1..=70).map(|n| (n, 100, 100)).collect();
    let server = TestServer::start(&chars, &[]).await;
    let mut crowd = Vec::new();
    for n in 1..=69 {
        let mut c = Client::connect(server.addr).await;
        c.send(&cz_enter(n)).await;
        c.read_exact_bytes(23).await;
        c.send(&[0x7d, 0x00]).await;
        crowd.push(c);
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut newcomer = Client::connect(server.addr).await;
    newcomer.send(&cz_enter(70)).await;
    newcomer.read_exact_bytes(23).await;
    newcomer.send(&[0x7d, 0x00]).await;
    let mut seen = std::collections::HashSet::new();
    for f in newcomer.collect(Duration::from_millis(300)).await {
        if is_entry(&f) {
            seen.insert(entry_aid(&f));
        }
    }
    newcomer.send(&cz_request_time(1)).await;
    assert!(
        newcomer.wait_opcode(0x007f, LONG).await.is_some(),
        "still connected"
    );
    assert_eq!(seen.len(), 69, "every unit in view arrives");
    drop(crowd);
}
