//! One item database entry as stored in `item_*.yml`, with rAthena's defaults.

use ro_core::ItemId;
use serde::{Deserialize, Serialize};

use crate::yaml::{is_false, is_zero};
use crate::{AmmoType, Gender, ItemType, ItemUse, WeaponType};

/// `Jobs` of an item usable by every job: all bits of the three base-class sets.
pub const ALL_JOBS: [u64; 3] = [u64::MAX; 3];
/// `Classes` of an item usable by every class (renewal `ITEMJ_ALL`, seven bits).
pub const ALL_CLASSES: u8 = 0x7f;
/// rAthena's default `Trade.Override` GM level.
const DEFAULT_NO_DROP_OVERRIDE: u16 = 100;

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
    #[serde(default, skip_serializing_if = "is_zero")]
    pub locations: u32,
    /// Sprite id (`look` / `wItemSpriteNumber`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub view: u32,
    /// The `AliasName` target: the client is sent this id instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<ItemId>,
    /// Card slots.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub slots: u16,
    /// Weapons only; `Fist` otherwise.
    #[serde(default, skip_serializing_if = "is_default")]
    pub weapon_type: WeaponType,
    /// Ammo only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo_type: Option<AmmoType>,
    /// Weapons and ammo.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub attack: u32,
    /// Weapons; imported, unused for now.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub magic_attack: u32,
    /// Armor and shadow gear: hard DEF.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub defense: u32,
    /// Weapons: attack range in cells, at most 14.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub range: u8,
    /// Weapons: 1..=5.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub weapon_level: u8,
    /// Armor: 1 or more.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub armor_level: u8,
    /// Lowest base level that may equip it; 0 = no minimum.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub equip_level_min: u16,
    /// Highest base level that may equip it; 0 = no maximum.
    #[serde(default, skip_serializing_if = "is_zero")]
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
    #[serde(default, skip_serializing_if = "is_zero")]
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
    #[serde(default, skip_serializing_if = "is_zero")]
    pub drop_effect: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_drop: bool,
    /// GM level that may drop a `NoDrop` item.
    #[serde(
        default = "default_no_drop_override",
        skip_serializing_if = "is_default_no_drop_override"
    )]
    pub no_drop_override: u16,
    /// The use effect of a consumable; `None` when the script isn't a lifted heal.
    #[serde(rename = "Use", default, skip_serializing_if = "Option::is_none")]
    pub on_use: Option<ItemUse>,
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
            on_use: None,
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
