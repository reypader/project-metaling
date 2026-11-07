//! `Claim` lifecycle and the position writer (spec BIN-WARP-07).

use std::sync::{atomic::Ordering, Arc};

use ro_core::CharacterLooks;
use ro_core::{CharId, CharacterProgress};
use ro_engine::PlaySnapshot;
use ro_persistence::RepositoryError;
use tokio::sync::mpsc;

use super::{
    write_dto::{Write, WriteKind},
    *,
};
use crate::test_support::{at, eventually, potions, snapshot, FakeRepo, CHAR};

fn state() -> (Arc<SharedState>, mpsc::UnboundedReceiver<Write>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (Arc::new(SharedState::with_writer(tx)), rx)
}

fn is_online(state: &SharedState, char_id: CharId) -> bool {
    lock(&state.online).contains(&char_id)
}

#[test]
fn bin_warp_07_drop_with_last_records_queues_then_releases() {
    let (state, mut rx) = state();
    let mut claim = state.claim(CHAR).expect("claim");
    assert!(state.claim(CHAR).is_none(), "held");
    claim.set_last("prontera.gat", at(5, 6));
    drop(claim);
    assert_eq!(
        state.recent_position(CHAR),
        Some(("prontera.gat".to_string(), at(5, 6)))
    );
    let write = rx.try_recv().expect("write queued");
    assert_eq!(write.char_id, CHAR);
    assert_eq!(
        write.kind,
        WriteKind::Position {
            map: "prontera.gat".into(),
            pos: at(5, 6)
        }
    );
    assert!(!is_online(&state, CHAR), "released");
}

#[test]
fn bin_warp_07_drop_without_last_only_releases() {
    let (state, mut rx) = state();
    drop(state.claim(CHAR).expect("claim"));
    assert!(rx.try_recv().is_err(), "nothing queued");
    assert_eq!(state.recent_position(CHAR), None);
    assert!(
        state.claim(CHAR).is_some(),
        "released, so it can be claimed again"
    );
}

/// An unheld release is a bug: an error, a `debug_assert!` in debug builds, else `false`.
#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "release of unclaimed"))]
fn bin_warp_07_unheld_release_is_an_error() {
    let (state, _rx) = state();
    assert!(!state.release(CHAR));
}

#[test]
fn bin_warp_07_claim_in_a_dropped_channel_is_released_once() {
    let (state, _rx) = state();
    // Pushed before the receiver drops.
    let (tx, rx) = mpsc::channel::<Claim>(1);
    tx.try_send(state.claim(CHAR).expect("claim"))
        .map_err(|_| ())
        .expect("queued");
    assert!(is_online(&state, CHAR));
    drop(rx);
    drop(tx);
    assert!(!is_online(&state, CHAR));

    // Pushed after the receiver drops: the failed send hands it back, and it drops.
    let (tx, rx) = mpsc::channel::<Claim>(1);
    drop(rx);
    let claim = state.claim(CHAR).expect("claim again");
    assert!(tx.try_send(claim).is_err());
    drop(tx);
    assert!(!is_online(&state, CHAR));
    assert!(state.claim(CHAR).is_some(), "no double release");
}

#[tokio::test]
async fn bin_warp_07_failed_write_is_retried_until_it_lands() {
    let repo = FakeRepo::new(true);
    repo.failures.store(2, Ordering::Relaxed);
    let state = SharedState::new(repo.clone());
    let mut claim = state.claim(CHAR).expect("claim");
    claim.set_last("geffen.gat", at(7, 8));
    drop(claim);
    assert!(eventually(|| state.recent_position(CHAR).is_none()).await);
    assert_eq!(repo.writes(), [(CHAR, "geffen.gat".to_string(), at(7, 8))]);
}

#[tokio::test]
async fn bin_warp_07_writes_land_in_queue_order() {
    let repo = FakeRepo::new(false);
    let state = SharedState::new(repo.clone());
    for (x, y) in [(10, 10), (20, 20)] {
        let mut claim = state.claim(CHAR).expect("claim");
        claim.set_last("prontera.gat", at(x, y));
    }
    let newest = Some(("prontera.gat".to_string(), at(20, 20)));
    assert_eq!(state.recent_position(CHAR), newest);

    repo.gate.add_permits(1);
    assert!(eventually(|| repo.writes().len() == 1).await);
    assert_eq!(repo.writes()[0].2, at(10, 10), "older write first");
    assert_eq!(
        state.recent_position(CHAR),
        newest,
        "an older write doesn't clear a newer position"
    );

    repo.gate.add_permits(1);
    assert!(eventually(|| state.recent_position(CHAR).is_none()).await);
    assert_eq!(repo.writes()[1].2, at(20, 20));
}

/// Dropping a claim writes the position first, then the inventory (spec §2.8).
#[test]
fn bin_item_drop_writes_position_then_inventory() {
    let (state, mut rx) = state();
    let mut claim = state.claim(CHAR).expect("claim");
    claim.set_play(snapshot(3));
    claim.set_last("prontera.gat", at(1, 2));
    drop(claim);
    assert!(matches!(
        rx.try_recv().expect("first").kind,
        WriteKind::Position { .. }
    ));
    assert_eq!(
        rx.try_recv().expect("second").kind,
        WriteKind::Play(snapshot(3))
    );
    assert_eq!(
        state.recent_play(CHAR).map(|r| r.inventory),
        Some(potions(3))
    );
}

/// Only `Unavailable` is retried: a write that can't succeed doesn't stall the queue.
#[tokio::test]
async fn bin_item_writer_drops_permanent_errors() {
    let repo = FakeRepo::new(true);
    *lock(&repo.inventory_error) = Some(RepositoryError::Internal("too many rows".into()));
    let state = SharedState::new(repo.clone());
    state.persist_play(CHAR, snapshot(1));
    state.persist(CHAR, "geffen.gat".into(), at(3, 3));
    assert!(
        eventually(|| repo.writes().len() == 1).await,
        "the position behind it lands"
    );
    assert_eq!(
        state.recent_play(CHAR).map(|r| r.inventory),
        Some(potions(1)),
        "the dropped inventory stays for a re-entry in this process"
    );

    *lock(&repo.inventory_error) = Some(RepositoryError::NotFound);
    state.persist_play(CHAR, snapshot(2));
    assert!(
        eventually(|| state.recent_play(CHAR).is_none()).await,
        "a deleted character's write is done"
    );

    *lock(&repo.inventory_error) = None;
    state.persist_play(CHAR, snapshot(4));
    assert!(eventually(|| state.recent_play(CHAR).is_none()).await);
    assert_eq!(
        lock(&repo.inventory_saves).last(),
        Some(&(CHAR, potions(4)))
    );
}

/// The writer saves the snapshot's looks with its inventory (spec BIN-EQ-02).
#[tokio::test]
async fn bin_eq_02_writer_saves_looks_with_inventory() {
    let repo = FakeRepo::new(true);
    let state = SharedState::new(repo.clone());
    let looks = CharacterLooks {
        weapon_type: 1,
        shield: 1,
        head_top: 5,
        ..CharacterLooks::default()
    };
    state.persist_play(
        CHAR,
        PlaySnapshot {
            inventory: potions(1),
            looks,
            progress: CharacterProgress::default(),
            skills: Vec::new(),
        },
    );
    assert!(eventually(|| state.recent_play(CHAR).is_none()).await);
    assert_eq!(lock(&repo.looks).get(&CHAR), Some(&looks));
}
