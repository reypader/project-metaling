//! Item registry YAML (`ITEM_DATA`) loading (spec DB-ITEM-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use ro_core::ItemId;
use ro_db::{DbLoadError, ItemData, ItemDb, ItemType, ITEM_DATA_FILES};

const VALID: &str = r#"
Header:
  Type: ITEM_DATA
  Version: 1
Body:
  - Id: 501
    AegisName: Red_Potion
    Name: Red Potion
    Type: Healing
    Weight: 70
  - Id: 909
    AegisName: Jellopy
    Name: Jellopy
    Type: Etc
    Weight: 10
    StackAmount: 500
    StackInventory: true
    DropEffect: 1
  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    Weight: 400
    Locations: 2
    View: 1
    Slots: 3
    UniqueId: false
    BindOnEquip: true
    NoDrop: true
    NoDropOverride: 99
  - Id: 1202
    AegisName: Knife_
    Name: Knife
    Type: Weapon
    Weight: 400
    Locations: 2
    ViewId: 1201
"#;

fn header(body: &str) -> String {
    format!("Header:\n  Type: ITEM_DATA\n  Version: 1\nBody:\n{body}")
}

fn knife() -> ItemData {
    ItemData {
        id: ItemId(1201),
        aegis_name: "Knife".into(),
        name: "Knife".into(),
        item_type: ItemType::Weapon,
        weight: 400,
        locations: 0x2,
        view: 1,
        view_id: None,
        slots: 3,
        stack_amount: 0,
        stack_inventory: false,
        unique_id: false,
        bind_on_equip: true,
        drop_effect: 0,
        no_drop: true,
        no_drop_override: 99,
    }
}

fn load(text: &str) -> Result<ItemDb, DbLoadError> {
    let items = ItemDb::parse_file("items.yml", text)?;
    ItemDb::from_items("items.yml", items)
}

/// DB-ITEM-01: every field loads; defaults apply to omitted keys.
#[test]
fn db_item_01_loads_every_field() {
    let db = load(VALID).expect("valid file loads");
    assert_eq!(db.len(), 4);
    assert_eq!(db.get(ItemId(1201)), Some(&knife()));

    let potion = db.get(ItemId(501)).expect("Red Potion");
    assert_eq!(potion.item_type, ItemType::Healing);
    assert_eq!(potion.weight, 70);
    assert_eq!(potion.locations, 0);
    assert_eq!(potion.view_id, None);
    assert_eq!(potion.no_drop_override, 100, "Trade.Override default");
    assert!(!potion.stack_inventory && !potion.no_drop);

    let jellopy = db.get(ItemId(909)).expect("Jellopy");
    assert_eq!((jellopy.stack_amount, jellopy.stack_inventory), (500, true));
    assert_eq!(jellopy.drop_effect, 1);

    assert_eq!(
        db.get(ItemId(1202)).expect("alias").view_id,
        Some(ItemId(1201))
    );
    assert_eq!(db.get(ItemId(1202)).expect("alias").client_id(), 1201);
    assert_eq!(potion.client_id(), 501);
    assert_eq!(db.get(ItemId(4)), None);
}

/// DB-ITEM-01: lookups by id, lowercase aegis name and lowercase name.
#[test]
fn db_item_01_lookups() {
    let db = load(VALID).expect("valid file loads");
    assert_eq!(
        db.by_aegis_name("red_potion").map(|i| i.id),
        Some(ItemId(501))
    );
    assert_eq!(db.by_aegis_name("KNIFE_").map(|i| i.id), Some(ItemId(1202)));
    assert_eq!(db.by_name("red potion").map(|i| i.id), Some(ItemId(501)));
    // Two items share the name "Knife": the lowest id wins.
    assert_eq!(db.by_name("knife").map(|i| i.id), Some(ItemId(1201)));
    assert_eq!(db.by_name("nothing"), None);
    let ids: Vec<u32> = db.iter().map(|i| i.id.0).collect();
    assert_eq!(ids, vec![501, 909, 1201, 1202], "id order");
}

/// DB-ITEM-01: write then read gives the same items, and defaults are omitted.
#[test]
fn db_item_01_round_trips() {
    let items = ItemDb::parse_file("items.yml", VALID).expect("parses");
    let text = ItemDb::file_to_yaml(&items).expect("serializes");
    let again = ItemDb::parse_file("again.yml", &text).expect("reparses");
    assert_eq!(items, again);
    assert!(!text.contains("NoDrop: false"), "{text}");
    assert_eq!(
        text.matches("ViewId").count(),
        1,
        "only the alias has a ViewId: {text}"
    );
    let alias_only = ItemDb::file_to_yaml(&items[3..]).expect("serializes");
    assert!(alias_only.contains("ViewId: 1201"), "{alias_only}");
}

