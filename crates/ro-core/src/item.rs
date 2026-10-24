//! Items shared by persistence, the engine and the packet layer: the stored item, a character's
//! inventory snapshot, and the resolved list entry the inventory packets carry.

use serde::{Deserialize, Serialize};

/// An item database id (`nameid`). `0` means "no item".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub u32);

/// Inventory capacity (rAthena `INVENTORY_BASE_SIZE`; expansion is deferred).
pub const INVENTORY_SLOTS: usize = 100;

/// Largest stack (rAthena `MAX_AMOUNT`).
pub const MAX_ITEM_AMOUNT: u16 = 30000;

/// Card slots per item (rAthena `MAX_SLOTS`).
pub const ITEM_CARD_SLOTS: usize = 4;

/// Random option slots per item (rAthena `MAX_ITEM_RDM_OPT`).
pub const ITEM_OPTION_SLOTS: usize = 5;

/// One random option. `id == 0` is an unused slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemOption {
    pub id: i16,
    pub value: i16,
    pub param: u8,
}

/// One stored item (rAthena `struct item` without the SQL row id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryItem {
    pub item_id: ItemId,
    /// `1..=MAX_ITEM_AMOUNT`; non-stackables are always 1.
    pub amount: u16,
    /// Positions the item is worn at (`EQP_*` mask); 0 = not worn.
    pub equip: u32,
    pub identified: bool,
    pub refine: u8,
    /// Non-zero = broken.
    pub attribute: u8,
    pub cards: [u32; ITEM_CARD_SLOTS],
    pub options: [ItemOption; ITEM_OPTION_SLOTS],
    /// Unix time a rental expires; 0 = permanent.
    pub expire_time: u32,
    pub favorite: bool,
    /// rAthena `bound_type`: 0 none, 1 account, 2 guild, 3 party, 4 character.
    pub bound: u8,
    /// `(char_id << 32) | counter`; 0 = none.
    pub unique_id: u64,
    /// Equip-switch position mask; 0 = none.
    pub equip_switch: u32,
    pub enchant_grade: u8,
}

impl InventoryItem {
    /// A plain identified item with every other field zero.
    pub fn new(item_id: ItemId, amount: u16) -> Self {
        Self {
            item_id,
            amount,
            equip: 0,
            identified: true,
            refine: 0,
            attribute: 0,
            cards: [0; ITEM_CARD_SLOTS],
            options: [ItemOption::default(); ITEM_OPTION_SLOTS],
            expire_time: 0,
            favorite: false,
            bound: 0,
            unique_id: 0,
            equip_switch: 0,
            enchant_grade: 0,
        }
    }
}

/// A character's inventory as it moves between persistence, map-servers and map instances.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CharacterInventory {
    /// Slot `i` goes on the wire as index `i + 2`. `None` is a hole. At most `INVENTORY_SLOTS`
    /// long; a load from the database has no holes.
    pub slots: Vec<Option<InventoryItem>>,
    /// Next `unique_id` counter value (rAthena `uniqueitem_counter`).
    pub unique_item_counter: u32,
}

impl CharacterInventory {
    /// The stored items in slot order, holes skipped.
    pub fn items(&self) -> impl Iterator<Item = &InventoryItem> {
        self.slots.iter().flatten()
    }
}

/// One item as the inventory packets show it: the stored item resolved against the item
/// database. The engine fills the fields that differ per packet (`bind_on_equip_type`, `look`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemEntry {
    /// Wire index (slot + 2).
    pub index: u16,
    /// Client item id: the alias view id when set, else the item id.
    pub item_id: u32,
    /// Wire item type (`itemtype()`).
    pub item_type: u8,
    /// Stack amount (lists) or the amount added (`0x0b41`).
    pub amount: u16,
    /// The item database `Locations` mask.
    pub location: u32,
    /// Positions currently worn at.
    pub wear_state: u32,
    pub cards: [u32; ITEM_CARD_SLOTS],
    pub expire_time: u32,
    pub bind_on_equip_type: u16,
    /// Sprite number (`wItemSpriteNumber` in lists, `look` in `0x0b41`).
    pub look: u16,
    pub options: [ItemOption; ITEM_OPTION_SLOTS],
    pub refine: u8,
    pub grade: u8,
    pub identified: bool,
    pub damaged: bool,
    pub favorite: bool,
}

/// A floor item as `0x0add` / `0x009d` show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorItemAppearance {
    /// Floor item object id.
    pub id: u32,
    /// Client item id (alias view id or item id).
    pub item_id: u32,
    pub item_type: u8,
    pub identified: bool,
    pub x: u16,
    pub y: u16,
    pub sub_x: u8,
    pub sub_y: u8,
    pub amount: u16,
    pub show_drop_effect: bool,
    pub drop_effect_mode: u16,
}
