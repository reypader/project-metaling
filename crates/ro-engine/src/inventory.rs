//! A player's inventory: slots, weight, `unique_id` assignment, and the add/remove rules
//! (spec `specs/items/spec.md` §2.1; catalog `mechanics/inventory.md` §3-§5, §8).

use ro_core::{
    CharId, CharacterInventory, InventoryItem, ItemEntry, ItemId, INVENTORY_SLOTS, MAX_ITEM_AMOUNT,
};
use ro_db::{ItemData, ItemDb};

/// `e_additem_result` codes; they go on the wire unchanged (`0x0b41` `result`).
pub(crate) const ADD_INVALID: u8 = 1;
pub(crate) const ADD_OVERWEIGHT: u8 = 2;
pub(crate) const ADD_NO_SLOT: u8 = 4;
pub(crate) const ADD_OVER_AMOUNT: u8 = 5;
pub(crate) const ADD_STACK_LIMIT: u8 = 7;

/// Base max weight of every job until per-job `MaxWeight` exists (0.1 units).
const BASE_MAX_WEIGHT: u32 = 20_000;
/// Max weight per point of STR.
const MAX_WEIGHT_PER_STR: u32 = 300;
/// Wire type of an item missing from the registry (Etc).
const UNKNOWN_ITEM_TYPE: u8 = 3;
/// `EQP_VISIBLE`: heads, garment and costumes, the positions with a sprite.
const EQP_VISIBLE: u32 = 0x301 | 0x4 | 0x3C00;
/// `card[0]` markers: forged, created, pet egg.
const CARD0_FORGE: u32 = 0x00FF;
const CARD0_CREATE: u32 = 0x00FE;
const CARD0_PET: u32 = 0x0100;

/// `max_weight = 20000 + 300 × STR`.
pub(crate) fn max_weight(str: u16) -> u32 {
    BASE_MAX_WEIGHT + MAX_WEIGHT_PER_STR * u32::from(str)
}

/// One player's inventory component.
#[derive(Debug, Clone)]
pub(crate) struct PlayerInventory {
    /// Always `INVENTORY_SLOTS` long; `None` is a hole.
    slots: Vec<Option<InventoryItem>>,
    /// Σ Weight × amount, 0.1 units. Unknown ids weigh 0.
    weight: u32,
    max_weight: u32,
    char_id: CharId,
    unique_item_counter: u32,
    /// Bumped on every change; the map-server saves when it differs from the last save.
    revision: u64,
}

/// A successful add: where it went and how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Added {
    pub(crate) slot: usize,
    pub(crate) amount: u16,
}

fn item_weight(items: &ItemDb, id: ItemId) -> u32 {
    items.get(id).map_or(0, |d| d.weight)
}

fn is_stackable(data: Option<&ItemData>) -> bool {
    data.is_none_or(ItemData::is_stackable)
}

impl PlayerInventory {
    /// Builds the component from a loaded or transferred inventory. Slots past
    /// `INVENTORY_SLOTS` are dropped.
    pub(crate) fn new(
        inventory: CharacterInventory,
        char_id: CharId,
        str: u16,
        items: &ItemDb,
    ) -> Self {
        let mut slots = inventory.slots;
        slots.resize(INVENTORY_SLOTS, None);
        let weight = slots
            .iter()
            .flatten()
            .map(|item| item_weight(items, item.item_id).saturating_mul(u32::from(item.amount)))
            .fold(0u32, u32::saturating_add);
        Self {
            slots,
            weight,
            max_weight: max_weight(str),
            char_id,
            unique_item_counter: inventory.unique_item_counter,
            revision: 0,
        }
    }

    pub(crate) fn weight(&self) -> u32 {
        self.weight
    }

