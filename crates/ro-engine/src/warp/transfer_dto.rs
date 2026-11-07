//! What a player carries across a map change, and the snapshot the map-server saves.

use std::sync::Arc;

use ro_core::{
    CharacterInventory, CharacterLooks, CharacterProgress, CharacterSpawn, LearnedSkill,
    MapPosition,
};

/// A player that left this map through a cross-map warp. The map-server hands it to the
/// destination map, which calls [`MapInstance::enter_by_warp`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTransfer {
    /// Identity and look, rebuilt from the entity; `pos` is `from_pos`.
    pub spawn: CharacterSpawn,
    /// Destination map as a `.gat` name.
    pub dest_map: Arc<str>,
    /// The warp's raw destination; the destination map resolves it.
    pub dest: (u16, u16),
    pub rewarp_count: u8,
    /// The cell that fired the warp, on this map.
    pub from_pos: MapPosition,
    /// Char-select looks of the worn gear, saved with `spawn.inventory`.
    pub looks: CharacterLooks,
}

/// What the map-server saves for an online character (`MapInstance::play_snapshot`), in one
/// transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaySnapshot {
    pub inventory: CharacterInventory,
    /// Char-select looks of the worn gear (written to the character's look columns).
    pub looks: CharacterLooks,
    /// Job, levels, stats, points, exp, HP/SP and the class-change levels.
    pub progress: CharacterProgress,
    /// The skill rows: visible skills with a level (hidden levels are not saved).
    pub skills: Vec<LearnedSkill>,
}
