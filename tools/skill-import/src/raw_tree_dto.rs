//! `skill_tree.yml` entries as rAthena writes them.

use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawJob {
    pub(crate) job: String,
    #[serde(default)]
    pub(crate) inherit: Ordered,
    #[serde(default)]
    pub(crate) tree: Vec<RawTreeEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawTreeEntry {
    pub(crate) name: String,
    pub(crate) max_level: Option<u8>,
    pub(crate) exclude: Option<bool>,
    pub(crate) base_level: Option<u16>,
    pub(crate) job_level: Option<u16>,
    pub(crate) requires: Option<Vec<RawRequirement>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawRequirement {
    pub(crate) name: String,
    pub(crate) level: u8,
}

/// `Inherit`, in file order: a later parent wins a skill both parents define.
#[derive(Debug, Default)]
pub(crate) struct Ordered(pub(crate) Vec<(String, bool)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of job names to booleans")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, bool>()? {
                    out.push((key, value));
                }
                Ok(Ordered(out))
            }
        }
        deserializer.deserialize_map(OrderedVisitor)
    }
}
