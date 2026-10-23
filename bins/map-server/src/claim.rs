//! Character claims and the position writer (spec `specs/warp-map-change/spec.md` §1).
//!
//! Each online character has exactly one `Claim`. Dropping it is the only release: it records
//! the last known position for a fast re-entry, queues the database write, and only then frees
//! the character. One writer task runs the writes in queue order.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use ro_core::{CharId, MapPosition};
use ro_persistence::CharacterRepository;
use tokio::sync::mpsc;
use tracing::error;

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A position not yet confirmed written, tagged with its write's sequence number.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecentPosition {
    map: String,
    pos: MapPosition,
    seq: u64,
}

type RecentPositions = Arc<Mutex<HashMap<CharId, RecentPosition>>>;

/// One queued database write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositionWrite {
    pub(crate) char_id: CharId,
    pub(crate) map: String,
    pub(crate) pos: MapPosition,
    pub(crate) seq: u64,
}

/// State shared by the service handle and every map actor.
pub(crate) struct SharedState {
    /// Characters claimed by an entering or joined session, on any map.
    online: Mutex<HashSet<CharId>>,
    /// Last position of characters whose write hasn't landed yet.
    recent_positions: RecentPositions,
    next_seq: AtomicU64,
    writer_tx: mpsc::UnboundedSender<PositionWrite>,
}

impl SharedState {
    /// Starts the process's position writer on the current runtime.
    pub(crate) fn new(repo: Arc<dyn CharacterRepository>) -> Arc<Self> {
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();
        let state = Self::with_writer(writer_tx);
        tokio::spawn(run_position_writer(
            repo,
            Arc::clone(&state.recent_positions),
            writer_rx,
        ));
        Arc::new(state)
    }

    /// State whose writes go to `writer_tx` (tests read the other end).
    fn with_writer(writer_tx: mpsc::UnboundedSender<PositionWrite>) -> Self {
        Self {
            online: Mutex::new(HashSet::new()),
            recent_positions: Arc::new(Mutex::new(HashMap::new())),
            next_seq: AtomicU64::new(1),
            writer_tx,
        }
    }

    /// Claims `char_id`; `None` if it is already online somewhere.
    pub(crate) fn claim(self: &Arc<Self>, char_id: CharId) -> Option<Claim> {
        lock(&self.online).insert(char_id).then(|| Claim {
            shared: Arc::clone(self),
            char_id,
            last: None,
        })
    }

    /// Frees `char_id`. Only `Claim::drop` calls this, so an unheld release is a bug.
    fn release(&self, char_id: CharId) -> bool {
        let held = lock(&self.online).remove(&char_id);
        if !held {
            error!("release of character {} that is not claimed", char_id.0);
            debug_assert!(held, "release of unclaimed character {}", char_id.0);
        }
        held
    }

    /// The newest position of `char_id` whose write hasn't landed yet.
    pub(crate) fn recent_position(&self, char_id: CharId) -> Option<(String, MapPosition)> {
        lock(&self.recent_positions)
            .get(&char_id)
            .map(|r| (r.map.clone(), r.pos))
    }

    /// Records the position for a re-entry that races the write, then queues the write.
    fn persist(&self, char_id: CharId, map: String, pos: MapPosition) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        lock(&self.recent_positions).insert(
            char_id,
            RecentPosition {
                map: map.clone(),
                pos,
                seq,
            },
        );
        let write = PositionWrite {
            char_id,
            map,
            pos,
            seq,
        };
        if self.writer_tx.send(write).is_err() {
            error!(
                "position writer gone; position of char {} not persisted",
                char_id.0
            );
        }
    }
}

/// Runs writes in queue order. After a write lands, its recent entry is cleared unless a newer
/// release replaced it.
async fn run_position_writer(
    repo: Arc<dyn CharacterRepository>,
    recent: RecentPositions,
    mut rx: mpsc::UnboundedReceiver<PositionWrite>,
) {
    while let Some(write) = rx.recv().await {
        match repo
            .update_position(write.char_id, &write.map, write.pos)
            .await
        {
            Ok(()) => {
                let mut recent = lock(&recent);
                if recent
                    .get(&write.char_id)
                    .is_some_and(|r| r.seq == write.seq)
                {
                    recent.remove(&write.char_id);
                }
            }
            Err(err) => error!(
                "Failed to persist position of char {}: {}",
                write.char_id.0, err
            ),
        }
    }
}

/// One character's entry in the online set. It moves with the session (into the map actor,
/// and between maps on a warp); dropping it persists `last` (if set) and releases.
pub struct Claim {
    shared: Arc<SharedState>,
    char_id: CharId,
    last: Option<(String, MapPosition)>,
}

impl Claim {
    pub fn char_id(&self) -> CharId {
        self.char_id
    }

