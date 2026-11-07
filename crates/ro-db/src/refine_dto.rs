//! Refine table rows as stored in `refine.yml`, and the bonus they give.

use serde::{Deserialize, Serialize};

use crate::yaml::flow_list;

/// Which table an item uses: weapons by weapon level, armor by armor level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RefineGroup {
    Armor,
    Weapon,
}

/// One `(group, level)` row as stored in `refine.yml`. Entry `i` is the total at refine `i + 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct RefineRow {
    pub group: RefineGroup,
    pub level: u8,
    #[serde(serialize_with = "flow_list")]
    pub bonus: Vec<u32>,
    /// Empty when every entry is 0.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub random_bonus: Vec<u32>,
}

/// The bonus at one refine level, in 1/100 units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefineBonus {
    pub bonus: u32,
    pub random_bonus: u32,
}
