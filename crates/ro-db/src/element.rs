use serde::{Deserialize, Serialize};

/// rAthena element (a mob's defense element; later also weapon and ammo elements).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Element {
    #[default]
    Neutral,
    Water,
    Earth,
    Fire,
    Wind,
    Poison,
    Holy,
    Dark,
    Ghost,
    Undead,
}

/// The highest `ElementLevel`.
pub const MAX_ELEMENT_LEVEL: u8 = 4;
