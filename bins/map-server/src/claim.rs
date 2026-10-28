//! Character claims and the ordered writer (specs `specs/warp-map-change/spec.md` §1,
//! `specs/items/spec.md` §2.8).
//!
//! Each online character has exactly one `Claim`. Dropping it is the only release: it records
//! the last known position (and play state) for a fast re-entry, queues the database writes, and
//! only then frees the character. One writer task runs every write in queue order, so a
//! character's position and play-state writes never reorder.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::Duration,
};

use ro_core::{CharId, CharacterInventory, CharacterProgress, MapPosition};
use ro_engine::PlaySnapshot;
use ro_persistence::{CharacterRepository, InventoryRepository, RepositoryError};
use tokio::sync::mpsc;
use tracing::{error, warn};

/// Everything the map-server reads and writes in the database.
pub trait MapRepository: CharacterRepository + InventoryRepository {}

impl<T: CharacterRepository + InventoryRepository + ?Sized> MapRepository for T {}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A value not yet confirmed written, tagged with its write's sequence number.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recent<T> {
    value: T,
    seq: u64,
}

type RecentMap<T> = Arc<Mutex<HashMap<CharId, Recent<T>>>>;

/// What one queued write stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WriteKind {
    Position {
        map: String,
        pos: MapPosition,
    },
    /// The progress, the look columns and the inventory, in one transaction.
    Play(PlaySnapshot),
}

/// One queued database write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Write {
    pub(crate) char_id: CharId,
    pub(crate) kind: WriteKind,
    pub(crate) seq: u64,
}

/// The part of a play-state write a re-entry needs before it lands (the looks are rebuilt from
/// the worn gear).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecentPlay {
    pub(crate) inventory: CharacterInventory,
    pub(crate) progress: CharacterProgress,
}

/// Writes not yet landed, by kind.
#[derive(Default)]
struct RecentWrites {
    positions: RecentMap<(String, MapPosition)>,
    plays: RecentMap<RecentPlay>,
}

impl RecentWrites {
    fn clone_handles(&self) -> Self {
        Self {
            positions: Arc::clone(&self.positions),
            plays: Arc::clone(&self.plays),
        }
    }

    /// Whether `write` is still the newest of its kind for its character.
    fn is_newest(&self, write: &Write) -> bool {
        match &write.kind {
            WriteKind::Position { .. } => lock(&self.positions)
                .get(&write.char_id)
                .is_some_and(|r| r.seq == write.seq),
            WriteKind::Play(_) => lock(&self.plays)
                .get(&write.char_id)
                .is_some_and(|r| r.seq == write.seq),
        }
    }

    /// Forgets the recent value if `write` is still the newest of its kind.
    fn landed(&self, write: &Write) {
        match &write.kind {
            WriteKind::Position { .. } => {
                let mut recent = lock(&self.positions);
                if recent
                    .get(&write.char_id)
                    .is_some_and(|r| r.seq == write.seq)
                {
                    recent.remove(&write.char_id);
                }
            }
            WriteKind::Play(_) => {
                let mut recent = lock(&self.plays);
                if recent
                    .get(&write.char_id)
                    .is_some_and(|r| r.seq == write.seq)
                {
                    recent.remove(&write.char_id);
                }
            }
        }
    }
}

/// State shared by the service handle and every map actor.
pub(crate) struct SharedState {
    /// Characters claimed by an entering or joined session, on any map.
    online: Mutex<HashSet<CharId>>,
    /// Last position and play state of characters whose write hasn't landed yet.
    recent: RecentWrites,
    next_seq: AtomicU64,
    writer_tx: mpsc::UnboundedSender<Write>,
}

impl SharedState {
    /// Starts the process's writer on the current runtime.
    pub(crate) fn new(repo: Arc<dyn MapRepository>) -> Arc<Self> {
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();
        let state = Self::with_writer(writer_tx);
        tokio::spawn(run_writer(repo, state.recent.clone_handles(), writer_rx));
        Arc::new(state)
    }

