//! `skill_db.yml` entries as rAthena writes them, and the file wrapper both files share.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RathenaFile<T> {
    #[serde(default = "Vec::new")]
    pub(crate) body: Vec<T>,
}

/// One `skill_db.yml` entry. Keys this importer doesn't use are ignored. Shapes: catalog
/// `db/skills.md` "skill_db.yml cast/requirement fields".
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawSkill {
    pub(crate) id: u16,
    pub(crate) name: String,
    pub(crate) max_level: u8,
    pub(crate) target_type: Option<String>,
    pub(crate) range: Option<PerLevel<RawRange>>,
    pub(crate) requires: Option<RawRequires>,
    #[serde(default)]
    pub(crate) flags: BTreeMap<String, bool>,
    pub(crate) cast_time: Option<PerLevel<RawTime>>,
    pub(crate) fixed_cast_time: Option<PerLevel<RawTime>>,
    pub(crate) after_cast_act_delay: Option<PerLevel<RawTime>>,
    pub(crate) after_cast_walk_delay: Option<PerLevel<RawTime>>,
    pub(crate) cooldown: Option<PerLevel<RawTime>>,
    pub(crate) hit_count: Option<PerLevel<RawCount>>,
    pub(crate) element: Option<RawElement>,
    #[serde(default)]
    pub(crate) damage_flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub(crate) cast_time_flags: BTreeMap<String, bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawRequires {
    pub(crate) sp_cost: Option<PerLevel<RawAmount>>,
    pub(crate) hp_cost: Option<PerLevel<RawAmount>>,
    pub(crate) zeny_cost: Option<PerLevel<RawAmount>>,
    pub(crate) weapon: Option<BTreeMap<String, bool>>,
    #[serde(default)]
    pub(crate) item_cost: Vec<RawItemCost>,
    pub(crate) state: Option<String>,
}

/// rAthena's per-level keys: one value for every level, or a `{Level, value}` list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum PerLevel<T> {
    Scalar(i32),
    List(Vec<T>),
}

/// One `{Level, value}` entry of a per-level list.
pub(crate) trait Entry {
    fn pair(&self) -> (u16, i32);
}

macro_rules! entry {
    ($name:ident, $field:ident) => {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        pub(crate) struct $name {
            level: u16,
            $field: i32,
        }

        impl Entry for $name {
            fn pair(&self) -> (u16, i32) {
                (self.level, self.$field)
            }
        }
    };
}

entry!(RawRange, size);
entry!(RawAmount, amount);
entry!(RawTime, time);
entry!(RawCount, count);

/// `Element`: one name for every level, or a `{Level, Element}` list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawElement {
    Scalar(String),
    List(Vec<RawElementEntry>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawElementEntry {
    pub(crate) level: u16,
    pub(crate) element: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawItemCost {
    pub(crate) item: String,
    pub(crate) amount: i32,
    pub(crate) level: Option<u16>,
}
