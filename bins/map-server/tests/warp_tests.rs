//! Warp map change in the map-server (spec `specs/warp-map-change/spec.md` BIN-WARP-01..06).
//! In-memory repo; every map is a 200x200 walkable grid unless a test loads `.gat` files.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use map_server::map_data_loader;
use ro_db::{MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_packets::RawPacket;

const MAP2: &str = "geffen.gat";
const SHORT: Duration = Duration::from_millis(400);
const LONG: Duration = Duration::from_secs(4);

const A: u32 = 1;
const O: u32 = 2;
const P: u32 = 3;

/// `ZC_PAR_CHANGE` var ids sent at entry.
const SP_WEIGHT: u16 = 24;
const SP_MAXWEIGHT: u16 = 25;

fn warp_to(name: &str, (x, y): (u16, u16), map: &str, dest: (u16, u16)) -> NpcSpawn {
    NpcSpawn {
        name: name.into(),
        unique_name: name.into(),
        x,
        y,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some((1, 1)),
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: map.into(),
            x: dest.0,
            y: dest.1,
            trigger_while_hidden: false,
        }),
        source: "test:1".into(),
    }
}

fn npcs(list: Vec<NpcSpawn>) -> MapNpcs {
    MapNpcs { npcs: list }
}

/// prontera has a warp at (110, 100) (area x 109..=111) to geffen (100, 100).
fn cross_map_loader() -> map_server::MapDataLoader {
    maps_loader(vec![(
        MAP,
        npcs(vec![warp_to("to_geffen", (110, 100), "geffen", (100, 100))]),
    )])
}

/// prontera has a warp at (110, 100) to prontera (150, 150).
fn same_map_loader() -> map_server::MapDataLoader {
    maps_loader(vec![(
        MAP,
        npcs(vec![warp_to("loop", (110, 100), "prontera", (150, 150))]),
    )])
}

fn cell(pos: Option<ro_core::MapPosition>) -> Option<(u16, u16)> {
    pos.map(|p| (p.x, p.y))
}

fn vanish_of(n: u32) -> impl Fn(&RawPacket) -> bool {
    move |f| f.opcode.0 == 0x0080 && frame_u32(f, 2) == account(n)
}

fn spawn_of(n: u32) -> impl Fn(&RawPacket) -> bool {
    move |f| f.opcode.0 == 0x09fe && entry_aid(f) == account(n)
}

/// Walks `a` east from (105, 100) into the warp at (110, 100) and returns the `0x0091`.
async fn walk_into_warp(a: &mut Client) -> (String, u16, u16) {
    a.send(&cz_request_move(112, 100)).await;
    let frame = a.wait_opcode(0x0091, LONG).await.expect("0x0091");
    mapmove(&frame)
}

/// Checks the post-`0x007d` burst: weights, no login-only packets.
fn assert_entry_burst(frames: &[RawPacket]) {
    let pars: Vec<u16> = frames
        .iter()
        .filter(|f| f.opcode.0 == 0x00b0)
        .map(|f| frame_u16(f, 2))
        .collect();
    assert_eq!(pars, [SP_WEIGHT, SP_MAXWEIGHT], "0x00b0 WEIGHT, MAXWEIGHT");
    for op in [0x0283, 0x0b18, 0x02eb] {
        assert!(
            frames.iter().all(|f| f.opcode.0 != op),
            "no {op:#06x} after a warp"
        );
    }
}

// ---------- log capture ----------

/// Captures this thread's WARN and ERROR logs. `#[tokio::test]` runs every task on the test
/// thread, so the actors' and sessions' logs land here too.
struct Logs(Arc<Mutex<Vec<u8>>>);

struct LogWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Logs {
    fn capture() -> (Self, tracing::subscriber::DefaultGuard) {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let writer = Arc::clone(&buf);
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::WARN)
            .with_writer(move || LogWriter(Arc::clone(&writer)))
            .finish();
        (Self(buf), tracing::subscriber::set_default(subscriber))
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }

    fn count(&self, needle: &str) -> usize {
        self.text().matches(needle).count()
    }
}

// ---------- BIN-WARP-01 ----------