    /// State whose writes go to `writer_tx` (tests read the other end).
    fn with_writer(writer_tx: mpsc::UnboundedSender<Write>) -> Self {
        Self {
            online: Mutex::new(HashSet::new()),
            recent: RecentWrites::default(),
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
            play: None,
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
        lock(&self.recent.positions)
            .get(&char_id)
            .map(|r| r.value.clone())
    }

    /// The newest inventory and progress of `char_id` whose write hasn't landed yet.
    pub(crate) fn recent_play(&self, char_id: CharId) -> Option<RecentPlay> {
        lock(&self.recent.plays)
            .get(&char_id)
            .map(|r| r.value.clone())
    }

    fn queue(&self, char_id: CharId, kind: WriteKind, seq: u64) {
        if self.writer_tx.send(Write { char_id, kind, seq }).is_err() {
            error!("writer gone; state of char {} not persisted", char_id.0);
        }
    }

    /// Records the position for a re-entry that races the write, then queues the write.
    pub(crate) fn persist(&self, char_id: CharId, map: String, pos: MapPosition) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        lock(&self.recent.positions).insert(
            char_id,
            Recent {
                value: (map.clone(), pos),
                seq,
            },
        );
        self.queue(char_id, WriteKind::Position { map, pos }, seq);
    }

    /// Records the inventory and progress for a re-entry that races the write, then queues the
    /// write. A re-entry rebuilds the looks from the worn gear, so they aren't kept.
    pub(crate) fn persist_play(&self, char_id: CharId, snapshot: PlaySnapshot) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        lock(&self.recent.plays).insert(
            char_id,
            Recent {
                value: RecentPlay {
                    inventory: snapshot.inventory.clone(),
                    progress: snapshot.progress,
                },
                seq,
            },
        );
        self.queue(char_id, WriteKind::Play(snapshot), seq);
    }
}

/// First and longest wait between retries of a failed write.
const RETRY_FIRST: Duration = Duration::from_millis(50);
const RETRY_MAX: Duration = Duration::from_secs(5);

async fn apply(repo: &dyn MapRepository, write: &Write) -> Result<(), RepositoryError> {
    match &write.kind {
        WriteKind::Position { map, pos } => repo.update_position(write.char_id, map, *pos).await,
        WriteKind::Play(snapshot) => {
            repo.save_play_state(
                write.char_id,
                &snapshot.inventory,
                &snapshot.looks,
                &snapshot.progress,
            )
            .await
        }
    }
}

/// Runs writes in queue order. After a write lands, its recent entry is cleared unless a newer
/// write of the same kind replaced it.
///
/// Only `Unavailable` (the database is down or busy) is retried, with backoff, until the write
/// lands or a newer write of the same kind for the same character supersedes it: the
/// char-server reads the stored map, so a lost write after a warp would send the client to the
/// wrong map. Any other error can't succeed on retry and would stall every queued write behind
/// it: `NotFound` (the character was deleted) is done, anything else is logged and dropped. A
/// dropped write keeps its recent entry, so a re-entry in this process still sees it.
async fn run_writer(
    repo: Arc<dyn MapRepository>,
    recent: RecentWrites,
    mut rx: mpsc::UnboundedReceiver<Write>,
) {
    while let Some(write) = rx.recv().await {
        let mut wait = RETRY_FIRST;
        loop {
            match apply(repo.as_ref(), &write).await {
                Ok(()) | Err(RepositoryError::NotFound) => {
                    recent.landed(&write);
                    break;
                }
                Err(RepositoryError::Unavailable(err)) => {
                    warn!(
                        "Failed to persist char {} (retrying in {:?}): {}",
                        write.char_id.0, wait, err
                    );
                    tokio::time::sleep(wait).await;
                    wait = (wait * 2).min(RETRY_MAX);
                    if !recent.is_newest(&write) {
                        // A newer write of this kind for this character is queued behind it.
                        break;
                    }
                }
                Err(err) => {
                    error!(
                        "Dropping write for char {} that can't succeed: {}",
                        write.char_id.0, err
                    );
                    break;
                }
            }
        }
    }
}

/// One character's entry in the online set. It moves with the session (into the map actor,
/// and between maps on a warp); dropping it persists `last` and then `play` (if set) and
/// releases.
pub struct Claim {
    shared: Arc<SharedState>,
    char_id: CharId,
    last: Option<(String, MapPosition)>,
    /// Boxed: only the drop path reads it, and it keeps `Claim` (moved with every join) small.
    play: Option<Box<PlaySnapshot>>,
}

impl Claim {
    pub fn char_id(&self) -> CharId {
        self.char_id
    }

    /// Records the newest known position, written when the claim drops.
    pub(crate) fn set_last(&mut self, map: &str, pos: MapPosition) {
        self.last = Some((map.to_string(), pos));
    }

