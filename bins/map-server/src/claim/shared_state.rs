//! State shared by the service handle and every map actor: the online set and recent writes.

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use ro_core::{CharId, MapPosition};
use ro_engine::PlaySnapshot;
use tokio::sync::mpsc;
use tracing::error;

use super::{
    handle::Claim,
    map_repository::MapRepository,
    recent_writes::RecentWrites,
    write_dto::{Recent, RecentPlay, Write, WriteKind},
    writer::run_writer,
};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// State shared by the service handle and every map actor.
pub(crate) struct SharedState {
    /// Characters claimed by an entering or joined session, on any map.
    pub(super) online: Mutex<HashSet<CharId>>,
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
    pub(super) fn with_writer(writer_tx: mpsc::UnboundedSender<Write>) -> Self {
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
    pub(super) fn release(&self, char_id: CharId) -> bool {
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

    /// Records the inventory, progress and skills for a re-entry that races the write, then
    /// queues the write. A re-entry rebuilds the looks from the worn gear, so they aren't kept.
    pub(crate) fn persist_play(&self, char_id: CharId, snapshot: PlaySnapshot) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        lock(&self.recent.plays).insert(
            char_id,
            Recent {
                value: RecentPlay {
                    inventory: snapshot.inventory.clone(),
                    progress: snapshot.progress,
                    skills: snapshot.skills.clone(),
                },
                seq,
            },
        );
        self.queue(char_id, WriteKind::Play(snapshot), seq);
    }
}
