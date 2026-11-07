//! Skill flag groups and item costs as stored in `skill_db.yml`.

use serde::{Deserialize, Serialize};

use crate::yaml::is_false;

/// The skill flags that matter before casting exists (catalog `db/skills.md` "Flags").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillFlags {
    /// A quest ("platinum") skill: not learnable with points, not counted, not reset.
    #[serde(default, skip_serializing_if = "is_false")]
    pub quest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub wedding: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub spirit: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub npc: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub guild: bool,
    /// `range2` adds the player's Vulture's Eye level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_vulture: bool,
    /// `range2` adds the player's Snake Eye level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_snake_eye: bool,
    /// `range2` adds the player's Radius level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_radius: bool,
    /// `range2` adds a bonus from the player's Research Trap level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_research_trap: bool,
    /// `range2` is Shadow Jump's range at the player's Shadow Jump level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_shadow_jump: bool,
}

impl SkillFlags {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// rAthena `DamageFlags` (`NK_*`; catalog `db/skills.md` "DamageFlags").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct DamageFlags {
    /// A support skill: no damage step.
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_damage: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub splash: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub splash_split: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_atk_card: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_element: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_defense: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_flee: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_def_card: bool,
    /// The skill can crit.
    #[serde(default, skip_serializing_if = "is_false")]
    pub critical: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_long_card: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub simple_defense: bool,
}

impl DamageFlags {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// rAthena `CastTimeFlags` (`SKILL_CAST_*`; catalog `scripting/skills.md` §4.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct CastTimeFlags {
    /// DEX and INT don't shorten the cast.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_dex: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_status: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_item_bonus: bool,
}

impl CastTimeFlags {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// An item a cast consumes (`Requires.ItemCost`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillItemCost {
    /// Item aegis name (resolved against the item registry by the user).
    pub item: String,
    pub amount: u32,
    /// Only this level needs the item; `None` = every level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
}
