//! Inventory wire helpers: slot/index mapping, card ids, list entries and weights.

use ro_core::{InventoryItem, ItemEntry, ItemId, INVENTORY_SLOTS};
use ro_db::{ItemData, ItemDb};

use crate::equipment::EQP_VISIBLE;
use crate::protocol_consts::UNKNOWN_ITEM_TYPE;

/// `card[0]` markers: forged, created, pet egg.
const CARD0_FORGE: u32 = 0x00FF;

const CARD0_CREATE: u32 = 0x00FE;

const CARD0_PET: u32 = 0x0100;

pub(crate) fn item_weight(items: &ItemDb, id: ItemId) -> u32 {
    items.get(id).map_or(0, |d| d.weight)
}

pub(crate) fn is_stackable(data: Option<&ItemData>) -> bool {
    data.is_none_or(ItemData::is_stackable)
}

/// Wire slot index: slot + 2.
pub(crate) fn wire_index(slot: usize) -> u16 {
    u16::try_from(slot + 2).unwrap_or(u16::MAX)
}

/// Server slot of a wire index, if in range.
pub(crate) fn slot_of(index: u16) -> Option<usize> {
    let slot = usize::from(index).checked_sub(2)?;
    (slot < INVENTORY_SLOTS).then_some(slot)
}

/// rAthena `clif_addcards`: forged and created items verbatim, pet eggs as intimacy/renamed,
/// otherwise each card as its client id.
fn wire_cards(cards: &[u32; 4], items: &ItemDb) -> [u32; 4] {
    match cards[0] {
        CARD0_FORGE | CARD0_CREATE => *cards,
        CARD0_PET => [0, 0, cards[3] >> 1, cards[3] & 1],
        _ => cards.map(|c| {
            if c == 0 {
                0
            } else {
                items.get(ItemId(c)).map_or(c, ItemData::client_id)
            }
        }),
    }
}

/// The inventory-list form of an item (`0x0b09` / `0x0b39`).
pub(crate) fn list_entry(
    slot: usize,
    item: &InventoryItem,
    amount: u16,
    data: Option<&ItemData>,
    items: &ItemDb,
) -> ItemEntry {
    let location = data.map_or(0, |d| d.locations);
    let bind_on_equip = data.is_some_and(|d| d.bind_on_equip);
    ItemEntry {
        index: wire_index(slot),
        item_id: data.map_or(item.item_id.0, ItemData::client_id),
        item_type: data.map_or(UNKNOWN_ITEM_TYPE, ItemData::wire_type),
        amount,
        location,
        wear_state: item.equip,
        cards: wire_cards(&item.cards, items),
        expire_time: item.expire_time,
        bind_on_equip_type: if item.bound != 0 {
            2
        } else {
            u16::from(bind_on_equip)
        },
        look: match data {
            Some(d) if location & EQP_VISIBLE != 0 => u16::try_from(d.view).unwrap_or(0),
            _ => 0,
        },
        options: item.options,
        refine: item.refine,
        grade: item.enchant_grade,
        identified: item.identified,
        damaged: item.attribute != 0,
        favorite: item.favorite,
    }
}
