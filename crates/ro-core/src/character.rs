//! Character spawn snapshots and creation requests.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, CharId, CharacterSummary, MapPosition, Sex};

/// Default walk speed in ms per orthogonal cell (rAthena `DEFAULT_WALK_SPEED`).
pub const DEFAULT_WALK_SPEED: u16 = 150;

/// Snapshot the map-server builds from a `CharacterSummary` plus the session to spawn a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSpawn {
    pub account_id: AccountId,
    pub char_id: CharId,
    pub name: String,
    pub sex: Sex,
    pub job_class: u16,
    pub base_level: u16,
    pub hair_style: u16,
    pub hair_color: u16,
    pub pos: MapPosition,
    /// Walk speed in ms per orthogonal cell.
    pub speed: u16,
}

impl CharacterSpawn {
    /// Builds a spawn at the summary's last position with the default walk speed.
    /// The caller must check `summary.account_id` against the session's account first.
    pub fn from_summary(summary: &CharacterSummary) -> Self {
        Self {
            account_id: summary.account_id,
            char_id: summary.char_id,
            name: summary.name.clone(),
            sex: summary.sex,
            job_class: summary.job_class,
            base_level: summary.base_level,
            hair_style: summary.hair_style,
            hair_color: summary.hair_color,
            pos: summary.last_pos,
            speed: DEFAULT_WALK_SPEED,
        }
    }
}

/// Request DTO for creating a new character on an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateCharacterRequest {
    pub account_id: AccountId,
    pub name: String,
    pub slot: u8,
    pub hair_color: u16,
    pub hair_style: u16,
    pub starting_job: u32,
    pub sex: Sex,
}
