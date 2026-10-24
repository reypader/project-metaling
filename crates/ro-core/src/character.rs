//! Character spawn snapshots and creation requests.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, CharId, CharacterSummary, MapPosition, Sex};
use crate::item::CharacterInventory;

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
    /// Base STR (max weight).
    pub str: u16,
    /// Account permission group; 99 may use every GM command.
    pub group_id: i32,
    pub inventory: CharacterInventory,
}

impl CharacterSpawn {
    /// Builds a spawn at the summary's last position with the default walk speed, STR 1, group 0
    /// and an empty inventory; the map-server fills those from its play-state load.
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
            str: 1,
            group_id: 0,
            inventory: CharacterInventory::default(),
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