    pub(crate) fn max_weight(&self) -> u32 {
        self.max_weight
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn get(&self, slot: usize) -> Option<&InventoryItem> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// A copy for saving; trailing holes are trimmed.
    pub(crate) fn snapshot(&self) -> CharacterInventory {
        let used = self
            .slots
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        CharacterInventory {
            slots: self.slots[..used].to_vec(),
            unique_item_counter: self.unique_item_counter,
        }
    }

    /// Moves the inventory out (cross-map warp); trailing holes are trimmed.
    pub(crate) fn into_character_inventory(mut self) -> CharacterInventory {
        let used = self
            .slots
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        self.slots.truncate(used);
        CharacterInventory {
            slots: self.slots,
            unique_item_counter: self.unique_item_counter,
        }
    }

    fn next_unique_id(&mut self) -> u64 {
        let id = (u64::from(self.char_id.0) << 32) | u64::from(self.unique_item_counter);
        self.unique_item_counter = self.unique_item_counter.wrapping_add(1);
        id
    }

    /// rAthena `pc_additem`: adds `amount` of `item` (its own `amount` is ignored). Checks run in
    /// rAthena's order and the first failure returns its result code with nothing changed.
    pub(crate) fn add(
        &mut self,
        item: &InventoryItem,
        amount: u32,
        items: &ItemDb,
    ) -> Result<Added, u8> {
        if item.item_id.0 == 0 || amount == 0 {
            return Err(ADD_INVALID);
        }
        let amount = match u16::try_from(amount) {
            Ok(a) if a <= MAX_ITEM_AMOUNT => a,
            _ => return Err(ADD_OVER_AMOUNT),
        };
        let data = items.get(item.item_id);
        let stack_cap = data.filter(|d| d.stack_inventory).map(|d| d.stack_amount);
        if stack_cap.is_some_and(|cap| amount > cap) {
            return Err(ADD_STACK_LIMIT);
        }
        let added_weight = data
            .map_or(0, |d| d.weight)
            .saturating_mul(u32::from(amount));
        if u64::from(self.weight) + u64::from(added_weight) > u64::from(self.max_weight) {
            return Err(ADD_OVERWEIGHT);
        }
        let flag_unique = data.is_some_and(|d| d.unique_id);
        let stackable = is_stackable(data);

        let mut key_unique_id = item.unique_id;
        let mut generated = false;
        if flag_unique && key_unique_id == 0 {
            key_unique_id = (u64::from(self.char_id.0) << 32) | u64::from(self.unique_item_counter);
            generated = true;
        }

        if stackable && item.expire_time == 0 {
            let merge = self.slots.iter().position(|s| {
                s.as_ref().is_some_and(|s| {
                    s.item_id == item.item_id
                        && s.bound == item.bound
                        && s.unique_id == key_unique_id
                        && s.expire_time == 0
                        && s.cards == item.cards
                })
            });
            if let Some(slot) = merge {
                let Some(stack) = self.slots[slot].as_mut() else {
                    return Err(ADD_INVALID);
                };
                let limit = stack_cap.unwrap_or(MAX_ITEM_AMOUNT).min(MAX_ITEM_AMOUNT);
                if u32::from(stack.amount) + u32::from(amount) > u32::from(limit) {
                    return Err(ADD_OVER_AMOUNT);
                }
                stack.amount += amount;
                self.finish_add(added_weight, generated);
                return Ok(Added { slot, amount });
            }
        }

        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            return Err(ADD_NO_SLOT);
        };
        let mut new = item.clone();
        new.amount = amount;
        new.equip = 0;
        new.equip_switch = 0;
        new.favorite = false;
        if generated {
            new.unique_id = key_unique_id;
        } else if !stackable && new.unique_id == 0 {
            new.unique_id = self.next_unique_id();
        }
        self.slots[slot] = Some(new);
        self.finish_add(added_weight, generated);
        Ok(Added { slot, amount })
    }

    fn finish_add(&mut self, added_weight: u32, generated_unique_id: bool) {
        if generated_unique_id {
            self.unique_item_counter = self.unique_item_counter.wrapping_add(1);
        }
        self.weight = self.weight.saturating_add(added_weight);
        self.revision += 1;
    }

