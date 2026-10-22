//! Engine-foundation map-server scenarios (spec §2): BIN-01..09, BIN-SRV-01..04/06, BIN-CFG-01.
//! In-memory repo + injected 200x200 grid; characters `n` are account 200000n / char 15000n.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;
use map_server::load_walkability;
use ro_core::{CharId, MapPosition, RawPacket};

const SHORT: Duration = Duration::from_millis(300);
const LONG: Duration = Duration::from_secs(4);

fn entries_about(frames: &[RawPacket], n: u32) -> usize {
    frames
        .iter()
        .filter(|f| is_entry(f) && entry_aid(f) == account(n))
        .count()
}

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

// ---------- BIN-09 ----------

fn act_type(frame: &RawPacket) -> u8 {
    frame_u8(frame, 29)
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

// ---------- BIN-SRV-01 ----------

#[tokio::test]
async fn bin_srv_01_tick_loop_advances_walks_without_client_traffic() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(103, 103)).await;
    // No further packets from the client: only the 20 ms tick moves the character.
    assert!(server.wait_position(1, (103, 103), LONG).await);
}

// ---------- BIN-SRV-06 ----------

#[tokio::test]
async fn bin_srv_06_transfer_reservation_sets_entry_position() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    assert!(server.service.accept_transfer(
        CharId(char_id(1)),
        "geffen.gat".to_string(),
        Some(MapPosition {
            x: 50,
            y: 60,
            dir: 2
        }),
    ));
    assert!(!server
        .service
        .accept_transfer(CharId(char_id(1)), "../evil.gat".to_string(), None));
    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(1)).await;
    let burst = a.read_exact_bytes(23).await;
    let pos: [u8; 3] = burst[16..19].try_into().unwrap();
    assert_eq!(ro_packets::unpack_pos_dir(&pos), (50, 60, 2));
    let at = server
        .service
        .character_position("geffen.gat", CharId(char_id(1)))
        .await;
    assert_eq!(at.map(|p| (p.x, p.y)), Some((50, 60)));
}

// ---------- BIN-CFG-01 ----------

fn gat_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"GRAT\x01\x02".to_vec();
    b.extend_from_slice(&width.to_le_bytes());
    b.extend_from_slice(&height.to_le_bytes());
    for _ in 0..width * height {
        b.extend_from_slice(&[0u8; 20]);
    }
    b
}