#[tokio::test]
async fn bin_warp_01_cross_map_end_to_end() {
    let repo =
        InMemoryRepo::seeded_on(&[(A, MAP, 105, 100), (O, MAP, 107, 103), (P, MAP2, 102, 101)]);
    let server = TestServer::start_with_loader(repo, cross_map_loader()).await;
    let mut o = server.join(O).await;
    let mut p = server.join(P).await;
    let mut a = server.join(A).await;
    o.collect(SHORT).await;

    assert_eq!(walk_into_warp(&mut a).await, (MAP2.to_string(), 100, 100));
    let vanish = o.wait_for(LONG, vanish_of(A)).await.expect("0x0080 for A");
    assert_eq!(frame_u8(&vanish, 6), 0, "vanish type 0");

    a.send(&[0x7d, 0x00]).await;
    let frames = a.collect(SHORT).await;
    assert_entry_burst(&frames);
    assert!(
        frames
            .iter()
            .any(|f| is_entry(f) && entry_aid(f) == account(P)),
        "entry packet for P"
    );
    assert!(
        p.wait_for(LONG, spawn_of(A)).await.is_some(),
        "P sees A spawn"
    );

    a.send(&cz_request_time(7)).await;
    assert!(
        a.wait_opcode(0x007f, LONG).await.is_some(),
        "0x0360 answered"
    );
    assert_eq!(cell(server.position_on(MAP2, A).await), Some((100, 100)));
    assert_eq!(server.position_on(MAP, A).await, None);

    a.send(&cz_request_move(103, 100)).await;
    assert!(
        a.wait_opcode(0x0087, LONG).await.is_some(),
        "walks on map 2"
    );
}

// ---------- BIN-WARP-02 ----------

#[tokio::test]
async fn bin_warp_02_same_map_end_to_end() {
    let repo = InMemoryRepo::seeded(&[(A, 105, 100), (O, 107, 103), (P, 152, 150)]);
    let server = TestServer::start_with_loader(repo, same_map_loader()).await;
    let mut o = server.join(O).await;
    let mut p = server.join(P).await;
    let mut a = server.join(A).await;
    o.collect(SHORT).await;

    assert_eq!(walk_into_warp(&mut a).await, (MAP.to_string(), 150, 150));
    let vanish = o.wait_for(LONG, vanish_of(A)).await.expect("0x0080 for A");
    assert_eq!(frame_u8(&vanish, 6), 0, "vanish type 0");

    a.send(&[0x7d, 0x00]).await;
    let frames = a.collect(SHORT).await;
    assert_entry_burst(&frames);
    assert!(frames
        .iter()
        .any(|f| is_entry(f) && entry_aid(f) == account(P)));
    assert!(
        p.wait_for(LONG, spawn_of(A)).await.is_some(),
        "P sees A spawn"
    );
    assert!(
        o.collect(SHORT).await.iter().all(|f| !spawn_of(A)(f)),
        "O is out of view of the destination"
    );
    assert_eq!(cell(server.position(A).await), Some((150, 150)));

    a.send(&cz_request_move(153, 152)).await;
    assert!(a.wait_opcode(0x0087, LONG).await.is_some());
}

// ---------- BIN-WARP-03 ----------