    /// rAthena `pc_delitem`: removes `amount` from `slot`. Returns the removed part (the stack
    /// with `amount` set). Fails on an empty slot, a zero amount or more than the stack.
    pub(crate) fn remove(
        &mut self,
        slot: usize,
        amount: u16,
        items: &ItemDb,
    ) -> Option<InventoryItem> {
        let entry = self.slots.get_mut(slot)?;
        let stack = entry.as_mut()?;
        if amount == 0 || amount > stack.amount {
            return None;
        }
        let mut removed = stack.clone();
        removed.amount = amount;
        stack.amount -= amount;
        if stack.amount == 0 {
            *entry = None;
        }
        self.weight = self
            .weight
            .saturating_sub(item_weight(items, removed.item_id).saturating_mul(u32::from(amount)));
        self.revision += 1;
        Some(removed)
    }

    /// Stackable and non-stackable list entries, in slot order.
    pub(crate) fn list_entries(&self, items: &ItemDb) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
        let mut normal = Vec::new();
        let mut equip = Vec::new();
        for (slot, item) in self.slots.iter().enumerate() {
            let Some(item) = item else { continue };
            let data = items.get(item.item_id);
            let entry = list_entry(slot, item, item.amount, data, items);
            if is_stackable(data) {
                normal.push(entry);
            } else {
                equip.push(entry);
            }
        }
        (normal, equip)
    }

    /// The `0x0b41` entry for `amount` just added at `slot`.
    pub(crate) fn pickup_entry(
        &self,
        slot: usize,
        amount: u16,
        items: &ItemDb,
    ) -> Option<ItemEntry> {
        let item = self.get(slot)?;
        let data = items.get(item.item_id);
        let mut entry = list_entry(slot, item, amount, data, items);
        let bind_on_equip = data.is_some_and(|d| d.bind_on_equip);
        entry.bind_on_equip_type = if item.bound != 0 && !is_stackable(data) {
            2
        } else {
            u16::from(bind_on_equip)
        };
        entry.look = data.map_or(0, |d| u16::try_from(d.view).unwrap_or(0));
        Some(entry)
    }
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
fn list_entry(
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ro_db::ItemType;

    pub(crate) fn data(id: u32, item_type: ItemType, weight: u32) -> ItemData {
        ItemData {
            id: ItemId(id),
            aegis_name: format!("Item_{id}"),
            name: format!("Item {id}"),
            item_type,
            weight,
            locations: if item_type.is_stackable() { 0 } else { 0x2 },
            view: 0,
            view_id: None,
            slots: 0,
            stack_amount: 0,
            stack_inventory: false,
            unique_id: false,
            bind_on_equip: false,
            drop_effect: 0,
            no_drop: false,
            no_drop_override: 100,
        }
    }

    /// 501 potion (7.0), 909 jellopy (1.0), 1201 knife (40.0), 700 capped at 3, 701 UniqueId.
    pub(crate) fn test_items() -> ItemDb {
        let mut capped = data(700, ItemType::Etc, 1);
        capped.stack_amount = 3;
        capped.stack_inventory = true;
        let mut unique = data(701, ItemType::Etc, 1);
        unique.unique_id = true;
        ItemDb::from_items(
            "test",
            vec![
                data(501, ItemType::Healing, 70),
                data(909, ItemType::Etc, 10),
                data(1201, ItemType::Weapon, 400),
                capped,
                unique,
            ],
        )
        .expect("valid test items")
    }

    fn inv(str: u16) -> PlayerInventory {
        PlayerInventory::new(
            CharacterInventory::default(),
            CharId(150_000),
            str,
            &test_items(),
        )
    }

    fn item(id: u32) -> InventoryItem {
        InventoryItem::new(ItemId(id), 1)
    }

    fn slots(inv: &PlayerInventory) -> Vec<Option<(u32, u16)>> {
        let used = inv.snapshot().slots;
        used.iter()
            .map(|s| s.as_ref().map(|i| (i.item_id.0, i.amount)))
            .collect()
    }

    /// ENG-INV-01: result codes 1, 2, 4, 5 and 7 in check order; nothing changes on failure.
    #[test]
    fn eng_inv_01_add_result_codes() {
        let items = test_items();
        let mut inv = inv(1);
        assert_eq!(inv.add(&item(0), 1, &items), Err(ADD_INVALID));
        assert_eq!(inv.add(&item(501), 0, &items), Err(ADD_INVALID));
        assert_eq!(inv.add(&item(909), 30_001, &items), Err(ADD_OVER_AMOUNT));
        // Stack limit is checked before weight: 4 > 3 even though the weight would also fit.
        assert_eq!(inv.add(&item(700), 4, &items), Err(ADD_STACK_LIMIT));
        // 20300 max: 290 potions weigh 20300 (fits), 291 don't.
        assert_eq!(inv.add(&item(501), 291, &items), Err(ADD_OVERWEIGHT));
        // Amount over 30000 is checked before weight.
        assert_eq!(inv.add(&item(501), 40_000, &items), Err(ADD_OVER_AMOUNT));
        assert_eq!((inv.weight(), inv.revision()), (0, 0), "nothing changed");
        assert_eq!(slots(&inv), vec![]);

        assert!(inv.add(&item(909), 29_000, &items).is_err(), "weight");
        let mut light =
            PlayerInventory::new(CharacterInventory::default(), CharId(1), 10_000, &items);
        assert_eq!(
            light.add(&item(909), 29_000, &items),
            Ok(Added {
                slot: 0,
                amount: 29_000
            })
        );
        let before = light.snapshot();
        assert_eq!(
            light.add(&item(909), 1001, &items),
            Err(ADD_OVER_AMOUNT),
            "merge overflow"
        );
        assert_eq!(light.snapshot(), before);
        assert_eq!(light.add(&item(700), 3, &items).map(|a| a.slot), Ok(1));
        assert_eq!(
            light.add(&item(700), 1, &items),
            Err(ADD_OVER_AMOUNT),
            "stack cap on merge"
        );

        // Fill every slot with knives: the 101st has no slot.
        for _ in 1..INVENTORY_SLOTS - 1 {
            light.add(&item(1201), 1, &items).expect("slot free");
        }
        assert_eq!(light.add(&item(1201), 1, &items), Err(ADD_NO_SLOT));
        assert_eq!(
            light.add(&item(909), 1, &items).map(|a| a.slot),
            Ok(0),
            "merge needs no slot"
        );
    }

    /// ENG-INV-02: stackables merge on the key; non-stackables take a slot each; the lowest
    /// empty slot is used; unique ids are assigned.
    #[test]
    fn eng_inv_02_merge_slots_unique_ids() {
        let items = test_items();
        let mut inv = inv(50);
        assert_eq!(inv.add(&item(501), 5, &items).map(|a| a.slot), Ok(0));
        assert_eq!(inv.add(&item(1201), 1, &items).map(|a| a.slot), Ok(1));
        assert_eq!(inv.add(&item(1201), 1, &items).map(|a| a.slot), Ok(2));
        assert_eq!(
            inv.add(&item(501), 2, &items),
            Ok(Added { slot: 0, amount: 2 })
        );
        let mut carded = item(501);
        carded.cards[0] = 4001;
        assert_eq!(
            inv.add(&carded, 1, &items).map(|a| a.slot),
            Ok(3),
            "cards differ"
        );
        let mut bound = item(501);
        bound.bound = 4;
        assert_eq!(
            inv.add(&bound, 1, &items).map(|a| a.slot),
            Ok(4),
            "bound differs"
        );
        let mut rental = item(501);
        rental.expire_time = 99;
        assert_eq!(
            inv.add(&rental, 1, &items).map(|a| a.slot),
            Ok(5),
            "rentals never merge"
        );
        let mut refined = item(501);
        refined.refine = 3;
        assert_eq!(
            inv.add(&refined, 1, &items).map(|a| a.slot),
            Ok(0),
            "refine not in the key"
        );
        assert_eq!(inv.get(0).map(|i| i.amount), Some(8));

        inv.remove(1, 1, &items).expect("knife removed");
        assert_eq!(
            inv.add(&item(909), 1, &items).map(|a| a.slot),
            Ok(1),
            "lowest hole"
        );

        let knife_ids: Vec<u64> = [2]
            .iter()
            .filter_map(|&s| inv.get(s).map(|i| i.unique_id))
            .collect();
        assert_eq!(
            knife_ids,
            vec![(150_000u64 << 32) | 1],
            "second knife got counter 1"
        );
        assert_eq!(
            inv.get(0).map(|i| i.unique_id),
            Some(0),
            "stackables get none"
        );

        assert_eq!(inv.add(&item(701), 1, &items).map(|a| a.slot), Ok(6));
        assert_eq!(
            inv.add(&item(701), 1, &items).map(|a| a.slot),
            Ok(7),
            "UniqueId never merges"
        );
        assert_eq!(
            inv.get(6).map(|i| i.unique_id),
            Some((150_000u64 << 32) | 2)
        );
        assert_eq!(
            inv.get(7).map(|i| i.unique_id),
            Some((150_000u64 << 32) | 3)
        );
        assert_eq!(inv.snapshot().unique_item_counter, 4);

        let mut worn = item(1201);
        worn.equip = 2;
        worn.equip_switch = 2;
        worn.favorite = true;
        let slot = inv.add(&worn, 1, &items).expect("added").slot;
        let added = inv.get(slot).expect("there");
        assert_eq!(
            (added.equip, added.equip_switch, added.favorite),
            (0, 0, false)
        );
    }

    /// ENG-INV-03: removal updates the weight and leaves a hole.
    #[test]
    fn eng_inv_03_remove() {
        let items = test_items();
        let mut inv = inv(1);
        inv.add(&item(501), 5, &items).expect("potions");
        inv.add(&item(909), 2, &items).expect("jellopy");
        assert_eq!(inv.weight(), 5 * 70 + 2 * 10);
        assert_eq!(inv.remove(0, 6, &items), None, "more than the stack");
        assert_eq!(inv.remove(0, 0, &items), None);
        assert_eq!(inv.remove(5, 1, &items), None, "empty slot");
        assert_eq!(inv.remove(500, 1, &items), None, "out of range");
        let removed = inv.remove(0, 2, &items).expect("partial");
        assert_eq!((removed.item_id, removed.amount), (ItemId(501), 2));
        assert_eq!(inv.weight(), 3 * 70 + 20);
        inv.remove(0, 3, &items).expect("rest");
        assert_eq!(slots(&inv), vec![None, Some((909, 2))], "hole at 0");
        assert_eq!(inv.weight(), 20);
    }

    /// ENG-INV-05: max weight from STR; unknown ids weigh 0, list as Etc and are kept.
    #[test]
    fn eng_inv_05_max_weight_and_unknown_ids() {
        assert_eq!(max_weight(1), 20_300);
        assert_eq!(max_weight(11), 23_300);
        let items = test_items();
        let stored = CharacterInventory {
            slots: vec![
                Some(InventoryItem::new(ItemId(4_000_000), 3)),
                Some(InventoryItem::new(ItemId(501), 1)),
            ],
            unique_item_counter: 5,
        };
        let inv = PlayerInventory::new(stored.clone(), CharId(1), 11, &items);
        assert_eq!((inv.weight(), inv.max_weight()), (70, 23_300));
        let (normal, equip) = inv.list_entries(&items);
        assert!(equip.is_empty());
        assert_eq!(normal[0].item_id, 4_000_000);
        assert_eq!(normal[0].item_type, 3);
        assert_eq!(normal[0].location, 0);
        assert_eq!(inv.snapshot(), stored);
    }

    #[test]
    fn wire_index_round_trip() {
        assert_eq!(wire_index(0), 2);
        assert_eq!(slot_of(2), Some(0));
        assert_eq!(slot_of(101), Some(99));
        assert_eq!(slot_of(102), None);
        assert_eq!(slot_of(1), None);
    }
}
