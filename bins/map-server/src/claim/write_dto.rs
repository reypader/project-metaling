//! Queued database writes and the values a re-entry reads before they land.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ro_core::{CharId, CharacterInventory, CharacterProgress, LearnedSkill, MapPosition};
use ro_engine::PlaySnapshot;

/// A value not yet confirmed written, tagged with its write's sequence number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Recent<T> {
    pub(super) value: T,
    pub(super) seq: u64,
}

pub(super) type RecentMap<T> = Arc<Mutex<HashMap<CharId, Recent<T>>>>;

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
    pub(crate) skills: Vec<LearnedSkill>,
}
