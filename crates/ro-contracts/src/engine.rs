//! Engine error types and session lifecycle contracts.

use crate::character::CharacterSpawn;
use crate::ids::{AccountId, CharId, MapPosition, SessionId};
use crate::packet::{ScopedServerEvent, SessionIntent};

/// Errors returned by the map engine during session and entity lifecycle interactions.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    #[error("Session not found: {0:?}")]
    UnknownSession(SessionId),

    #[error("Account not found: {0:?}")]
    UnknownAccount(AccountId),

    #[error("Character not found: {0:?}")]
    UnknownCharacter(CharId),

    #[error("Entity not found: {0}")]
    EntityNotFound(u32),

    #[error("Invalid coordinate cell ({x}, {y}) on map")]
    InvalidCell { x: u16, y: u16 },

    #[error("Path could not be found to destination ({dest_x}, {dest_y})")]
    PathNotFound { dest_x: u16, dest_y: u16 },

    #[error("Client intent rejected: {0}")]
    IntentRejected(&'static str),

    #[error("Map not found: {0}")]
    MapNotFound(String),

    #[error("Character already entered the map: {0:?}")]
    CharacterAlreadyEntered(CharId),

    #[error("World entity capacity exceeded")]
    WorldCapacityExceeded,
}

/// A single map instance's simulation, driven by the map-server actor.
///
/// All times are the actor's monotonic `now_ms`; wire ticks are `now_ms as u32`.
pub trait MapEngine: Send {
    fn map_name(&self) -> &str;

    /// Creates the entity (not yet visible; it is announced on `NotifyActorInit`).
    /// A duplicate char_id returns `Err(CharacterAlreadyEntered)`.
    fn spawn_character(
        &mut self,
        spawn: CharacterSpawn,
        now_ms: u64,
    ) -> Result<Vec<ScopedServerEvent>, EngineError>;

    /// Queues an intent for the next `step_tick`. `EnterMap` is not accepted here.
    fn push_intent(&mut self, intent: SessionIntent);

    /// Runs one tick and appends its events to `out` (the caller reuses the buffer).
    /// Bad per-intent input is logged and dropped; `Err` only for engine-wide faults.
    fn step_tick(
        &mut self,
        now_ms: u64,
        out: &mut Vec<ScopedServerEvent>,
    ) -> Result<(), EngineError>;

    /// Clears `out`, then fills it with the characters visible from `center` (Chebyshev range
    /// `AREA_SIZE`). The caller reuses `out` across calls so routing doesn't allocate per event.
    fn query_aoi_characters(&self, center: MapPosition, out: &mut Vec<CharId>);

    fn character_position(&self, char_id: CharId) -> Option<MapPosition>;

    /// Removes the entity and emits `UnitVanished{type 2}` to the AoI excluding the leaver.
    fn remove_character(&mut self, char_id: CharId) -> Result<Vec<ScopedServerEvent>, EngineError>;
}