/// Type rules the inventory uses: stackable split and wire type.
#[test]
fn db_item_01_type_rules() {
    use ItemType::*;
    for t in [Healing, Usable, Etc, Card, Ammo, Cash] {
        assert!(t.is_stackable(), "{t:?}");
    }
    for t in [Armor, Weapon, PetEgg, PetArmor, ShadowGear] {
        assert!(!t.is_stackable(), "{t:?}");
    }
    assert_eq!(Healing.wire_type(0), 0);
    assert_eq!(Usable.wire_type(0), 2);
    assert_eq!(Etc.wire_type(0), 3);
    assert_eq!(Armor.wire_type(0x10), 4);
    assert_eq!(Weapon.wire_type(0x2), 5);
    assert_eq!(Card.wire_type(0), 6);
    assert_eq!(PetEgg.wire_type(0), 4, "pet eggs show as armor");
    assert_eq!(PetArmor.wire_type(0), 8);
    assert_eq!(Ammo.wire_type(0x8000), 10);
    assert_eq!(Cash.wire_type(0), 18);
    assert_eq!(ShadowGear.wire_type(0x20000), 5, "shadow weapon");
    assert_eq!(ShadowGear.wire_type(0x10000), 4, "other shadow gear");
}

fn expect_err(text: &str) -> DbLoadError {
    load(text).expect_err("must be rejected")
}

/// DB-ITEM-02: a wrong header is rejected.
#[test]
fn db_item_02_rejects_wrong_header() {
    for text in [
        VALID.replace("Type: ITEM_DATA", "Type: ITEM_DB"),
        VALID.replace("Version: 1", "Version: 3"),
    ] {
        assert!(matches!(expect_err(&text), DbLoadError::Schema { .. }));
    }
}

/// DB-ITEM-02: duplicate ids and aegis names (case-insensitive) and unknown keys are rejected.
#[test]
fn db_item_02_rejects_bad_entries() {
    let entry = |id: u32, aegis: &str| {
        format!("  - Id: {id}\n    AegisName: {aegis}\n    Name: x\n    Type: Etc\n    Weight: 1\n")
    };
    let dup_id = header(&(entry(7, "A") + &entry(7, "B")));
    assert!(matches!(
        expect_err(&dup_id),
        DbLoadError::DuplicateId { id: 7, .. }
    ));
    let dup_aegis = header(&(entry(7, "Apple") + &entry(8, "APPLE")));
    assert!(matches!(expect_err(&dup_aegis), DbLoadError::Schema { .. }));

    let unknown_key = header(&(entry(7, "A") + "    Script: heal\n"));
    assert!(matches!(
        expect_err(&unknown_key),
        DbLoadError::Parse { .. }
    ));
    let delay_consume = header(&entry(7, "A").replace("Type: Etc", "Type: DelayConsume"));
    assert!(
        matches!(expect_err(&delay_consume), DbLoadError::Parse { .. }),
        "DelayConsume is normalized by the importer"
    );
    let zero_alias = header(&(entry(7, "A") + "    ViewId: 0\n"));
    assert!(matches!(
        expect_err(&zero_alias),
        DbLoadError::Schema { .. }
    ));
}

/// DB-ITEM-02: a directory load needs all three files; ids are unique across them.
#[test]
fn db_item_02_load_dir() {
    let dir = std::env::temp_dir().join(format!("ro-db-item-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let [usable, equip, etc] = ITEM_DATA_FILES;
    std::fs::write(dir.join(usable), header("  - Id: 501\n    AegisName: Red_Potion\n    Name: Red Potion\n    Type: Healing\n    Weight: 70\n")).expect("write");
    std::fs::write(dir.join(equip), header("  []\n")).expect("write");
    let missing = ItemDb::load_dir(&dir).expect_err("etc file missing");
    assert!(matches!(missing, DbLoadError::Io { .. }), "{missing:?}");

    std::fs::write(dir.join(etc), header("  - Id: 909\n    AegisName: Jellopy\n    Name: Jellopy\n    Type: Etc\n    Weight: 10\n")).expect("write");
    let db = ItemDb::load_dir(&dir).expect("loads");
    assert_eq!(db.len(), 2);

    std::fs::write(
        dir.join(etc),
        header(
            "  - Id: 501\n    AegisName: Other\n    Name: Other\n    Type: Etc\n    Weight: 10\n",
        ),
    )
    .expect("write");
    let dup = ItemDb::load_dir(&dir).expect_err("id 501 is in two files");
    assert!(
        matches!(dup, DbLoadError::DuplicateId { id: 501, .. }),
        "{dup:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn shipped_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/item")
}

/// DB-ITEM-03: the shipped data loads and every file carries the license header.
#[test]
fn db_item_03_shipped_data_loads() {
    let dir = shipped_dir();
    for file in ITEM_DATA_FILES {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
        assert!(
            text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"),
            "{} lacks the SPDX header",
            path.display()
        );
        assert!(
            text.contains("rAthena") && text.contains("crates/ro-db/data/NOTICE.md"),
            "{} lacks attribution",
            path.display()
        );
    }
    let db = ItemDb::load_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        db.len() > 25_000,
        "expected the full import, found {}",
        db.len()
    );
    let potion = db.get(ItemId(501)).expect("Red Potion");
    assert_eq!((potion.item_type, potion.weight), (ItemType::Healing, 70));
    let knife = db.by_aegis_name("knife").expect("Knife");
    assert_eq!((knife.id, knife.locations), (ItemId(1201), 0x2));
}