    /// Records the play state, written after the position when the claim drops.
    pub(crate) fn set_play(&mut self, snapshot: PlaySnapshot) {
        self.play = Some(Box::new(snapshot));
    }

    /// Queues a play-state write now (map change, autosave, `@job`); the claim stays held.
    pub(crate) fn save_play(&self, snapshot: PlaySnapshot) {
        self.shared.persist_play(self.char_id, snapshot);
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        // Record before release: a login that claims right after sees the new state.
        if let Some((map, pos)) = self.last.take() {
            self.shared.persist(self.char_id, map, pos);
        }
        if let Some(snapshot) = self.play.take() {
            self.shared.persist_play(self.char_id, *snapshot);
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
    use ro_core::{
        AccountId, CharacterLooks, CharacterSummary, CreateCharacterRequest, InventoryItem, ItemId,
        Sex,
    };
    use ro_persistence::{
        CancelDeleteOutcome, CreateCharacterError, DeleteReserveOutcome, PlayState, RepositoryError,
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
        /// Writes that fail before one succeeds.
        pub(crate) failures: AtomicUsize,
        /// Stored inventories and the inventory saves, in order.
        pub(crate) inventories: Mutex<HashMap<CharId, CharacterInventory>>,
        pub(crate) inventory_saves: Mutex<Vec<(CharId, CharacterInventory)>>,
        /// The looks of the last inventory save, by character.
        pub(crate) looks: Mutex<HashMap<CharId, CharacterLooks>>,
        /// The error every inventory save returns, if set.
        pub(crate) inventory_error: Mutex<Option<RepositoryError>>,
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
                looks: CharacterLooks::default(),
            };
            let permits = if open_gate { Semaphore::MAX_PERMITS } else { 0 };
            Arc::new(Self {
                chars: Mutex::new(HashMap::from([(CHAR, summary)])),
                writes: Mutex::new(Vec::new()),
                finds: AtomicUsize::new(0),
                gate: Semaphore::new(permits),
                failures: AtomicUsize::new(0),
                inventories: Mutex::new(HashMap::new()),
                inventory_saves: Mutex::new(Vec::new()),
                looks: Mutex::new(HashMap::new()),
                inventory_error: Mutex::new(None),
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
            if self
                .failures
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(RepositoryError::Unavailable("injected failure".into()));
            }
            lock(&self.writes).push((id, map_name.to_string(), pos));
            if let Some(c) = lock(&self.chars).get_mut(&id) {
                c.last_map = map_name.to_string();
                c.last_pos = pos;
            }
            Ok(())
        }
    }

    #[async_trait]
    impl InventoryRepository for FakeRepo {
        async fn load_play_state(
            &self,
            char_id: CharId,
        ) -> Result<Option<PlayState>, RepositoryError> {
            if !lock(&self.chars).contains_key(&char_id) {
                return Ok(None);
            }
            Ok(Some(PlayState {
                status: Default::default(),
                group_id: 0,
                inventory: lock(&self.inventories)
                    .get(&char_id)
                    .cloned()
                    .unwrap_or_default(),
            }))
        }

        async fn save_play_state(
            &self,
            char_id: CharId,
            inventory: &CharacterInventory,
            looks: &CharacterLooks,
            _progress: &CharacterProgress,
        ) -> Result<(), RepositoryError> {
            self.gate.acquire().await.expect("gate open").forget();
            if let Some(err) = lock(&self.inventory_error).clone() {
                return Err(err);
            }
            lock(&self.inventory_saves).push((char_id, inventory.clone()));
            lock(&self.looks).insert(char_id, *looks);
            lock(&self.inventories).insert(char_id, inventory.clone());
            Ok(())
        }
    }

    pub(crate) fn potions(n: u16) -> CharacterInventory {
        CharacterInventory {
            slots: vec![Some(InventoryItem::new(ItemId(501), n))],
            unique_item_counter: 0,
        }
    }

    /// `potions(n)` with bare looks, as the engine would snapshot it.
    pub(crate) fn snapshot(n: u16) -> PlaySnapshot {
        PlaySnapshot {
            inventory: potions(n),
            looks: CharacterLooks::default(),
            progress: CharacterProgress::default(),
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
            },
        );
        assert!(eventually(|| state.recent_play(CHAR).is_none()).await);
        assert_eq!(lock(&repo.looks).get(&CHAR), Some(&looks));
    }
}
