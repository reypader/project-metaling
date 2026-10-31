//! Combat through the map-server (spec `specs/combat/spec.md` BIN-CMB-01, PER-CMB-01): a kill
//! over TCP with the shipped mob and job data, and its exp reaching the saves. The Postgres
//! side of the save (exp and levels round-trip) is `ro-persistence`'s PER-INV-04 / PER-STAT-01.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;
use map_server::MapServiceOptions;
use ro_core::{CharacterProgress, CharacterStatus};
use ro_db::{MapMobSpawns, MapNpcs, MobId, MobSpawn};
use ro_packets::RawPacket;

const A: u32 = 1;
/// The first mob gid on a map.
const MOB_GID: u32 = 110_000_000;
/// Long enough for a base-50 Novice to kill a 55-HP Poring bare-handed.
const KILL: Duration = Duration::from_secs(20);
const LONG: Duration = Duration::from_secs(2);
const VANISH_DIED: u8 = 1;
const SP_BASEEXP: u16 = 1;
const SP_JOBEXP: u16 = 2;

/// One Poring (shipped 1002: 55 HP, 150/40 exp) next to the seeded character at (100, 100).
fn poring_loader() -> map_server::MapDataLoader {
    let spawn = MobSpawn {
        name: "Poring".into(),
        mob_id: MobId(1002),
        x: 101,
        y: 100,
        span: (0, 0),
        amount: 1,
        delay1: 5000,
        delay2: 0,
        level: None,
        boss: false,
        event: None,
        source: "test:poring".into(),
    };
    data_loader(
        &[],
        MapNpcs::default(),
        MapMobSpawns {
            spawns: vec![spawn],
        },
    )
}

/// A base-50, job-9 Novice: the kill (150/40) levels neither (bars far above, and 340).
fn seeded_repo() -> InMemoryRepo {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_progress(
        A,
        CharacterProgress {
            job_class: 0,
            base_level: 50,
            status: CharacterStatus {
                job_level: 9,
                ..CharacterStatus::default()
            },
            change_level: [0; 3],
        },
    );
    repo
}

/// `CZ_REQUEST_ACT` on `target`.
fn request_act(target: u32, action: u8) -> Vec<u8> {
    let mut b = vec![0x37, 0x04];
    b.extend_from_slice(&target.to_le_bytes());
    b.push(action);
    b
}

fn is_death_vanish(f: &RawPacket) -> bool {
    f.opcode.0 == 0x0080 && frame_u32(f, 2) == MOB_GID && frame_u8(f, 6) == VANISH_DIED
}

/// Attacks the Poring with action 7 and returns every frame up to its death vanish.
async fn kill_poring(a: &mut Client) -> Vec<RawPacket> {
    a.send(&request_act(MOB_GID, 7)).await;
    let vanish = a
        .wait_for(KILL, is_death_vanish)
        .await
        .expect("0x0080 type 1 for the Poring");
    let mut frames = a.collect(Duration::ZERO).await;
    frames.push(vanish);
    frames
}

/// The `0x0acb` value for `var` in `frames`, last one wins.
fn long_param(frames: &[RawPacket], var: u16) -> Option<i64> {
    frames
        .iter()
        .filter(|f| f.opcode.0 == 0x0acb && frame_u16(f, 2) == var)
        .map(|f| i64::from_le_bytes(f.payload[2..10].try_into().expect("8 bytes")))
        .last()
}

/// BIN-CMB-01: log in, attack an adjacent mob until it dies: the swings (`0x08c8`), the HP bars
/// (`0x0977`, the last at 0), the exp (`0x0acb`, `0x0acc` ×2) and the death vanish, in order.
#[tokio::test]
async fn bin_cmb_01_kill_over_tcp() {
    let server = TestServer::start_with_loader(seeded_repo(), poring_loader()).await;
    let (mut a, _) = enter(&server, A).await;
    let frames = kill_poring(&mut a).await;
    let pos = |op: u16| frames.iter().position(|f| f.opcode.0 == op);
    let swing = pos(0x08c8).expect("a swing");
    assert_eq!(frame_u32(&frames[swing], 6), MOB_GID, "the swing's target");
    let dead_bar = frames
        .iter()
        .position(|f| f.opcode.0 == 0x0977 && frame_u32(f, 6) == 0)
        .expect("0x0977 hp 0");
    assert_eq!(
        (
            frame_u32(&frames[dead_bar], 2),
            frame_u32(&frames[dead_bar], 10)
        ),
        (MOB_GID, 55)
    );
    let exp = pos(0x0acc).expect("0x0acc");
    assert!(swing < dead_bar && dead_bar < exp, "{:?}", ops(&frames));
    let gains: Vec<(u16, i64)> = frames
        .iter()
        .filter(|f| f.opcode.0 == 0x0acc)
        .map(|f| {
            let amount = i64::from_le_bytes(f.payload[4..12].try_into().expect("8 bytes"));
            (frame_u16(f, 14), amount)
        })
        .collect();
    assert_eq!(gains, [(SP_BASEEXP, 150), (SP_JOBEXP, 40)]);
    assert_eq!(long_param(&frames, SP_BASEEXP), Some(150));
    assert_eq!(long_param(&frames, SP_JOBEXP), Some(40));
}

/// PER-CMB-01: the kill's exp reaches the autosave without a logout (the progress changed),
/// and the next login loads it.
#[tokio::test]
async fn per_cmb_01_kill_exp_is_saved_and_survives_relog() {
    let options = MapServiceOptions {
        autosave_interval: Duration::from_millis(1200),
    };
    let server = TestServer::start_with_options(seeded_repo(), poring_loader(), options).await;
    let (mut a, _) = enter(&server, A).await;
    kill_poring(&mut a).await;
    let repo = server.repo.clone();
    let exp_saved = || {
        repo.stored_progress(A)
            .is_some_and(|p| (p.status.base_exp, p.status.job_exp) == (150, 40))
    };
    assert!(
        wait_until_for(exp_saved, Duration::from_secs(4)).await,
        "autosaved while online: {:?}",
        repo.stored_progress(A)
    );
    a.close().await;
    assert!(wait_until(exp_saved).await);

    let mut a = Client::connect(server.addr).await;
    a.send(&cz_enter(A)).await;
    a.read_exact_bytes(23 + 4).await;
    let mut login = a.read_login_burst().await;
    a.send(&[0x7d, 0x00]).await;
    login.extend(a.collect(LONG).await);
    assert_eq!(
        long_param(&login, SP_BASEEXP),
        Some(150),
        "{:?}",
        ops(&login)
    );
    assert_eq!(long_param(&login, SP_JOBEXP), Some(40));
    assert!(
        login.iter().all(|f| f.opcode.0 != 0x0acc),
        "no gain on login"
    );
}
