//! Item registry YAML (`ITEM_DATA`) loading (spec DB-ITEM-01..05).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use ro_core::ItemId;
use ro_db::{
    AmmoType, DbLoadError, Gender, ItemData, ItemDb, ItemType, WeaponType, ALL_CLASSES, ALL_JOBS,
    ITEM_DATA_FILES,
};

const VALID: &str = r#"
Header:
  Type: ITEM_DATA
  Version: 2
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
    format!("Header:\n  Type: ITEM_DATA\n  Version: 2\nBody:\n{body}")
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
        ..ItemData::default()
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
        VALID.replace("Version: 2", "Version: 3"),
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

const EQUIPMENT: &str = r#"
Header:
  Type: ITEM_DATA
  Version: 2
Body:
  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    Weight: 400
    Locations: 2
    Slots: 3
    WeaponType: Dagger
    Attack: 17
    MagicAttack: 5
    Range: 1
    WeaponLevel: 1
    EquipLevelMin: 1
    EquipLevelMax: 99
    Gender: Female
    Jobs: [1, 2, 0]
    Classes: 3
    Refineable: true
  - Id: 1750
    AegisName: Arrow
    Name: Arrow
    Type: Ammo
    Weight: 1
    Locations: 32768
    AmmoType: Arrow
    Attack: 25
  - Id: 2101
    AegisName: Guard
    Name: Guard
    Type: Armor
    Weight: 300
    Locations: 32
    View: 1
    Defense: 20
    ArmorLevel: 1
  - Id: 1599
    AegisName: Plain
    Name: Plain
    Type: Weapon
    Weight: 1
    Locations: 2
"#;

/// DB-ITEM-04: the v2 equipment keys load, round-trip, and are omitted when default.
#[test]
fn db_item_04_equipment_fields_round_trip() {
    let items = ItemDb::parse_file("equip.yml", EQUIPMENT).expect("v2 file parses");
    let knife = &items[0];
    assert_eq!(knife.weapon_type, WeaponType::Dagger);
    assert_eq!(knife.weapon_type.id(), 1, "W_DAGGER");
    assert_eq!(knife.ammo_type, None);
    assert_eq!(
        (
            knife.attack,
            knife.magic_attack,
            knife.range,
            knife.weapon_level
        ),
        (17, 5, 1, 1)
    );
    assert_eq!((knife.equip_level_min, knife.equip_level_max), (1, 99));
    assert_eq!(knife.gender, Gender::Female);
    assert_eq!(knife.jobs, [1, 2, 0]);
    assert_eq!(knife.classes, 3);
    assert!(knife.refineable);

    let arrow = &items[1];
    assert_eq!(arrow.ammo_type, Some(AmmoType::Arrow));
    assert_eq!(arrow.ammo_type.map(AmmoType::id), Some(1), "AMMO_ARROW");
    assert_eq!(arrow.attack, 25);
    let guard = &items[2];
    assert_eq!((guard.defense, guard.armor_level), (20, 1));

    let plain = &items[3];
    assert_eq!(
        plain.weapon_type,
        WeaponType::Fist,
        "no WeaponType = W_FIST"
    );
    assert_eq!(plain.gender, Gender::Both);
    assert_eq!(plain.jobs, ALL_JOBS, "no Jobs = every job");
    assert_eq!(plain.classes, ALL_CLASSES, "no Classes = every class");
    assert_eq!((plain.equip_level_min, plain.equip_level_max), (0, 0));
    assert!(!plain.refineable);

    let text = ItemDb::file_to_yaml(&items).expect("serializes");
    let again = ItemDb::parse_file("again.yml", &text).expect("reparses");
    assert_eq!(items, again);
    let plain_text = ItemDb::file_to_yaml(&items[3..]).expect("serializes");
    for key in [
        "WeaponType",
        "AmmoType",
        "Attack",
        "MagicAttack",
        "Defense",
        "Range",
        "WeaponLevel",
        "ArmorLevel",
        "EquipLevel",
        "Gender",
        "Jobs",
        "Classes",
        "Refineable",
    ] {
        assert!(
            !plain_text.contains(key),
            "default {key} written: {plain_text}"
        );
    }
}

/// DB-ITEM-04: every weapon and ammo type name round-trips with its rAthena number.
#[test]
fn db_item_04_type_numbers() {
    let weapons = [
        (WeaponType::Fist, 0),
        (WeaponType::Dagger, 1),
        (WeaponType::OneHandSword, 2),
        (WeaponType::TwoHandSword, 3),
        (WeaponType::OneHandSpear, 4),
        (WeaponType::TwoHandSpear, 5),
        (WeaponType::OneHandAxe, 6),
        (WeaponType::TwoHandAxe, 7),
        (WeaponType::Mace, 8),
        (WeaponType::TwoHandMace, 9),
        (WeaponType::Staff, 10),
        (WeaponType::Bow, 11),
        (WeaponType::Knuckle, 12),
        (WeaponType::Musical, 13),
        (WeaponType::Whip, 14),
        (WeaponType::Book, 15),
        (WeaponType::Katar, 16),
        (WeaponType::Revolver, 17),
        (WeaponType::Rifle, 18),
        (WeaponType::Gatling, 19),
        (WeaponType::Shotgun, 20),
        (WeaponType::Grenade, 21),
        (WeaponType::Huuma, 22),
        (WeaponType::TwoHandStaff, 23),
    ];
    for (t, id) in weapons {
        assert_eq!(t.id(), id, "{t:?}");
    }
    let ammo = [
        (AmmoType::Arrow, 1),
        (AmmoType::Dagger, 2),
        (AmmoType::Bullet, 3),
        (AmmoType::Shell, 4),
        (AmmoType::Grenade, 5),
        (AmmoType::Shuriken, 6),
        (AmmoType::Kunai, 7),
        (AmmoType::Cannonball, 8),
        (AmmoType::ThrowWeapon, 9),
    ];
    for (t, id) in ammo {
        assert_eq!(t.id(), id, "{t:?}");
    }
}

/// DB-ITEM-05: a v1 file is rejected (the data is regenerated, no backwards compatibility).
#[test]
fn db_item_05_rejects_v1() {
    let v1 = VALID.replace("Version: 2", "Version: 1");
    assert!(matches!(expect_err(&v1), DbLoadError::Schema { .. }));
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
    assert_eq!(
        (knife.weapon_type, knife.attack, knife.range),
        (WeaponType::Dagger, 17, 1),
        "DB-ITEM-03: v2 equipment fields"
    );
    let guard = db.get(ItemId(2101)).expect("Guard");
    assert_eq!(guard.defense, 20);
    let arrow = db.get(ItemId(1750)).expect("Arrow");
    assert_eq!(arrow.ammo_type, Some(AmmoType::Arrow));
    let rings = [ItemId(2634), ItemId(2635)].map(|id| db.get(id).expect("wedding ring").gender);
    assert_eq!(
        rings,
        [Gender::Male, Gender::Female],
        "forced wedding-ring genders"
    );
}