    /// Records the newest known position, written when the claim drops.
    pub(crate) fn set_last(&mut self, map: &str, pos: MapPosition) {
        self.last = Some((map.to_string(), pos));
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        // Record before release: a login that claims right after sees the new position.
        if let Some((map, pos)) = self.last.take() {
            self.shared.persist(self.char_id, map, pos);
        }
        self.shared.release(self.char_id);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! `Claim` lifecycle and the position writer (spec BIN-WARP-07).

    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use async_trait::async_trait;
    use ro_core::{AccountId, CharacterSummary, CreateCharacterRequest, Sex};
    use ro_persistence::{
        CancelDeleteOutcome, CreateCharacterError, DeleteReserveOutcome, RepositoryError,
    };
    use tokio::sync::Semaphore;

    use super::*;

    pub(crate) const CHAR: CharId = CharId(150_001);
    pub(crate) const ACCOUNT: AccountId = AccountId(2_000_001);

    pub(crate) fn at(x: u16, y: u16) -> MapPosition {
        MapPosition { x, y, dir: 0 }
    }

    /// One character on `prontera.gat` at (100, 100). Each write waits for a `gate` permit.
    pub(crate) struct FakeRepo {
        chars: Mutex<HashMap<CharId, CharacterSummary>>,
        pub(crate) writes: Mutex<Vec<(CharId, String, MapPosition)>>,
        pub(crate) finds: AtomicUsize,
        pub(crate) gate: Semaphore,
    }

    impl FakeRepo {
        pub(crate) fn new(open_gate: bool) -> Arc<Self> {
            let summary = CharacterSummary {
                char_id: CHAR,
                account_id: ACCOUNT,
                slot: 0,
                name: "Char1".into(),
                sex: Sex::Female,
                job_class: 0,
                base_level: 1,
                job_level: 1,
                hair_style: 1,
                hair_color: 1,
                last_map: "prontera.gat".into(),
                last_pos: at(100, 100),
                delete_date: None,
            };
            let permits = if open_gate { Semaphore::MAX_PERMITS } else { 0 };
            Arc::new(Self {
                chars: Mutex::new(HashMap::from([(CHAR, summary)])),
                writes: Mutex::new(Vec::new()),
                finds: AtomicUsize::new(0),
                gate: Semaphore::new(permits),
            })
        }

        pub(crate) fn writes(&self) -> Vec<(CharId, String, MapPosition)> {
            lock(&self.writes).clone()
        }
    }

    #[async_trait]
    impl CharacterRepository for FakeRepo {
        async fn list_by_account(
            &self,
            _account_id: AccountId,
        ) -> Result<Vec<CharacterSummary>, RepositoryError> {
            Ok(Vec::new())
        }
        async fn find_by_slot(
            &self,
            _account_id: AccountId,
            _slot: u8,
        ) -> Result<Option<CharacterSummary>, RepositoryError> {
            Ok(None)
        }
        async fn find_by_id(
            &self,
            id: CharId,
        ) -> Result<Option<CharacterSummary>, RepositoryError> {
            self.finds.fetch_add(1, Ordering::Relaxed);
            Ok(lock(&self.chars).get(&id).cloned())
        }
        async fn create_character(
            &self,
            _req: CreateCharacterRequest,
            _max_slots: u8,
        ) -> Result<CharacterSummary, CreateCharacterError> {
            Err(CreateCharacterError::CreationDenied)
        }
        async fn reserve_delete(
            &self,
            _account_id: AccountId,
            _char_id: CharId,
            _delay_seconds: u32,
        ) -> Result<DeleteReserveOutcome, RepositoryError> {
            Ok(DeleteReserveOutcome::Denied)
        }
        async fn cancel_delete(
            &self,
            _account_id: AccountId,
            _char_id: CharId,
        ) -> Result<CancelDeleteOutcome, RepositoryError> {
            Ok(CancelDeleteOutcome::NotFound)
        }
        async fn update_position(
            &self,
            id: CharId,
            map_name: &str,
            pos: MapPosition,
        ) -> Result<(), RepositoryError> {
            self.gate.acquire().await.expect("gate open").forget();
            lock(&self.writes).push((id, map_name.to_string(), pos));
            if let Some(c) = lock(&self.chars).get_mut(&id) {
                c.last_map = map_name.to_string();
                c.last_pos = pos;
            }
            Ok(())
        }
    }

    /// Polls `cond` every 5 ms for up to 2 s.
    pub(crate) async fn eventually(cond: impl Fn() -> bool) -> bool {
        for _ in 0..400 {
            if cond() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        false
    }

    fn state() -> (Arc<SharedState>, mpsc::UnboundedReceiver<PositionWrite>) {
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
        assert_eq!(
            (write.char_id, write.map.as_str(), write.pos),
            (CHAR, "prontera.gat", at(5, 6))
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
}
