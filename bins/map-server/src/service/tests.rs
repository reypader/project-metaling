//! `resolve_entry` claiming (spec BIN-WARP-07).

use std::sync::atomic::Ordering;
use std::{sync::Arc, time::Instant};

use bytes::Bytes;
use ro_core::{AccountId, CharacterSpawn, ClientIntent, SessionId, SessionIntent};
use ro_db::{MapGrid, MapMobSpawns, MapNpcs, NpcKind};
use ro_engine::Registries;
use tokio::sync::mpsc;

use super::{world_actor::MapWorldActor, *};
use crate::claim::SharedState;
use crate::test_support::{at, eventually, FakeRepo, ACCOUNT, CHAR};

fn service(repo: Arc<FakeRepo>) -> MapServiceHandle {
    let grid = Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims"));
    let data = MapData {
        grid,
        npcs: Arc::new(MapNpcs::default()),
        mobs: Arc::new(MapMobSpawns::default()),
        registries: Registries::default(),
    };
    MapServiceHandle::new(repo, Arc::new(move |_: &str| data.clone()))
}

#[tokio::test]
async fn bin_warp_07_resolve_entry_claims_before_reading() {
    let repo = FakeRepo::new(true);
    let service = service(repo.clone());
    let held = service.shared.claim(CHAR).expect("claim");
    assert!(service.resolve_entry(ACCOUNT, CHAR).await.is_none());
    assert_eq!(
        repo.finds.load(Ordering::Relaxed),
        0,
        "rejected before any read"
    );
    drop(held);
    let (_, spawn, _claim) = service.resolve_entry(ACCOUNT, CHAR).await.expect("enters");
    assert_eq!(spawn.pos, at(100, 100));
}

#[tokio::test]
async fn bin_warp_07_resolve_entry_uses_a_racing_release() {
    let repo = FakeRepo::new(false);
    let service = service(repo.clone());
    let mut claim = service.shared.claim(CHAR).expect("claim");
    claim.set_last("prontera.gat", at(120, 130));
    drop(claim);

    // The write is still blocked: the recorded position wins over the database.
    let (_, spawn, claim) = service.resolve_entry(ACCOUNT, CHAR).await.expect("enters");
    assert_eq!(spawn.pos, at(120, 130));
    drop(claim);

    // Once the write lands, the database has it.
    repo.gate.add_permits(1);
    assert!(eventually(|| service.shared.recent_position(CHAR).is_none()).await);
    let (_, spawn, _claim) = service.resolve_entry(ACCOUNT, CHAR).await.expect("enters");
    assert_eq!(spawn.pos, at(120, 130));
}

/// A handoff whose session task is gone releases its claim (with the trigger cell) and
/// closes the outbound queue, whether it was queued before or after the receiver dropped.
#[tokio::test]
async fn bin_warp_07_handoff_in_a_dropped_channel_releases() {
    let repo = FakeRepo::new(true);
    let service = service(repo.clone());
    for push_before_drop in [true, false] {
        let mut claim = service.shared.claim(CHAR).expect("claim");
        claim.set_last("prontera.gat", at(110, 100));
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Bytes>(1);
        let handoff = MapHandoff {
            claim,
            spawn: spawn_at(110, 100),
            dest_map: Arc::from("geffen.gat"),
            dest: (100, 100),
            rewarp_count: 0,
            outbound_tx,
        };
        let (control_tx, control_rx) = mpsc::channel::<SessionControl>(1);
        if push_before_drop {
            assert!(control_tx
                .try_send(SessionControl::ChangeMap(handoff))
                .is_ok());
            drop(control_rx);
        } else {
            drop(control_rx);
            assert!(control_tx
                .try_send(SessionControl::ChangeMap(handoff))
                .is_err());
        }
        drop(control_tx);
        assert_eq!(
            service.shared.recent_position(CHAR),
            Some(("prontera.gat".to_string(), at(110, 100))),
            "trigger cell recorded"
        );
        assert!(outbound_rx.recv().await.is_none(), "outbound queue closed");
        let again = service.shared.claim(CHAR);
        assert!(
            again.is_some(),
            "released (push before drop: {push_before_drop})"
        );
    }
}

/// Eviction in the transfer tick (the session's queue is full) releases the claim with the
/// trigger cell, and no handoff is sent.
#[tokio::test]
async fn bin_warp_07_eviction_in_the_transfer_tick_keeps_the_trigger_cell() {
    let shared = SharedState::new(FakeRepo::new(false));
    let warp = ro_db::NpcSpawn {
        name: "w".into(),
        unique_name: "w".into(),
        x: 50,
        y: 50,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: ro_db::NpcState::Normal,
        trigger: Some((1, 1)),
        duplicate_of: None,
        kind: NpcKind::Warp(ro_db::WarpDest {
            map: "geffen".into(),
            x: 100,
            y: 100,
            trigger_while_hidden: false,
        }),
        source: "t:1".into(),
    };
    let data = MapData {
        grid: Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims")),
        npcs: Arc::new(MapNpcs { npcs: vec![warp] }),
        mobs: Arc::new(MapMobSpawns::default()),
        registries: Registries::default(),
    };
    let mut actor = MapWorldActor::new(
        "prontera.gat".into(),
        data,
        Instant::now(),
        MapServiceOptions::default(),
    );
    let (outbound_tx, _outbound_rx) = mpsc::channel(1);
    let (control_tx, mut control_rx) = mpsc::channel(1);
    let session_id = SessionId(1);
    actor.join(JoinSession {
        session_id,
        account_id: ACCOUNT,
        spawn: CharacterSpawn {
            inventory: crate::test_support::potions(7),
            ..spawn_at(50, 50)
        },
        claim: shared.claim(CHAR).expect("claim"),
        outbound_tx,
        control_tx,
        entry: Entry::Login,
    });
    // ZC_ACCEPT_ENTER filled the one-slot queue, so this tick's batch can't be sent. The
    // arrival cell is a warp cell, so the same tick transfers the character out.
    actor.player_intent(SessionIntent::new(
        session_id,
        Some(ACCOUNT),
        Some(CHAR),
        ClientIntent::NotifyActorInit,
    ));
    actor.tick();
    assert!(control_rx.try_recv().is_err(), "evicted: no handoff");
    assert_eq!(
        shared.recent_position(CHAR),
        Some(("prontera.gat".to_string(), at(50, 50)))
    );
    assert_eq!(
        shared.recent_play(CHAR).map(|r| r.inventory),
        Some(crate::test_support::potions(7)),
        "the inventory that left with the warp is saved even though the hand-off never ran"
    );
    assert!(shared.claim(CHAR).is_some(), "released");
}

fn spawn_at(x: u16, y: u16) -> CharacterSpawn {
    CharacterSpawn {
        account_id: ACCOUNT,
        char_id: CHAR,
        name: "Char1".into(),
        sex: ro_core::Sex::Female,
        job_class: 0,
        base_level: 1,
        hair_style: 1,
        hair_color: 1,
        pos: at(x, y),
        speed: 150,
        status: Default::default(),
        group_id: 0,
        inventory: Default::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    }
}

#[tokio::test]
async fn bin_warp_07_rejection_after_claim_releases() {
    let repo = FakeRepo::new(true);
    let service = service(repo);
    let foreign = AccountId(ACCOUNT.0 + 1);
    assert!(service.resolve_entry(foreign, CHAR).await.is_none());
    assert!(
        service.resolve_entry(ACCOUNT, CHAR).await.is_some(),
        "the rejected entry released its claim"
    );
}
