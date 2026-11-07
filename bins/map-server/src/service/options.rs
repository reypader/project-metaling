//! Tunables of a map service.

use std::time::Duration;

/// rAthena `autosave_time`: how often an online character's changed inventory is saved.
pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(300);

/// Tunables of a map service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapServiceOptions {
    /// How often an online character's inventory is saved if it changed.
    pub autosave_interval: Duration,
}

impl Default for MapServiceOptions {
    fn default() -> Self {
        Self {
            autosave_interval: AUTOSAVE_INTERVAL,
        }
    }
}
