//! The item registry (`ITEM_DATA` YAML), produced offline by `tools/item-import` from rAthena's
//! item database. Only the fields the inventory, floor items, `@item` and equipment use.

use std::collections::HashMap;
use std::path::Path;

use ro_core::ItemId;
use serde::{Deserialize, Serialize};

use crate::yaml::{is_false, Schema};
use crate::DbLoadError;

/// `Header.Type` of an item file.
pub const ITEM_DATA_TYPE: &str = "ITEM_DATA";
/// `Header.Version` this crate reads and writes (2 added the equipment fields).
pub const ITEM_DATA_VERSION: u32 = 2;
/// `Jobs` of an item usable by every job: all bits of the three base-class sets.
pub const ALL_JOBS: [u64; 3] = [u64::MAX; 3];
/// `Classes` of an item usable by every class (renewal `ITEMJ_ALL`, seven bits).
pub const ALL_CLASSES: u8 = 0x7f;
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

/// Item type after rAthena's load rules (`DelayConsume` is already `Usable`). `Etc` is rAthena's
/// default type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Healing,
    Usable,
    #[default]
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

/// A weapon's `SubType` (rAthena `W_*`). Items that aren't weapons, and weapons without a
/// `SubType`, are `Fist`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum WeaponType {
    #[default]
    Fist,
    Dagger,
    OneHandSword,
    TwoHandSword,
    OneHandSpear,
    TwoHandSpear,
    OneHandAxe,
    TwoHandAxe,
    Mace,
    TwoHandMace,
    Staff,
    Bow,
    Knuckle,
    Musical,
    Whip,
    Book,
    Katar,
    Revolver,
    Rifle,
    Gatling,
    Shotgun,
    Grenade,
    Huuma,
    TwoHandStaff,
}

impl WeaponType {
    /// The `W_*` number (char-select `weapon` field, ASPD tables).
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// An ammo item's `SubType` (rAthena `AMMO_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmmoType {
    Arrow = 1,
    Dagger,
    Bullet,
    Shell,
    Grenade,
    Shuriken,
    Kunai,
    Cannonball,
    ThrowWeapon,
}

impl AmmoType {
    /// The `AMMO_*` number.
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// Who may wear an item (rAthena `e_sex` values: female 0, male 1, both 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Female,
    Male,
    #[default]
    Both,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn all_jobs() -> [u64; 3] {
    ALL_JOBS
}

fn is_all_jobs(v: &[u64; 3]) -> bool {
    *v == ALL_JOBS
}

fn all_classes() -> u8 {
    ALL_CLASSES
}

fn is_all_classes(v: &u8) -> bool {
    *v == ALL_CLASSES
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
    /// Weapons only; `Fist` otherwise.
    #[serde(default, skip_serializing_if = "is_default")]
    pub weapon_type: WeaponType,
    /// Ammo only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo_type: Option<AmmoType>,
    /// Weapons and ammo.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub attack: u32,
    /// Weapons; imported, unused for now.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub magic_attack: u32,
    /// Armor and shadow gear: hard DEF.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub defense: u32,
    /// Weapons: attack range in cells, at most 14.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub range: u8,
    /// Weapons: 1..=5.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub weapon_level: u8,
    /// Armor: 1 or more.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub armor_level: u8,
    /// Lowest base level that may equip it; 0 = no minimum.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub equip_level_min: u16,
    /// Highest base level that may equip it; 0 = no maximum.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub equip_level_max: u16,
    #[serde(default, skip_serializing_if = "is_default")]
    pub gender: Gender,
    /// rAthena `class_base`: base-class bitsets for the first-class, 2-1 and 2-2 branches.
    #[serde(default = "all_jobs", skip_serializing_if = "is_all_jobs")]
    pub jobs: [u64; 3],
    /// rAthena `ITEMJ_*` mask (normal, upper, baby, third, third upper, third baby, fourth).
    #[serde(default = "all_classes", skip_serializing_if = "is_all_classes")]
    pub classes: u8,
    /// Imported, unused for now.
    #[serde(default, skip_serializing_if = "is_false")]
    pub refineable: bool,
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

/// An `Etc` item with id 0 and every other field at its YAML default. For tests and builders:
/// `ItemData { id, .., ..ItemData::default() }`.
impl Default for ItemData {
    fn default() -> Self {
        Self {
            id: ItemId(0),
            aegis_name: String::new(),
            name: String::new(),
            item_type: ItemType::default(),
            weight: 0,
            locations: 0,
            view: 0,
            view_id: None,
            slots: 0,
            weapon_type: WeaponType::default(),
            ammo_type: None,
            attack: 0,
            magic_attack: 0,
            defense: 0,
            range: 0,
            weapon_level: 0,
            armor_level: 0,
            equip_level_min: 0,
            equip_level_max: 0,
            gender: Gender::default(),
            jobs: ALL_JOBS,
            classes: ALL_CLASSES,
            refineable: false,
            stack_amount: 0,
            stack_inventory: false,
            unique_id: false,
            bind_on_equip: false,
            drop_effect: 0,
            no_drop: false,
            no_drop_override: DEFAULT_NO_DROP_OVERRIDE,
        }
    }
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
