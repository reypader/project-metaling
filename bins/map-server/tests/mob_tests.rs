//! Mob spawning in the map-server (spec `specs/mob-spawning/spec.md` BIN-MOB-01).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::*;
use map_server::{load_mob_spawns, map_data_loader, MapConfig};

const LONG: Duration = Duration::from_secs(4);
const START_NPC_ID: u32 = 110_000_000;

/// One Poring on a fixed cell next to the seeded character at (100, 100).
const PRONTERA_YML: &str = "Header:\n  Type: MOB_SPAWN_DB\n  Version: 1\nBody:\n  - Name: Poring\n    MobId: 1002\n    X: 103\n    Y: 104\n    Amount: 1\n    Delay1: 5000\n    Source: t:1\n";

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("metaling-mob-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn bin_mob_01_loaded_mobs_are_seen_on_entry() {
    let dir = TempDir::new("entry");
    std::fs::write(dir.0.join("prontera.yml"), PRONTERA_YML).expect("write");
    let loader = map_data_loader(None, None, Some(dir.0.clone()), Default::default());
    let server =
        TestServer::start_with_loader(InMemoryRepo::seeded(&[(1, 100, 100)]), loader).await;
    // Not `join`: it discards the entry frames this test looks for.
    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(1)).await;
    a.read_exact_bytes(23).await;
    a.send(&[0x7d, 0x00]).await;
    let mob = a
        .wait_for(LONG, |f| {
            f.opcode.0 == 0x09ff && entry_aid(f) == START_NPC_ID
        })
        .await
        .expect("0x09ff for the mob");
    assert_eq!(frame_u8(&mob, 4), 0x05, "object_type mob");
    assert_eq!(entry_gid(&mob), 0);
    assert_eq!(frame_u16(&mob, 13), 150, "speed");
    assert_eq!(frame_u16(&mob, 23), 1002, "job = mob id");
    assert_eq!(frame_len(&mob), 108);
}

#[test]
fn bin_mob_01_load_mob_spawns_from_dir() {
    let dir = TempDir::new("load");
    std::fs::write(dir.0.join("prontera.yml"), PRONTERA_YML).expect("write");
    std::fs::write(
        dir.0.join("broken.yml"),
        PRONTERA_YML.replace("Amount: 1", "Amount: 0"),
    )
    .expect("write");

    let loaded = load_mob_spawns(Some(&dir.0), "prontera.gat");
    assert_eq!(loaded.spawns.len(), 1);
    assert_eq!(loaded.spawns[0].name, "Poring");
    for (d, map) in [
        (Some(dir.0.as_path()), "missing.gat"),
        (Some(dir.0.as_path()), "broken.gat"),
        (None, "prontera.gat"),
    ] {
        assert!(load_mob_spawns(d, map).spawns.is_empty(), "{map}");
    }
}

#[test]
fn bin_mob_01_mob_spawn_data_dir_config() {
    let env = |k: &str| (k == "MOB_SPAWN_DATA_DIR").then(|| "/env/mob".to_string());
    let from_env = MapConfig::from_args_and_env(Vec::<String>::new(), env);
    assert_eq!(from_env.mob_spawn_data_dir, Some(PathBuf::from("/env/mob")));
    let from_arg = MapConfig::from_args_and_env(["--mob-spawn-data-dir", "/arg/mob"], env);
    assert_eq!(from_arg.mob_spawn_data_dir, Some(PathBuf::from("/arg/mob")));
    assert_eq!(
        MapConfig::from_args_and_env(Vec::<String>::new(), |_| None).mob_spawn_data_dir,
        None
    );
}

#[test]
fn bin_mob_01_run_script_points_at_the_shipped_data() {
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../run_metaling_servers.sh"),
    )
    .expect("run_metaling_servers.sh");
    assert!(
        script.contains("MOB_SPAWN_DATA_DIR") && script.contains("crates/ro-db/data/mob_spawn"),
        "the run script exports MOB_SPAWN_DATA_DIR"
    );
}
