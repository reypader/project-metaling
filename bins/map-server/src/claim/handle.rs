//! A character's claim on being online.

use std::sync::Arc;

use ro_core::{CharId, MapPosition};
use ro_engine::PlaySnapshot;

use super::shared_state::SharedState;

/// One character's entry in the online set. It moves with the session (into the map actor,
/// and between maps on a warp); dropping it persists `last` and then `play` (if set) and
/// releases.
pub struct Claim {
    pub(super) shared: Arc<SharedState>,
    pub(super) char_id: CharId,
    pub(super) last: Option<(String, MapPosition)>,
    /// Boxed: only the drop path reads it, and it keeps `Claim` (moved with every join) small.
    pub(super) play: Option<Box<PlaySnapshot>>,
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