#[test]
fn bin_cfg_01_map_data_dir_gat_is_loaded() {
    let dir = std::env::temp_dir().join(format!("metaling-mapdata-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("tiny.gat"), gat_bytes(7, 5)).expect("write gat");
    let grid = load_walkability(Some(&dir), "tiny.gat");
    assert_eq!((grid.width(), grid.height()), (7, 5));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn bin_cfg_01_missing_or_bad_gat_falls_back_to_512() {
    let dir = std::env::temp_dir().join(format!("metaling-mapdata-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("broken.gat"), b"nope").expect("write");
    for (d, map) in [
        (Some(dir.as_path()), "missing.gat"),
        (Some(dir.as_path()), "broken.gat"),
        (None, "prontera.gat"),
    ] {
        let grid = load_walkability(d, map);
        assert_eq!((grid.width(), grid.height()), (512, 512), "{map}");
        assert!(grid.is_walkable(100, 100));
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn bin_cfg_01_map_data_dir_from_env() {
    let cfg = map_server::MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
        (key == "MAP_DATA_DIR").then(|| "/data/maps".to_string())
    });
    assert_eq!(
        cfg.map_data_dir,
        Some(std::path::PathBuf::from("/data/maps"))
    );
    let none = map_server::MapConfig::from_args_and_env(Vec::<String>::new(), |_| None);
    assert_eq!(none.map_data_dir, None);
}

// ---------- M6 review follow-ups ----------

fn burst_tick(burst: &[u8]) -> u32 {
    u32::from_le_bytes(burst[12..16].try_into().unwrap())
}

fn burst_cell(burst: &[u8]) -> (u16, u16) {
    let pos: [u8; 3] = burst[16..19].try_into().unwrap();
    let (x, y, _) = ro_packets::unpack_pos_dir(&pos);
    (x, y)
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
async fn bin_srv_06_rejected_entry_keeps_the_owners_reservation() {
    let server = TestServer::start(&[(1, 100, 100), (2, 100, 100)], &[]).await;
    // Map-only reservation: nothing is written to the repo, so only the reservation says geffen.
    assert!(server
        .service
        .accept_transfer(CharId(char_id(1)), "geffen.gat".to_string(), None));
    let mut thief = Client::connect(server.addr).await;
    thief.send(&cz_enter_raw(account(2), char_id(1))).await;
    let (bytes, eof) = thief.read_until_eof(LONG).await;
    assert!(eof && bytes.is_empty());
    let mut owner = Client::connect(server.addr).await;
    owner.send(&cz_enter(1)).await;
    owner.read_exact_bytes(23).await;
    let on_geffen = server
        .service
        .character_position("geffen.gat", CharId(char_id(1)))
        .await;
    assert_eq!(
        on_geffen.map(|p| (p.x, p.y)),
        Some((100, 100)),
        "reservation still applies"
    );
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

#[tokio::test]
async fn bin_srv_04_fast_reentry_sees_the_position_being_persisted() {
    let repo =
        InMemoryRepo::seeded(&[(1, 100, 100)]).with_persist_delay(Duration::from_millis(800));
    let server = TestServer::start_with(repo, &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(102, 100)).await;
    assert!(server.wait_position(1, (102, 100), LONG).await);
    a.send(&[0xb2, 0x00, 0x01]).await;
    assert!(a.wait_opcode(0x00b3, LONG).await.is_some());
    a.close().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut again = Client::connect(server.addr).await;
    again.send(&cz_enter(1)).await;
    let burst = again.read_exact_bytes(23).await;
    assert_eq!(
        burst_cell(&burst),
        (102, 100),
        "spawns where it left, not at the stale DB cell"
    );
}

#[tokio::test]
async fn bin_srv_01_all_maps_share_one_server_clock() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(server.service.accept_transfer(
        CharId(char_id(1)),
        "geffen.gat".to_string(),
        Some(MapPosition {
            x: 50,
            y: 60,
            dir: 0
        }),
    ));
    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(1)).await;
    let burst = a.read_exact_bytes(23).await;
    assert!(
        burst_tick(&burst) >= 400,
        "a newly started map uses the process clock: {}",
        burst_tick(&burst)
    );
}

#[tokio::test]
async fn bin_srv_04_restart_keeps_the_socket_open_for_the_client_to_close() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&[0xb2, 0x00, 0x01]).await;
    assert!(a.wait_opcode(0x00b3, LONG).await.is_some());
    let (_, eof) = a.read_until_eof(Duration::from_millis(400)).await;
    assert!(
        !eof,
        "server must not close the socket after ZC_RESTART_ACK"
    );
    assert!(
        server.aoi(100, 100).await.is_empty(),
        "character left the map"
    );
}

#[tokio::test]
async fn bin_srv_01_command_flood_does_not_stall_the_tick() {
    let server = TestServer::start(&[(1, 100, 100), (2, 150, 150)], &[]).await;
    let mut walker = server.join(1).await;
    let mut flooder = server.join(2).await;
    walker.send(&cz_request_move(105, 100)).await;
    let flood: Vec<u8> = (0..20_000u32).flat_map(cz_request_time).collect();
    let send = tokio::spawn(async move {
        for chunk in flood.chunks(6 * 500) {
            flooder.send(chunk).await;
        }
        flooder
    });
    assert!(
        server.wait_position(1, (105, 100), LONG).await,
        "walk completes while another client floods the actor"
    );
    drop(send.await);
}
