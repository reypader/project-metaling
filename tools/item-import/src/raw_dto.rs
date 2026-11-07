//! rAthena item entries as written, and how a repeated id merges into the earlier entry.

use std::collections::BTreeMap;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RathenaFile {
    #[serde(default)]
    pub(crate) body: Vec<RawItem>,
}

/// One rAthena entry. Every key is optional so a repeated id can merge only what it lists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawItem {
    pub(crate) id: u32,
    pub(crate) aegis_name: Option<String>,
    pub(crate) name: Option<String>,
    #[serde(rename = "Type")]
    pub(crate) item_type: Option<String>,
    pub(crate) weight: Option<u32>,
    pub(crate) slots: Option<u16>,
    pub(crate) view: Option<u32>,
    pub(crate) alias_name: Option<String>,
    pub(crate) locations: Option<BTreeMap<String, bool>>,
    pub(crate) sub_type: Option<String>,
    pub(crate) attack: Option<u32>,
    pub(crate) magic_attack: Option<u32>,
    pub(crate) defense: Option<u32>,
    pub(crate) range: Option<u32>,
    pub(crate) weapon_level: Option<u32>,
    pub(crate) armor_level: Option<u32>,
    pub(crate) equip_level_min: Option<u32>,
    pub(crate) equip_level_max: Option<u32>,
    pub(crate) gender: Option<String>,
    /// Replaced whole by a later entry (rAthena resets the bitsets when `Jobs` is present).
    pub(crate) jobs: Option<Switches>,
    /// Edited by a later entry: each entry's switches apply on top of the earlier value.
    pub(crate) classes: Option<Switches>,
    pub(crate) refineable: Option<bool>,
    pub(crate) flags: Option<RawFlags>,
    pub(crate) stack: Option<RawStack>,
    pub(crate) trade: Option<RawTrade>,
    pub(crate) script: Option<String>,
}

/// A YAML map of `name: bool`, in file order.
#[derive(Debug, Clone, Default)]
pub(crate) struct Switches(pub(crate) Vec<(String, bool)>);

impl<'de> Deserialize<'de> for Switches {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SwitchesVisitor;
        impl<'de> Visitor<'de> for SwitchesVisitor {
            type Value = Switches;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map of names to booleans")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Switches, A::Error> {
                let mut list = Vec::new();
                while let Some(entry) = map.next_entry::<String, bool>()? {
                    list.push(entry);
                }
                Ok(Switches(list))
            }
        }
        deserializer.deserialize_map(SwitchesVisitor)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawFlags {
    pub(crate) unique_id: Option<bool>,
    pub(crate) bind_on_equip: Option<bool>,
    pub(crate) drop_effect: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawStack {
    pub(crate) amount: Option<u16>,
    pub(crate) inventory: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawTrade {
    #[serde(rename = "Override")]
    pub(crate) override_level: Option<u16>,
    pub(crate) no_drop: Option<bool>,
}

/// `later` replaces each field it has; `None` keeps the earlier value.
pub(crate) fn take<T>(earlier: &mut Option<T>, later: Option<T>) {
    if later.is_some() {
        *earlier = later;
    }
}

impl RawItem {
    pub(crate) fn merge(&mut self, later: RawItem) {
        take(&mut self.aegis_name, later.aegis_name);
        take(&mut self.name, later.name);
        take(&mut self.item_type, later.item_type);
        take(&mut self.weight, later.weight);
        take(&mut self.slots, later.slots);
        take(&mut self.view, later.view);
        take(&mut self.alias_name, later.alias_name);
        take(&mut self.sub_type, later.sub_type);
        take(&mut self.attack, later.attack);
        take(&mut self.magic_attack, later.magic_attack);
        take(&mut self.defense, later.defense);
        take(&mut self.range, later.range);
        take(&mut self.weapon_level, later.weapon_level);
        take(&mut self.armor_level, later.armor_level);
        take(&mut self.equip_level_min, later.equip_level_min);
        take(&mut self.equip_level_max, later.equip_level_max);
        take(&mut self.gender, later.gender);
        take(&mut self.jobs, later.jobs);
        take(&mut self.refineable, later.refineable);
        take(&mut self.script, later.script);
        if let Some(Switches(later_classes)) = later.classes {
            // An earlier entry without `Classes` stood for every class.
            let mine = self
                .classes
                .get_or_insert_with(|| Switches(vec![("All".into(), true)]));
            mine.0.extend(later_classes);
        }
        if let Some(locations) = later.locations {
            self.locations
                .get_or_insert_with(BTreeMap::new)
                .extend(locations);
        }
        if let Some(flags) = later.flags {
            let mine = self.flags.get_or_insert_with(RawFlags::default);
            take(&mut mine.unique_id, flags.unique_id);
            take(&mut mine.bind_on_equip, flags.bind_on_equip);
            take(&mut mine.drop_effect, flags.drop_effect);
        }
        if let Some(stack) = later.stack {
            let mine = self.stack.get_or_insert_with(RawStack::default);
            take(&mut mine.amount, stack.amount);
            take(&mut mine.inventory, stack.inventory);
        }
        if let Some(trade) = later.trade {
            let mine = self.trade.get_or_insert_with(RawTrade::default);
            take(&mut mine.override_level, trade.override_level);
            take(&mut mine.no_drop, trade.no_drop);
        }
    }
}