/// Connects and enters as `n`, retrying while the character is still claimed. Returns the
/// client and the cell from `ZC_ACCEPT_ENTER`.
async fn login_retrying(server: &TestServer, n: u32) -> (Client, (u16, u16)) {
    for _ in 0..40 {
        let mut c = Client::connect(server.addr).await;
        c.send(&cz_enter(n)).await;
        let (bytes, eof) = c.read_until_eof(Duration::from_millis(250)).await;
        if !eof && bytes.len() >= 23 && bytes[0..2] == [0x83, 0x02] {
            let pos: [u8; 3] = bytes[16..19].try_into().unwrap();
            let (x, y, _) = ro_packets::unpack_pos_dir(&pos);
            return (c, (x, y));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("character {n} never became free to log in");
}

#[tokio::test]
async fn bin_warp_03_disconnect_in_transit_releases_once() {
    let (logs, _guard) = Logs::capture();
    // Both arrival cells are warp cells, so every login warps on its first 0x007d.
    let loader = maps_loader(vec![
        (
            MAP,
            npcs(vec![warp_to("go", (110, 100), "geffen", (100, 100))]),
        ),
        (
            MAP2,
            npcs(vec![warp_to("back", (100, 100), "prontera", (110, 100))]),
        ),
    ]);
    let repo = InMemoryRepo::seeded(&[(A, 110, 100)]);
    let server = TestServer::start_with_loader(repo, loader).await;

    // Closing at different delays after 0x007d hits every stage: before the tick, with the
    // handoff queued, and after the destination's join.
    for delay_ms in [0, 0, 2, 5, 10, 15, 20, 30, 40, 60, 90, 150] {
        let (mut a, _) = login_retrying(&server, A).await;
        a.send(&[0x7d, 0x00]).await;
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        a.close().await;
    }

    let (_a, at) = login_retrying(&server, A).await;
    let mut second = Client::connect(server.addr).await;
    second.send(&cz_enter(A)).await;
    let (bytes, eof) = second.read_until_eof(LONG).await;
    assert!(eof && bytes.is_empty(), "a concurrent login is rejected");

    let on_map1 = cell(server.position_on(MAP, A).await);
    let on_map2 = cell(server.position_on(MAP2, A).await);
    assert!(
        (on_map1 == Some(at) && at == (110, 100)) || (on_map2 == Some(at) && at == (100, 100)),
        "entered at a trigger cell or the destination: {at:?} map1 {on_map1:?} map2 {on_map2:?}"
    );
    assert_eq!(logs.count("not claimed"), 0, "{}", logs.text());
}

/// The destination rejects the warp join (here: the account is already on map 2 with another
/// character). The claim releases with the trigger cell and the server closes the socket.
#[tokio::test]
async fn bin_warp_03_rejected_warp_join_releases_with_the_trigger_cell() {
    const ALT: u32 = 4;
    let repo = InMemoryRepo::seeded_on(&[(A, MAP, 105, 100), (ALT, MAP2, 50, 50)]);
    repo.set_account(ALT, account(A));
    let server = TestServer::start_with_loader(repo, cross_map_loader()).await;
    let mut alt = Client::connect(server.addr).await;
    alt.send(&cz_enter_raw(account(A), char_id(ALT))).await;
    alt.read_exact_bytes(23).await;
    alt.send(&[0x7d, 0x00]).await;
    alt.collect(SHORT).await;

    let mut a = server.join(A).await;
    a.send(&cz_request_move(112, 100)).await;
    let frames = a.collect(Duration::from_secs(2)).await;
    assert!(frames.iter().any(|f| f.opcode.0 == 0x0087));
    assert!(frames.iter().all(|f| f.opcode.0 != 0x0091), "no 0x0091");
    let (_, eof) = a.read_until_eof(LONG).await;
    assert!(eof, "the server closes the session");

    let mut landed = false;
    for _ in 0..100 {
        if server.repo.stored(A).map(|(m, p)| (m, (p.x, p.y)))
            == Some((MAP.to_string(), (109, 100)))
        {
            landed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(landed, "trigger cell stored: {:?}", server.repo.stored(A));
    let (_a, at) = login_retrying(&server, A).await;
    assert_eq!(at, (109, 100));
}

// ---------- BIN-WARP-04 ----------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("metaling-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    fn sub(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(&path).expect("sub dir");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn warp_yaml(name: &str, x: u16, y: u16, map: &str) -> String {
    format!(
        "  - Name: {name}\n    X: {x}\n    Y: {y}\n    Dir: 0\n    Sprite: 45\n    Trigger:\n      X: 1\n      Y: 1\n    Kind: Warp\n    Warp:\n      Map: {map}\n      X: 10\n      Y: 10\n      TriggerWhileHidden: false\n    Source: t:{x}\n"
    )
}

/// prontera.yml: warps at (25, 20) to a map without a `.gat`, (30, 20) to an instance map,
/// (40, 40) back to prontera, and (45, 45) to a name too long for `0x0091`.
fn write_warp_npcs(dir: &Path) {
    let yaml = format!(
        "Header:\n  Type: NPC_SPAWN_DB\n  Version: 1\nBody:\n{}{}{}{}",
        warp_yaml("w_missing", 25, 20, "nowhere"),
        warp_yaml("w_instance", 30, 20, "1@tower"),
        warp_yaml("w_ok", 40, 40, "prontera"),
        warp_yaml("w_long", 45, 45, "abcdefghijkl"),
    );
    std::fs::write(dir.join("prontera.yml"), yaml).expect("write npcs");
}

#[tokio::test]
async fn bin_warp_04_unreachable_destinations_are_inert() {
    let root = TempDir::new("warp04-inert");
    let maps = root.sub("maps");
    let npc_dir = root.sub("npc");
    std::fs::write(maps.join("prontera.gat"), gat_file(60, 60)).expect("write gat");
    write_warp_npcs(&npc_dir);

    let (logs, _guard) = Logs::capture();
    let loader = map_data_loader(Some(maps), Some(npc_dir));
    let data = loader("prontera.gat");
    let triggers: Vec<_> = data
        .npcs
        .npcs
        .iter()
        .map(|n| (n.name.as_str(), n.trigger))
        .collect();
    assert_eq!(
        triggers,
        [
            ("w_missing", None),
            ("w_instance", None),
            ("w_ok", Some((1, 1))),
            ("w_long", None)
        ]
    );
    assert_eq!(logs.count("is inert"), 3, "one warning per warp");

    let server = TestServer::start_with_loader(InMemoryRepo::seeded(&[(A, 20, 20)]), loader).await;
    let mut a = server.join(A).await;
    a.send(&cz_request_move(34, 20)).await;
    assert!(a.wait_opcode(0x0087, LONG).await.is_some());
    assert!(
        server
            .wait_position(A, (34, 20), Duration::from_secs(4))
            .await,
        "walked across both areas"
    );
    assert!(
        a.collect(SHORT).await.iter().all(|f| f.opcode.0 != 0x0091),
        "no 0x0091"
    );
}

#[tokio::test]
async fn bin_warp_04_without_map_data_dir_uses_the_fallback_grid() {
    let root = TempDir::new("warp04-fallback");
    let npc_dir = root.sub("npc");
    write_warp_npcs(&npc_dir);
    let loader = map_data_loader(None, Some(npc_dir));
    let server = TestServer::start_with_loader(InMemoryRepo::seeded(&[(A, 20, 20)]), loader).await;
    let mut a = server.join(A).await;

    a.send(&cz_request_move(27, 20)).await;
    let frame = a.wait_opcode(0x0091, LONG).await.expect("0x0091");
    assert_eq!(mapmove(&frame), ("nowhere.gat".to_string(), 10, 10));
    a.send(&[0x7d, 0x00]).await;
    assert_entry_burst(&a.collect(SHORT).await);
    assert_eq!(
        cell(server.position_on("nowhere.gat", A).await),
        Some((10, 10))
    );
}

// ---------- BIN-WARP-05 ----------

async fn moves_ignored_while_loading(
    loader: map_server::MapDataLoader,
    dest_map: &str,
    dest: (u16, u16),
) {
    let repo = InMemoryRepo::seeded(&[(A, 105, 100)]);
    let server = TestServer::start_with_loader(repo, loader).await;
    let mut a = server.join(A).await;
    assert_eq!(
        walk_into_warp(&mut a).await,
        (dest_map.to_string(), dest.0, dest.1)
    );
    a.collect(Duration::from_millis(50)).await;

    a.send(&cz_request_move(dest.0 + 3, dest.1)).await;
    assert!(
        a.collect(SHORT).await.iter().all(|f| f.opcode.0 != 0x0087),
        "no 0x0087 before 0x007d"
    );
    assert_eq!(cell(server.position_on(dest_map, A).await), Some(dest));

    a.send(&[0x7d, 0x00]).await;
    assert_entry_burst(&a.collect(SHORT).await);
    a.send(&[0x7d, 0x00]).await;
    let dup = a.collect(SHORT).await;
    assert!(dup.is_empty(), "a duplicate 0x007d sends nothing: {dup:?}");

    a.send(&cz_request_move(dest.0 + 3, dest.1)).await;
    assert!(
        a.wait_opcode(0x0087, LONG).await.is_some(),
        "moves work again"
    );
}

#[tokio::test]
async fn bin_warp_05_moves_while_loading_are_ignored_cross_map() {
    moves_ignored_while_loading(cross_map_loader(), MAP2, (100, 100)).await;
}

#[tokio::test]
async fn bin_warp_05_moves_while_loading_are_ignored_same_map() {
    moves_ignored_while_loading(same_map_loader(), MAP, (150, 150)).await;
}

// ---------- BIN-WARP-06 ----------

#[tokio::test]
async fn bin_warp_06_restart_after_a_map_change() {
    let repo = InMemoryRepo::seeded(&[(A, 105, 100)]);
    let server = TestServer::start_with_loader(repo, cross_map_loader()).await;
    let mut a = server.join(A).await;
    walk_into_warp(&mut a).await;
    a.send(&[0x7d, 0x00]).await;
    a.collect(SHORT).await;

    a.send(&[0xb2, 0x00, 0x01]).await;
    assert!(a.wait_opcode(0x00b3, LONG).await.is_some(), "0x00b3");
    let stored = (MAP2.to_string(), (100, 100));
    let mut landed = false;
    for _ in 0..100 {
        if server.repo.stored(A).map(|(m, p)| (m, (p.x, p.y))) == Some(stored.clone()) {
            landed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(landed, "stored on map 2: {:?}", server.repo.stored(A));
    a.close().await;

    let (_a, at) = login_retrying(&server, A).await;
    assert_eq!(at, (100, 100));
    assert_eq!(cell(server.position_on(MAP2, A).await), Some((100, 100)));
}
