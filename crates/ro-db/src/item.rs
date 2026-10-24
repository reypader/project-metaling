//! The item registry (`ITEM_DATA` YAML), produced offline by `tools/item-import` from rAthena's
//! item database. Only the fields the inventory, floor items and `@item` use.

use std::collections::HashMap;
use std::path::Path;

use ro_core::ItemId;
use serde::{Deserialize, Serialize};

use crate::yaml::{is_false, Schema};
use crate::DbLoadError;

/// `Header.Type` of an item file.
pub const ITEM_DATA_TYPE: &str = "ITEM_DATA";
/// `Header.Version` this crate reads and writes.
pub const ITEM_DATA_VERSION: u32 = 1;
/// The files of an item data directory, in load order (mirrors rAthena's split).
pub const ITEM_DATA_FILES: [&str; 3] = ["item_usable.yml", "item_equip.yml", "item_etc.yml"];

/// `EQP_SHADOW_WEAPON`.
const EQP_SHADOW_WEAPON: u32 = 0x2_0000;
/// rAthena's default `Trade.Override` GM level.
const DEFAULT_NO_DROP_OVERRIDE: u16 = 100;

const SCHEMA: Schema = Schema {
    db_type: ITEM_DATA_TYPE,
    version: ITEM_DATA_VERSION,
};

/// Item type after rAthena's load rules (`DelayConsume` is already `Usable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Healing,
    Usable,
    Etc,
    Armor,
    Weapon,
    Card,
    PetEgg,
    PetArmor,
    Ammo,
    ShadowGear,
    Cash,
}

impl ItemType {
    /// Weapons, armor, pet eggs, pet armor and shadow gear take one slot per item.
    pub fn is_stackable(self) -> bool {
        !matches!(
            self,
            Self::Weapon | Self::Armor | Self::PetEgg | Self::PetArmor | Self::ShadowGear
        )
    }

    /// The `type` byte of the item packets. Pet eggs show as armor; shadow gear shows as a
    /// weapon on the shadow weapon slot, else as armor.
    pub fn wire_type(self, locations: u32) -> u8 {
        match self {
            Self::Healing => 0,
            Self::Usable => 2,
            Self::Etc => 3,
            Self::Armor | Self::PetEgg => 4,
            Self::Weapon => 5,
            Self::Card => 6,
            Self::PetArmor => 8,
            Self::Ammo => 10,
            Self::ShadowGear if locations & EQP_SHADOW_WEAPON != 0 => 5,
            Self::ShadowGear => 4,
            Self::Cash => 18,
        }
    }
}

fn default_no_drop_override() -> u16 {
    DEFAULT_NO_DROP_OVERRIDE
}

fn is_default_no_drop_override(v: &u16) -> bool {
    *v == DEFAULT_NO_DROP_OVERRIDE
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

/// One item database entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct ItemData {
    pub id: ItemId,
    pub aegis_name: String,
    pub name: String,
    #[serde(rename = "Type")]
    pub item_type: ItemType,
    /// In 0.1 weight units.
    pub weight: u32,
    /// `EQP_*` mask; 0 for non-equipment.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub locations: u32,
    /// Sprite id (`look` / `wItemSpriteNumber`).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub view: u32,
    /// The `AliasName` target: the client is sent this id instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<ItemId>,
    /// Card slots.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub slots: u16,
    /// Per-item stack cap; 0 = none.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub stack_amount: u16,
    /// The stack cap applies to the inventory.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stack_inventory: bool,
    /// Every add takes its own slot with a `unique_id`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unique_id: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bind_on_equip: bool,
    /// rAthena `e_item_drop_effect`; 0 = none.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub drop_effect: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_drop: bool,
    /// GM level that may drop a `NoDrop` item.
    #[serde(
        default = "default_no_drop_override",
        skip_serializing_if = "is_default_no_drop_override"
    )]
    pub no_drop_override: u16,
}

impl ItemData {
    pub fn is_stackable(&self) -> bool {
        self.item_type.is_stackable()
    }

    pub fn wire_type(&self) -> u8 {
        self.item_type.wire_type(self.locations)
    }

    /// The id the client is sent: the alias target when set, else the item's own id.
    pub fn client_id(&self) -> u32 {
        self.view_id.unwrap_or(self.id).0
    }
}

/// The global item registry. Lookups by id, and case-insensitively by aegis name or name.
#[derive(Debug, Clone, Default)]
pub struct ItemDb {
    /// Sorted by id.
    items: Vec<ItemData>,
    by_id: HashMap<ItemId, usize>,
    by_aegis: HashMap<String, usize>,
    /// The lowest id wins when names repeat.
    by_name: HashMap<String, usize>,
}

impl ItemDb {
    /// Parses one `ITEM_DATA` file. `path` is only a label for errors.
    pub fn parse_file(path: &str, text: &str) -> Result<Vec<ItemData>, DbLoadError> {
        SCHEMA.parse(path, text)
    }

    /// Serializes one file's items with the current header (no leading comments; the importer
    /// adds those).
    pub fn file_to_yaml(items: &[ItemData]) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(items)
    }

    /// Builds the registry. Ids and aegis names (case-insensitive) must be unique, and a
    /// `ViewId` must not be 0. `path` is only a label for errors.
    pub fn from_items(path: &str, mut items: Vec<ItemData>) -> Result<Self, DbLoadError> {
        items.sort_by_key(|item| item.id);
        let mut db = Self {
            by_id: HashMap::with_capacity(items.len()),
            by_aegis: HashMap::with_capacity(items.len()),
            by_name: HashMap::with_capacity(items.len()),
            items: Vec::new(),
        };
        for (i, item) in items.iter().enumerate() {
            if db.by_id.insert(item.id, i).is_some() {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: item.id.0,
                });
            }
            if item.view_id == Some(ItemId(0)) {
                return Err(schema_error(path, format!("item {}: ViewId 0", item.id.0)));
            }
            if db
                .by_aegis
                .insert(item.aegis_name.to_lowercase(), i)
                .is_some()
            {
                return Err(schema_error(
                    path,
                    format!("duplicate AegisName '{}'", item.aegis_name),
                ));
            }
            db.by_name.entry(item.name.to_lowercase()).or_insert(i);
        }
        db.items = items;
        Ok(db)
    }

    /// Loads every file of `ITEM_DATA_FILES` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let mut items = Vec::new();
        for file in ITEM_DATA_FILES {
            let path = dir.join(file);
            let Some(mut body) = SCHEMA.load(&path)? else {
                return Err(DbLoadError::Io {
                    path: path.display().to_string(),
                    message: "file not found".into(),
                });
            };
            items.append(&mut body);
        }
        Self::from_items(&dir.display().to_string(), items)
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemData> {
        self.by_id.get(&id).map(|&i| &self.items[i])
    }

    /// Case-insensitive aegis name lookup.
    pub fn by_aegis_name(&self, name: &str) -> Option<&ItemData> {
        self.by_aegis
            .get(&name.to_lowercase())
            .map(|&i| &self.items[i])
    }

    /// Case-insensitive display name lookup; the lowest id wins.
    pub fn by_name(&self, name: &str) -> Option<&ItemData> {
        self.by_name
            .get(&name.to_lowercase())
            .map(|&i| &self.items[i])
    }

    /// Items in id order.
    pub fn iter(&self) -> impl Iterator<Item = &ItemData> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn schema_error(path: &str, message: String) -> DbLoadError {
    DbLoadError::Schema {
        path: path.to_string(),
        message,
    }
}
