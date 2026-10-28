//! Stats through the map-server (spec `specs/stats/spec.md` BIN-STAT-01, BIN-STAT-02): the
//! progress saves with the inventory and comes back on the next login, `@job` saves at once,
//! and the battle config loads from `BATTLE_CONFIG`. The Postgres side of the save is
//! `ro-persistence`'s PER-STAT-01.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::time::Duration;

use common::*;
use map_server::load_battle_config;
use ro_mechanics::BattleConfig;

const A: u32 = 1;
const LONG: Duration = Duration::from_secs(2);
const SP_STR: u16 = 13;
const SWORDMAN: u16 = 1;

fn chat(n: u32, message: &str) -> Vec<u8> {
    let text = format!("Char{n} : {message}");
    let len = u16::try_from(4 + text.len()).expect("short");
    let mut frame = vec![0xf3, 0x00];
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(text.as_bytes());
    frame
}

/// `CZ_STATUS_CHANGE`: raise `var` by `amount`.
fn raise(var: u16, amount: u8) -> Vec<u8> {
    let mut frame = vec![0xbb, 0x00];
    frame.extend_from_slice(&var.to_le_bytes());
    frame.push(amount);
    frame
}

/// BIN-STAT-01: a stat-up, a logout, a relog: STR and the points come back in `0x00bd`.
#[tokio::test]
async fn bin_stat_01_stat_up_survives_relog() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    a.send(&raise(SP_STR, 3)).await;
    let ack = a.wait_opcode(0x00bc, LONG).await.expect("stat-up ack");
    assert_eq!(&ack.payload[..], &[0x0d, 0x00, 0x01, 0x04], "STR 1 -> 4");
    a.close().await;
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo
            .stored_progress(A)
            .is_some_and(|p| p.status.str == 4 && p.status.status_point == 48 - 6))
        .await,
        "saved on logout: {:?}",
        repo.stored_progress(A)
    );

    let (_a, frames) = enter(&server, A).await;
    let window = frames
        .iter()
        .find(|f| f.opcode.0 == 0x00bd)
        .expect("status window on login");
    let points = u16::from_le_bytes([window.payload[0], window.payload[1]]);
    assert_eq!((points, window.payload[2]), (42, 4), "points, then STR");
}

/// BIN-STAT-01: `@job` saves right away, without a logout.
#[tokio::test]
async fn bin_stat_01_job_change_saves_at_once() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_gm(A);
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    a.send(&chat(A, "@job swordman")).await;
    assert!(
        a.wait_opcode(0x010f, LONG).await.is_some(),
        "skill list re-sent"
    );
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo
            .stored_progress(A)
            .is_some_and(|p| p.job_class == SWORDMAN))
        .await,
        "saved while still online: {:?}",
        repo.stored_progress(A)
    );
    assert_eq!(
        repo.inventory_saves_of(A),
        1,
        "one save, not one per follow-up"
    );
}

/// BIN-STAT-02: an unset `BATTLE_CONFIG` gives the shipped defaults; the shipped
/// `config/battle.yml` loads to the same; a bad file is an error naming it.
#[test]
fn bin_stat_02_battle_config() {
    assert_eq!(
        load_battle_config(None).expect("defaults"),
        BattleConfig::default()
    );
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/battle.yml");
    assert_eq!(
        load_battle_config(Some(&shipped)).expect("shipped file loads"),
        BattleConfig::default()
    );

    let dir = std::env::temp_dir().join(format!("metaling-bin-stat-02-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let missing = dir.join("missing.yml");
    let unknown_key = dir.join("unknown_key.yml");
    std::fs::write(&unknown_key, "max_parametr: 120\n").expect("write");
    let not_yaml = dir.join("not_yaml.yml");
    std::fs::write(&not_yaml, "max_parameter: [\n").expect("write");
    for path in [&missing, &unknown_key, &not_yaml] {
        let err = load_battle_config(Some(path)).expect_err("refused");
        assert!(
            format!("{err:#}").contains(&path.display().to_string()),
            "{err:#}"
        );
    }
    let partial = dir.join("partial.yml");
    std::fs::write(&partial, "max_parameter: 120\n").expect("write");
    let config = load_battle_config(Some(&partial)).expect("partial loads");
    assert_eq!(config.max_parameter, 120);
    assert_eq!(config.max_aspd, BattleConfig::default().max_aspd);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
