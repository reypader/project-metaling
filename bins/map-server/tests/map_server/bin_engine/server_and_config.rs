//! The tick loop, transfers, re-entry and map-data config (BIN-SRV, BIN-CFG).

use super::*;

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
