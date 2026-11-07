//! Mob classification and drop entries as stored in `mob_db.yml`, and the mob id.

use ro_core::ItemId;
use serde::{Deserialize, Serialize};

/// rAthena mob `Class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobClass {
    Normal,
    Boss,
    Guardian,
    Battlefield,
    Event,
}

/// rAthena mob `Size` (damage tables only; not what the client renders).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobSize {
    #[default]
    Small,
    Medium,
    Large,
}

/// rAthena `Race`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobRace {
    #[default]
    Formless,
    Undead,
    Brute,
    Plant,
    Insect,
    Fish,
    Demon,
    Demihuman,
    Angel,
    Dragon,
    PlayerHuman,
    PlayerDoram,
}

/// One drop entry: `item` drops when `roll % 10000 < rate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct MobDrop {
    pub item: ItemId,
    /// `1..=10000`.
    pub rate: u16,
}

/// A mob_db id. Sent to the client as the unit's `job`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MobId(pub u32);

impl MobId {
    /// rAthena's mob id ranges: `1001..=3998` and `20021..=31998`.
    pub fn is_valid(self) -> bool {
        matches!(self.0, 1001..=3998 | 20021..=31998)
    }
}
