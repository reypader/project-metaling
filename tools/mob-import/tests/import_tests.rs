//! rAthena mob database conversion (spec TOOL-MOBDB-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use mob_import::{convert, render_file, Converted, SOURCE};
use ro_core::ItemId;
use ro_db::{ItemData, ItemDb, ItemType, MobClass, MobData, MobDb, MobDrop, MobId};

fn item(id: u32, aegis: &str) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: aegis.into(),
        name: aegis.into(),
        item_type: ItemType::Etc,
        weight: 1,
        locations: 0,
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

fn items() -> ItemDb {
    let list = (0..12)
        .map(|i| item(2000 + i, &format!("Item_{i}")))
        .chain([item(909, "Jellopy"), item(1202, "Knife_")])
        .collect();
    ItemDb::from_items("test", list).expect("valid items")
}

fn run(body: &str) -> Converted {
    let text = format!("Header:\n  Type: MOB_DB\n  Version: 5\nBody:\n{body}");
    convert(&text, &items()).expect("converts")
}

fn mob(out: &Converted, id: u32) -> &MobData {
    out.mobs
        .iter()
        .find(|m| m.id == MobId(id))
        .unwrap_or_else(|| panic!("mob {id} converted"))
}

fn drop(item: u32, rate: u16) -> MobDrop {
    MobDrop {
        item: ItemId(item),
        rate,
    }
}

/// TOOL-MOBDB-01: drops resolve by AegisName; unknown items are dropped with a warning; at most
/// 10; `Index` overwrites a slot; rate 0 is removed.
#[test]
fn tool_mobdb_01_drops() {
    let out = run("  - Id: 1002
    AegisName: PORING
    Name: Poring
    Level: 1
    Hp: 55
    WalkSpeed: 400
    Drops:
      - Item: jellopy
        Rate: 7000
      - Item: Knife_
        Rate: 100
        StealProtected: true
      - Item: Not_An_Item
        Rate: 50
      - Item: Item_1
        Rate: 0
      - Item: Item_2
        Rate: 20000
  - Id: 1003
    AegisName: MANY
    Name: Many
    Drops:
      - { Item: Item_0, Rate: 1 }
      - { Item: Item_1, Rate: 2 }
      - { Item: Item_2, Rate: 3 }
      - { Item: Item_3, Rate: 4 }
      - { Item: Item_4, Rate: 5 }
      - { Item: Item_5, Rate: 6 }
      - { Item: Item_6, Rate: 7 }
      - { Item: Item_7, Rate: 8 }
      - { Item: Item_8, Rate: 9 }
      - { Item: Item_9, Rate: 10 }
      - { Item: Item_10, Rate: 11 }
      - { Item: Item_11, Rate: 12, Index: 2 }
");
    let poring = mob(&out, 1002);
    assert_eq!(
        poring.drops,
        vec![drop(909, 7000), drop(1202, 100), drop(2002, 10000)],
        "db order kept, unknown and rate-0 entries removed, rate capped"
    );
    assert_eq!((poring.level, poring.hp, poring.walk_speed), (1, 55, 400));
    assert!(out
        .report
        .warnings
        .iter()
        .any(|w| w.contains("Not_An_Item")));

    let many = mob(&out, 1003);
    let rates: Vec<u16> = many.drops.iter().map(|d| d.rate).collect();
    assert_eq!(
        rates,
        vec![1, 2, 12, 4, 5, 6, 7, 8, 9, 10],
        "Index 2 overwrites; 11th dropped"
    );
    assert_eq!(many.drops[2].item, ItemId(2011));
    assert_eq!(
        (many.level, many.hp, many.walk_speed),
        (1, 1, 150),
        "defaults"
    );
}

/// TOOL-MOBDB-02: Class and Mvp are kept; MvpDrops at most 3; repeated ids merge; bad ids skip.
#[test]
fn tool_mobdb_02_class_mvp_merge() {
    let out = run("  - Id: 1038
    AegisName: OSIRIS
    Name: Osiris
    Class: Boss
    Modes:
      Mvp: true
      Detector: true
    MvpDrops:
      - { Item: Item_0, Rate: 5000 }
      - { Item: Item_1, Rate: 3000 }
      - { Item: Item_2, Rate: 500 }
      - { Item: Item_3, Rate: 400 }
  - Id: 1039
    AegisName: BAPHO_JR
    Name: Baphomet Jr.
    Class: boss
  - Id: 1040
    AegisName: EVENTY
    Name: Eventy
    Class: Event
    WalkSpeed: 5000
  - Id: 1039
    Level: 50
  - Id: 999
    AegisName: TOO_LOW
    Name: Too Low
  - Id: 1041
    AegisName: osiris
    Name: Duplicate Aegis
");
    let osiris = mob(&out, 1038);
    assert_eq!((osiris.class, osiris.mvp), (MobClass::Boss, true));
    assert_eq!(osiris.mvp_drops.len(), 3);
    assert_eq!(osiris.boss_type(), 2);
    let jr = mob(&out, 1039);
    assert_eq!((jr.class, jr.mvp, jr.level), (MobClass::Boss, false, 50));
    assert_eq!(jr.name, "Baphomet Jr.", "merge keeps unlisted keys");
    assert_eq!(mob(&out, 1040).class, MobClass::Event);
    assert_eq!(mob(&out, 1040).walk_speed, 1000, "clamped to 20..=1000");
    assert!(out
        .mobs
        .iter()
        .all(|m| m.id != MobId(999) && m.id != MobId(1041)));
    let ids: Vec<u32> = out.mobs.iter().map(|m| m.id.0).collect();
    assert_eq!(ids, vec![1038, 1039, 1040], "sorted");
    assert!(out.report.warnings.iter().any(|w| w.contains("999")));

    let text = render_file(&out.mobs, "abc123").expect("renders");
    assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
    assert!(text.contains("commit abc123") && text.contains(SOURCE));
    assert!(text.contains("tools/mob-import") && text.contains("crates/ro-db/data/NOTICE.md"));
    assert_eq!(
        MobDb::parse_file("rendered", &text).expect("reads"),
        out.mobs
    );
}

/// TOOL-MOBDB-03: the real import matches the catalog count (skipped without `RATHENA_PATH`).
#[test]
fn tool_mobdb_03_real_counts() {
    let Some(root) = std::env::var_os("RATHENA_PATH").map(PathBuf::from) else {
        eprintln!("RATHENA_PATH not set; skipping");
        return;
    };
    if !root.join(SOURCE).is_file() {
        eprintln!("no {SOURCE}; skipping");
        return;
    }
    let items_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ro-db/data/item");
    let items = ItemDb::load_dir(&items_dir).expect("generated item data");
    let out = mob_import::import(&root, &items).expect("imports");
    assert_eq!(
        out.mobs.len(),
        2675,
        "catalog db/mob_db.md live entry count"
    );
    let db = MobDb::from_mobs("import", out.mobs).expect("valid registry");
    let poring = db.get(MobId(1002)).expect("Poring");
    assert_eq!((poring.level, poring.hp, poring.walk_speed), (1, 55, 400));
    assert_eq!(poring.drops.first(), Some(&drop(909, 7000)));
    let osiris = db.get(MobId(1038)).expect("Osiris");
    assert_eq!((osiris.boss_type(), osiris.mvp_drops.len()), (2, 3));
    assert_eq!(osiris.drops.len(), 8);
    let bosses = db.iter().filter(|m| m.class == MobClass::Boss).count();
    assert_eq!(bosses, 732, "catalog Class: Boss count");
    assert_eq!(db.iter().filter(|m| m.mvp).count(), 190);
}
