//! Character spawn snapshots and creation requests.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, CharId, CharacterSummary, MapPosition, Sex};
use crate::item::CharacterInventory;

/// Default walk speed in ms per orthogonal cell (rAthena `DEFAULT_WALK_SPEED`).
pub const DEFAULT_WALK_SPEED: u16 = 150;

/// Allocated stats, job level, unspent points, exp and HP/SP of a character (the `characters`
/// row). The base level is `CharacterSpawn::base_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterStatus {
    pub str: u16,
    pub agi: u16,
    pub vit: u16,
    pub int: u16,
    pub dex: u16,
    pub luk: u16,
    pub job_level: u16,
    pub status_point: u16,
    pub skill_point: u16,
    pub base_exp: u64,
    pub job_exp: u64,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
}

impl Default for CharacterStatus {
    /// A fresh novice: every stat 1, job level 1, 48 status points (rAthena `start_status_points`) and the
    /// schema's default HP/SP (40/11).
    fn default() -> Self {
        Self {
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
            job_level: 1,
            status_point: 48,
            skill_point: 0,
            base_exp: 0,
            job_exp: 0,
            hp: 40,
            max_hp: 40,
            sp: 11,
            max_sp: 11,
        }
    }
}

/// The persisted progress the stat flows change (specs `specs/stats/spec.md` §2.7): the map
/// engine produces it, the map-server saves it with the inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterProgress {
    pub job_class: u16,
    pub base_level: u16,
    /// Allocated stats, job level, points, exp and HP/SP.
    pub status: CharacterStatus,
}

impl Default for CharacterProgress {
    /// A fresh base-level-1 Novice.
    fn default() -> Self {
        Self {
            job_class: 0,
            base_level: 1,
            status: CharacterStatus::default(),
        }
    }
}

impl CharacterSpawn {
    /// The progress part, for saving.
    pub fn progress(&self) -> CharacterProgress {
        CharacterProgress {
            job_class: self.job_class,
            base_level: self.base_level,
            status: self.status,
        }
    }
}

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
    /// Allocated stats, points and exp; STR also sets max weight.
    pub status: CharacterStatus,
    /// Account permission group; 99 may use every GM command.
    pub group_id: i32,
    pub inventory: CharacterInventory,
}

impl CharacterSpawn {
    /// Builds a spawn at the summary's last position with the default walk speed, a fresh
    /// character's status, group 0 and an empty inventory; the map-server fills those from its
    /// play-state load.
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
            status: CharacterStatus::default(),
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
