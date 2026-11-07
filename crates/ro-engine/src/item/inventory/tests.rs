use ro_core::{CharId, CharacterInventory, InventoryItem, ItemId, INVENTORY_SLOTS};

use super::player_inventory::{ADD_NO_SLOT, ADD_OVERWEIGHT, ADD_STACK_LIMIT};
use super::*;
use crate::equipment::{EQP_HAND_L, EQP_HAND_R};
use crate::item::test_support::test_items;

/// A Novice's max weight at `str` (job `MaxWeight` 20000 + 300 per STR).
fn inv(str: u16) -> PlayerInventory {
    PlayerInventory::new(
        CharacterInventory::default(),
        CharId(150_000),
        20_000 + 300 * u32::from(str),
        &test_items(),
    )
}

fn item(id: u32) -> InventoryItem {
    InventoryItem::new(ItemId(id), 1)
}

fn worn(id: u32, equip: u32) -> Option<InventoryItem> {
    let mut item = item(id);
    item.equip = equip;
    Some(item)
}

fn looks(slots: Vec<Option<InventoryItem>>) -> (u32, u32) {
    let items = test_items();
    let inventory = CharacterInventory {
        slots,
        unique_item_counter: 0,
    };
    let looks = PlayerInventory::new(inventory, CharId(150_000), 20_300, &items)
        .gear(&items)
        .unit_looks();
    (looks.weapon, looks.shield)
}

/// ENG-SET-08: the `0x01d7` hand looks: empty hands, an unworn weapon, a weapon and a
/// shield (an unknown id falls back to the id), a two-handed weapon.
#[test]
fn hand_looks_follow_the_worn_hands() {
    assert_eq!(looks(vec![]), (0, 0));
    assert_eq!(looks(vec![Some(item(1201))]), (0, 0), "not worn");
    assert_eq!(
        looks(vec![worn(2101, EQP_HAND_L), None, worn(1201, EQP_HAND_R)]),
        (1201, 2101)
    );
    assert_eq!(
        looks(vec![worn(1201, EQP_HAND_R | EQP_HAND_L)]),
        (1201, 0),
        "both hands from one slot"
    );
}

/// A position claimed by two loaded rows stays with the first; the second loads unworn.
#[test]
fn conflicting_worn_rows_load_first_wins() {
    let items = test_items();
    let inventory = CharacterInventory {
        slots: vec![worn(1201, EQP_HAND_R), worn(1201, EQP_HAND_R)],
        unique_item_counter: 0,
    };
    let inv = PlayerInventory::new(inventory, CharId(150_000), 20_300, &items);
    assert_eq!(inv.worn_at(EQP_HAND_R), Some(0));
    let equips: Vec<u32> = inv.snapshot().items().map(|i| i.equip).collect();
    assert_eq!(equips, [EQP_HAND_R, 0]);
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
    let mut light = PlayerInventory::new(
        CharacterInventory::default(),
        CharId(1),
        20_000 + 300 * 10_000,
        &items,
    );
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

/// ENG-INV-05: the max weight given; unknown ids weigh 0, list as Etc and are kept. (The max
/// weight formula is `ro_mechanics::calc_status`'s.)
#[test]
fn eng_inv_05_max_weight_and_unknown_ids() {
    let items = test_items();
    let stored = CharacterInventory {
        slots: vec![
            Some(InventoryItem::new(ItemId(4_000_000), 3)),
            Some(InventoryItem::new(ItemId(501), 1)),
        ],
        unique_item_counter: 5,
    };
    let inv = PlayerInventory::new(stored.clone(), CharId(1), 23_300, &items);
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
