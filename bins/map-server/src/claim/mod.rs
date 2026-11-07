//! Character claims and the ordered writer (specs `specs/warp-map-change/spec.md` §1,
//! `specs/items/spec.md` §2.8).
//!
//! Each online character has exactly one `Claim`. Dropping it is the only release: it records
//! the last known position (and play state) for a fast re-entry, queues the database writes, and
//! only then frees the character. One writer task runs every write in queue order, so a
//! character's position and play-state writes never reorder.

mod handle;
mod map_repository;
mod recent_writes;
mod shared_state;
mod write_dto;
mod writer;

pub use handle::Claim;
pub use map_repository::MapRepository;
#[cfg(test)]
pub(crate) use shared_state::lock;
pub(crate) use shared_state::SharedState;

#[cfg(test)]
mod tests;
