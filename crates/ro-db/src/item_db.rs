//! The item registry (`ITEM_DATA` YAML), produced offline by `tools/item-import` from rAthena's
//! item database. Only the fields the inventory, floor items, `@item`, equipment and item use
//! read.

use std::collections::HashMap;
use std::path::Path;

use ro_core::ItemId;

use crate::yaml::{schema_error, Schema};
use crate::{DbLoadError, ItemData};

/// `Header.Type` of an item file.
pub const ITEM_DATA_TYPE: &str = "ITEM_DATA";
/// `Header.Version` this crate reads and writes (2 added the equipment fields, 3 the `Use`
/// effect).
pub const ITEM_DATA_VERSION: u32 = 3;
/// The files of an item data directory, in load order (mirrors rAthena's split).
pub const ITEM_DATA_FILES: [&str; 3] = ["item_usable.yml", "item_equip.yml", "item_etc.yml"];

const SCHEMA: Schema = Schema {
    db_type: ITEM_DATA_TYPE,
    version: ITEM_DATA_VERSION,
};

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
