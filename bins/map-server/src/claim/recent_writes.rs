//! Writes not yet landed, so a re-entry sees the newest state.

use std::sync::Arc;

use ro_core::MapPosition;

use super::{
    shared_state::lock,
    write_dto::{RecentMap, RecentPlay, Write, WriteKind},
};

/// Writes not yet landed, by kind.
#[derive(Default)]
pub(super) struct RecentWrites {
    pub(super) positions: RecentMap<(String, MapPosition)>,
    pub(super) plays: RecentMap<RecentPlay>,
}

impl RecentWrites {
    pub(super) fn clone_handles(&self) -> Self {
        Self {
            positions: Arc::clone(&self.positions),
            plays: Arc::clone(&self.plays),
        }
    }

    /// Whether `write` is still the newest of its kind for its character.
    pub(super) fn is_newest(&self, write: &Write) -> bool {
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
    pub(super) fn landed(&self, write: &Write) {
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
