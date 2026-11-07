//! Item fixtures shared by the engine's unit tests.

use ro_core::ItemId;
use ro_db::{ItemData, ItemDb, ItemType};

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
        ..ItemData::default()
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
