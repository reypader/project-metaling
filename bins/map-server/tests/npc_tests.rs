//! NPC spawning in the map-server (spec `specs/npc-spawning/spec.md` BIN-NPC-01).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::*;
use map_server::{load_npcs, MapConfig};
use ro_db::{MapMobSpawns, MapNpcs, NpcKind, NpcSpawn, NpcState};

const LONG: Duration = Duration::from_secs(4);
const START_NPC_ID: u32 = 110_000_000;

const PRONTERA_YML: &str = "Header:\n  Type: NPC_SPAWN_DB\n  Version: 1\nBody:\n  - Name: Kafra\n    X: 1\n    Y: 2\n    Dir: 4\n    Sprite: 117\n    Kind: Script\n    Source: t:1\n";

fn kafra(x: u16, y: u16) -> NpcSpawn {
    NpcSpawn {
        name: "Kafra Employee#prt1".into(),
        unique_name: "Kafra Employee#prt1".into(),
        x,
        y,
        dir: 4,
        sprite: 117,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: None,
        duplicate_of: None,
        kind: NpcKind::Script,
        source: "t:1".into(),
    }
}

#[tokio::test]
async fn bin_npc_01_loaded_npcs_are_seen_on_entry() {
    let loader = data_loader(
        &[],
        MapNpcs {
            npcs: vec![kafra(103, 104)],
        },
        MapMobSpawns::default(),
    );
    let server =
        TestServer::start_with_loader(InMemoryRepo::seeded(&[(1, 100, 100)]), loader).await;
    // Not `join`: it discards the entry frames this test looks for.
    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(1)).await;
    a.read_exact_bytes(23).await;
    a.send(&[0x7d, 0x00]).await;
    let npc = a
        .wait_for(LONG, |f| {
            f.opcode.0 == 0x09ff && entry_aid(f) == START_NPC_ID
        })
        .await
        .expect("0x09ff for the NPC");
    assert_eq!(frame_u8(&npc, 4), 0x06, "object_type NPC");
    assert_eq!(entry_gid(&npc), 0);
    assert_eq!(frame_u16(&npc, 23), 117, "job = sprite");
    assert_eq!(frame_len(&npc), 108);
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bin_npc_01_load_npcs_from_dir() {
    let dir = TempDir(std::env::temp_dir().join(format!("metaling-npc-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).expect("temp dir");
    std::fs::write(dir.0.join("prontera.yml"), PRONTERA_YML).expect("write");
    std::fs::write(dir.0.join("broken.yml"), "Header: [").expect("write");

    let loaded = load_npcs(Some(&dir.0), "prontera.gat");
    assert_eq!(loaded.npcs.len(), 1);
    assert_eq!(loaded.npcs[0].name, "Kafra");
    for (d, map) in [
        (Some(dir.0.as_path()), "missing.gat"),
        (Some(dir.0.as_path()), "broken.gat"),
        (None, "prontera.gat"),
    ] {
        assert!(load_npcs(d, map).npcs.is_empty(), "{map}");
    }
}

#[test]
fn bin_npc_01_npc_data_dir_config() {
    let from_env = MapConfig::from_args_and_env(Vec::<String>::new(), |k| {
        (k == "NPC_DATA_DIR").then(|| "/env/npc".to_string())
    });
    assert_eq!(from_env.npc_data_dir, Some(PathBuf::from("/env/npc")));
    let from_arg = MapConfig::from_args_and_env(["--npc-data-dir", "/arg/npc"], |k| {
        (k == "NPC_DATA_DIR").then(|| "/env/npc".to_string())
    });
    assert_eq!(from_arg.npc_data_dir, Some(PathBuf::from("/arg/npc")));
    assert_eq!(
        MapConfig::from_args_and_env(Vec::<String>::new(), |_| None).npc_data_dir,
        None
    );
}
