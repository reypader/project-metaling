//! Counts against the real rAthena files, when RATHENA_PATH is set (TOOL-ITEM-05).

use ro_core::ItemId;
use ro_db::{AmmoType, ItemData, ItemDb, ItemType, WeaponType, ALL_JOBS};

use crate::common::*;

/// TOOL-ITEM-05: the real import matches the catalog counts (skipped without `RATHENA_PATH`).
#[test]
fn tool_item_05_real_counts() {
    let Some(root) = rathena() else {
        eprintln!("RATHENA_PATH not set; skipping");
        return;
    };
    let out = item_import::import(&root).expect("imports");
    let counts: Vec<usize> = out.files.iter().map(Vec::len).collect();
    assert_eq!(
        counts,
        vec![6415, 12982, 9959],
        "catalog db/item_db.md entry counts"
    );
    let out_warnings = out.report.warnings;
    let all: Vec<ItemData> = out.files.into_iter().flatten().collect();
    let db = ItemDb::from_items("import", all).expect("unique ids and aegis names");
    let count = |t| db.iter().filter(|i| i.item_type == t).count();
    // Catalog counts minus the load rules: DelayConsume joins Usable; some equipment has no
    // Locations and turns Etc.
    assert_eq!(count(ItemType::Healing), 463);
    assert_eq!(count(ItemType::Usable), 2800 + 742);
    assert_eq!(count(ItemType::Cash), 2410);
    assert_eq!(count(ItemType::Card), 5593);
    assert_eq!(count(ItemType::PetEgg), 149);
    assert!(count(ItemType::Weapon) <= 2806 && count(ItemType::Armor) <= 8954);
    assert_eq!(db.get(ItemId(501)).expect("Red Potion").weight, 70);
    assert_eq!(db.get(ItemId(909)).expect("Jellopy").weight, 10);
    let knife = db.get(ItemId(1201)).expect("Knife");
    assert_eq!((knife.weight, knife.locations, knife.slots), (400, 0x2, 3));
    assert_eq!(db.get(ItemId(2101)).expect("Guard").view, 1);
    assert_eq!(
        (
            knife.weapon_type,
            knife.attack,
            knife.range,
            knife.weapon_level
        ),
        (WeaponType::Dagger, 17, 1, 1)
    );
    assert_ne!(knife.jobs, ALL_JOBS, "the Knife has a job list");
    assert_eq!(db.get(ItemId(2101)).expect("Guard").defense, 20);
    assert_eq!(
        db.get(ItemId(1750)).expect("Arrow").ammo_type,
        Some(AmmoType::Arrow)
    );
    assert_eq!(
        db.get(ItemId(1701)).expect("Bow").weapon_type,
        WeaponType::Bow
    );
    let unknown_jobs: Vec<&String> = out_warnings
        .iter()
        .filter(|w| w.contains("unknown job"))
        .collect();
    assert!(
        unknown_jobs.is_empty(),
        "every shipped job name resolves: {unknown_jobs:?}"
    );
    // Catalog `scripting/items.md` §5: 288 whole-script heals.
    assert_eq!(
        (out.report.lifted_item_heal, out.report.lifted_percent_heal),
        (212, 76)
    );
}
