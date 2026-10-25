//! Core domain identifiers, newtypes, and primitives.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CharId(pub u32);

/// Unique identifier for an active network session connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sex {
    Female = 0,
    Male = 1,
    Server = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPosition {
    pub x: u16,
    pub y: u16,
    pub dir: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub char_id: CharId,
    pub account_id: AccountId,
    pub slot: u8,
    pub name: String,
    pub sex: Sex,
    pub job_class: u16,
    pub base_level: u16,
    pub job_level: u16,
    pub hair_style: u16,
    pub hair_color: u16,
    pub last_map: String,
    pub last_pos: MapPosition,
    pub delete_date: Option<u64>,
    pub looks: CharacterLooks,
}

/// The worn gear the char-select screen shows (the `characters` look columns). Not the same as
/// the map's [`UnitLooks`](crate::UnitLooks): the weapon here is a type, not an item id.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterLooks {
    /// The weapon type (rAthena `W_*`, or a dual-wield constant); 0 bare-handed.
    pub weapon_type: u16,
    /// The `View` of the armor in the left hand; 0 for nothing or a left-hand weapon.
    pub shield: u16,
    /// View ids after the costume override.
    pub head_top: u16,
    pub head_mid: u16,
    pub head_bottom: u16,
    pub robe: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharServerInfo {
    pub ip: [u8; 4],
    pub port: u16,
    pub name: String,
    pub users: u16,
    pub server_type: u16,
    pub new_server: u16,
}
