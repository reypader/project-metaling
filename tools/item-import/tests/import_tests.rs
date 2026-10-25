//! rAthena item database conversion (spec TOOL-ITEM-01..08).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use item_import::{convert, render_file, Converted, SOURCES};
use ro_core::ItemId;
use ro_db::{AmmoType, Gender, ItemData, ItemDb, ItemType, WeaponType, ALL_CLASSES, ALL_JOBS};

fn db(body: &str) -> String {
    format!("Header:\n  Type: ITEM_DB\n  Version: 3\nBody:\n{body}")
}

fn run(usable: &str, equip: &str, etc: &str) -> Converted {
    let (u, q, e) = (db(usable), db(equip), db(etc));
    convert(&[("usable.yml", &u), ("equip.yml", &q), ("etc.yml", &e)]).expect("converts")
}

fn find(out: &Converted, id: u32) -> &ItemData {
    out.files
        .iter()
        .flatten()
        .find(|i| i.id == ItemId(id))
        .unwrap_or_else(|| panic!("item {id} converted"))
}

/// TOOL-ITEM-01: type rules and location masks.
#[test]
fn tool_item_01_type_rules() {
    let usable = "  - Id: 501
    AegisName: Red_Potion
    Name: Red Potion
    Type: Healing
    Weight: 70
  - Id: 12000
    AegisName: Delayed
    Name: Delayed
    Type: DelayConsume
  - Id: 12001
    AegisName: Lower
    Name: Lower
    Type: usable
";
    let equip = "  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    Weight: 400
    Locations:
      Right_Hand: true
  - Id: 1250
    AegisName: Katar
    Name: Katar
    Type: Weapon
    Locations:
      Both_Hand: true
  - Id: 2601
    AegisName: Ring
    Name: Ring
    Type: Armor
    Locations:
      Both_Accessory: true
  - Id: 2101
    AegisName: Guard
    Name: Guard
    Type: Armor
    Locations:
      Left_Hand: true
  - Id: 2102
    AegisName: Buckler
    Name: Buckler
    Type: Armor
    View: 2
    Locations:
      Left_Hand: true
  - Id: 1299
    AegisName: No_Locations
    Name: No Locations
    Type: Weapon
  - Id: 2299
    AegisName: Shadow_On_Armor
    Name: Shadow On Armor
    Type: Armor
    Locations:
      Shadow_Armor: true
  - Id: 24000
    AegisName: Shadow_Weapon_Item
    Name: Shadow Weapon Item
    Type: ShadowGear
    Locations:
      Shadow_Weapon: true
  - Id: 9001
    AegisName: Egg
    Name: Egg
    Type: PetEgg
";
    let etc = "  - Id: 909
    AegisName: Jellopy
    Name: Jellopy
    Type: Etc
    Weight: 10
  - Id: 7000
    AegisName: Bogus_Type
    Name: Bogus Type
    Type: Spaceship
  - Id: 1750
    AegisName: Arrow
    Name: Arrow
    Type: Ammo
    Locations:
      Ammo: true
";
    let out = run(usable, equip, etc);
    let t = |id| find(&out, id).item_type;
    let loc = |id| find(&out, id).locations;
    assert_eq!(t(501), ItemType::Healing);
    assert_eq!(t(12000), ItemType::Usable, "DelayConsume -> Usable");
    assert_eq!(
        t(12001),
        ItemType::Usable,
        "type names are case-insensitive"
    );
    assert_eq!((t(1201), loc(1201)), (ItemType::Weapon, 0x2));
    assert_eq!(loc(1250), 0x22, "Both_Hand");
    assert_eq!(loc(2601), 0x88, "Both_Accessory");
    assert_eq!(
        find(&out, 2101).view,
        1,
        "a shield without View shows the Guard"
    );
    assert_eq!(find(&out, 2102).view, 2);
    assert_eq!(t(1299), ItemType::Etc, "equipment without Locations");
    assert_eq!(t(2299), ItemType::Etc, "shadow position on non-shadow gear");
    assert_eq!((t(24000), loc(24000)), (ItemType::ShadowGear, 0x2_0000));
    assert_eq!(t(9001), ItemType::PetEgg, "pet eggs need no Locations");
    assert_eq!(t(7000), ItemType::Etc, "invalid type");
    assert_eq!((t(1750), loc(1750)), (ItemType::Ammo, 0x8000));
    assert!(
        out.report.warnings.iter().any(|w| w.contains("Spaceship")),
        "{:?}",
        out.report.warnings
    );
}

/// TOOL-ITEM-02: AliasName resolves to ViewId; an unknown alias warns and stays unset.
#[test]
fn tool_item_02_alias() {
    let equip = "  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    Locations:
      Right_Hand: true
  - Id: 13000
    AegisName: Knife_Costume
    Name: Knife Costume
    Type: Weapon
    AliasName: knife
    Locations:
      Right_Hand: true
  - Id: 13001
    AegisName: Lost
    Name: Lost
    Type: Weapon
    AliasName: Nowhere
    Locations:
      Right_Hand: true
";
    let out = run("", equip, "");
    assert_eq!(find(&out, 13000).view_id, Some(ItemId(1201)));
    assert_eq!(find(&out, 13001).view_id, None);
    assert!(out.report.warnings.iter().any(|w| w.contains("Nowhere")));
}

/// TOOL-ITEM-03: a Stack node without Inventory gives StackInventory false; other flags.
#[test]
fn tool_item_03_stack_and_flags() {
    let usable = "  - Id: 501
    AegisName: Capped_No_Inventory
    Name: A
    Type: Healing
    Stack:
      Amount: 10
      Cart: true
  - Id: 502
    AegisName: Capped
    Name: B
    Type: Healing
    Stack:
      Amount: 20
      Inventory: true
    Flags:
      UniqueId: true
      DropEffect: CLIENT
    Trade:
      Override: 20
      NoDrop: true
";
    let equip = "  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    Slots: 9
    Locations:
      Right_Hand: true
    Flags:
      BindOnEquip: true
      DropEffect: purple_pillar
";
    let out = run(usable, equip, "");
    let a = find(&out, 501);
    assert_eq!((a.stack_amount, a.stack_inventory), (10, false));
    let b = find(&out, 502);
    assert_eq!((b.stack_amount, b.stack_inventory), (20, true));
    assert!(b.unique_id);
    assert_eq!(b.drop_effect, 1);
    assert_eq!((b.no_drop, b.no_drop_override), (true, 20));
    let knife = find(&out, 1201);
    assert!(knife.bind_on_equip);
    assert_eq!(knife.drop_effect, 4);
    assert_eq!(knife.slots, 4, "capped at 4 card slots");
    assert_eq!(knife.no_drop_override, 100, "default override");
}

/// TOOL-ITEM-04: repeated ids merge field by field into the first file; output is sorted and
/// every file has the header.
#[test]
fn tool_item_04_merge_sort_header() {
    let usable = "  - Id: 600
    AegisName: Later
    Name: Later
    Type: Usable
  - Id: 501
    AegisName: Red_Potion
    Name: Red Potion
    Type: Healing
    Weight: 70
";
    let etc = "  - Id: 501
    Weight: 80
    Trade:
      NoDrop: true
  - Id: 777
    Name: No Aegis
  - Id: 909
    AegisName: RED_POTION
    Name: Duplicate Aegis
    Type: Etc
";
    let out = run(usable, "", etc);
    let ids: Vec<u32> = out.files[0].iter().map(|i| i.id.0).collect();
    assert_eq!(
        ids,
        vec![501, 600],
        "sorted by id, merged entry stays in its first file"
    );
    assert!(out.files[2].is_empty(), "{:?}", out.files[2]);
    let potion = find(&out, 501);
    assert_eq!(potion.name, "Red Potion", "unlisted keys keep their value");
    assert_eq!(
        (potion.weight, potion.no_drop),
        (80, true),
        "listed keys override"
    );
    assert!(out.report.warnings.iter().any(|w| w.contains("777")));
    assert!(out.report.warnings.iter().any(|w| w.contains("RED_POTION")));

    let text = render_file(&out.files[0], "abc123", SOURCES[0].0).expect("renders");
    assert!(
        text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"),
        "{text}"
    );
    assert!(text.contains("commit abc123"), "{text}");
    assert!(text.contains(SOURCES[0].0), "{text}");
    assert!(text.contains("tools/item-import"), "{text}");
    assert!(text.contains("crates/ro-db/data/NOTICE.md"), "{text}");
    let parsed = ItemDb::parse_file("rendered", &text).expect("our loader reads it");
    assert_eq!(parsed, out.files[0]);
}

/// TOOL-ITEM-06: `SubType` maps to the weapon / ammo type; equipment fields are cleared by type;
/// `Range` is capped at 14 and `WeaponLevel` clamped to 1..=5.
#[test]
fn tool_item_06_subtype_and_cleared_fields() {
    let equip = "  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    SubType: Dagger
    Attack: 17
    MagicAttack: 3
    Range: 1
    WeaponLevel: 1
    EquipLevelMin: 1
    Refineable: true
    Locations:
      Right_Hand: true
  - Id: 1116
    AegisName: Katana
    Name: Katana
    Type: Weapon
    SubType: 2hSword
    Range: 20
    WeaponLevel: 9
    Locations:
      Both_Hand: true
  - Id: 1599
    AegisName: No_Level
    Name: No Level
    Type: Weapon
    Locations:
      Right_Hand: true
  - Id: 2101
    AegisName: Guard
    Name: Guard
    Type: Armor
    Defense: 20
    Attack: 5
    Range: 3
    WeaponLevel: 2
    EquipLevelMin: 10
    EquipLevelMax: 50
    Locations:
      Left_Hand: true
  - Id: 2301
    AegisName: Cotton_Shirt
    Name: Cotton Shirt
    Type: Armor
    Defense: 10
    ArmorLevel: 1
    Locations:
      Armor: true
  - Id: 1299
    AegisName: Demoted
    Name: Demoted
    Type: Weapon
    SubType: Dagger
    Attack: 9
";
    let etc = "  - Id: 1750
    AegisName: Arrow
    Name: Arrow
    Type: Ammo
    SubType: Arrow
    Attack: 25
    Defense: 4
    Range: 2
    Locations:
      Ammo: true
  - Id: 13200
    AegisName: Bullet
    Name: Bullet
    Type: Ammo
    SubType: bullet
    Locations:
      Ammo: true
  - Id: 4001
    AegisName: Poring_Card
    Name: Poring Card
    Type: Card
    SubType: Enchant
  - Id: 909
    AegisName: Jellopy
    Name: Jellopy
    Type: Etc
    SubType: Dagger
";
    let out = run("", equip, etc);
    let knife = find(&out, 1201);
    assert_eq!(knife.weapon_type, WeaponType::Dagger);
    assert_eq!(
        (
            knife.attack,
            knife.magic_attack,
            knife.range,
            knife.weapon_level
        ),
        (17, 3, 1, 1)
    );
    assert_eq!((knife.equip_level_min, knife.refineable), (1, true));
    let katana = find(&out, 1116);
    assert_eq!(
        katana.weapon_type,
        WeaponType::TwoHandSword,
        "SubType names are case-insensitive"
    );
    assert_eq!(katana.range, 14, "Range capped at AREA_SIZE");
    assert_eq!(katana.weapon_level, 5, "WeaponLevel clamped to 5");
    assert_eq!(
        find(&out, 1599).weapon_level,
        1,
        "a weapon's level 0 becomes 1"
    );
    assert_eq!(find(&out, 1599).weapon_type, WeaponType::Fist);

    let guard = find(&out, 2101);
    assert_eq!(guard.defense, 20);
    assert_eq!(
        (guard.attack, guard.range, guard.weapon_level),
        (0, 0, 0),
        "weapon fields cleared on armor"
    );
    assert_eq!(guard.armor_level, 1, "an armor's level 0 becomes 1");
    assert_eq!((guard.equip_level_min, guard.equip_level_max), (10, 50));
    assert_eq!(find(&out, 2301).armor_level, 1);

    let demoted = find(&out, 1299);
    assert_eq!(demoted.item_type, ItemType::Etc, "no Locations");
    assert_eq!(
        (demoted.weapon_type, demoted.attack),
        (WeaponType::Fist, 0),
        "cleared after the demotion"
    );

    let arrow = find(&out, 1750);
    assert_eq!(arrow.ammo_type, Some(AmmoType::Arrow));
    assert_eq!(arrow.attack, 25, "ammo keeps Attack");
    assert_eq!((arrow.defense, arrow.range), (0, 0), "cleared on ammo");
    assert_eq!(find(&out, 13200).ammo_type, Some(AmmoType::Bullet));
    let card = find(&out, 4001);
    assert_eq!(
        (card.weapon_type, card.ammo_type),
        (WeaponType::Fist, None),
        "card subtypes dropped"
    );
    assert!(
        out.files.iter().flatten().all(|i| i.id.0 != 909),
        "SubType on an Etc item rejects the entry"
    );
    assert!(
        out.report.warnings.iter().any(|w| w.contains("909")),
        "{:?}",
        out.report.warnings
    );
}

fn jobs_of(list: &str) -> [u64; 3] {
    let equip = format!(
        "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n{list}    Locations:\n      Right_Hand: true\n"
    );
    find(&run("", &equip, ""), 1201).jobs
}

/// TOOL-ITEM-07: `Jobs` -> the three base-class bitsets (catalog mechanics/equipment.md §3a).
#[test]
fn tool_item_07_jobs() {
    assert_eq!(jobs_of(""), ALL_JOBS, "absent = every job");
    assert_eq!(jobs_of("    Jobs:\n      All: true\n"), ALL_JOBS);
    assert_eq!(
        jobs_of("    Jobs:\n      Novice: true\n      Knight: true\n      Crusader: true\n"),
        [1 << 0, 1 << 1, 1 << 1],
        "Novice (0,0), Knight (1,1), Crusader (2,1)"
    );
    assert_eq!(
        jobs_of("    Jobs:\n      Archer: true\n      BardDancer: true\n      Super_Novice: true\n      Spirit_Handler: true\n      Soul_Linker: true\n"),
        [1 << 3, (1 << 0) | (1 << 10), (1 << 3) | (1 << 7)],
        "underscored and joined names resolve alike"
    );
    assert_eq!(
        jobs_of("    Jobs:\n      Novice: false\n      All: true\n"),
        [!1, u64::MAX, u64::MAX],
        "All applies first, then the exclusion"
    );
    assert_eq!(
        jobs_of("    Jobs:\n      Novice: true\n      Nobody: true\n"),
        ALL_JOBS,
        "an unknown name makes the item usable by every job"
    );

    // An overriding entry replaces the list.
    let usable = "  - Id: 1201\n    Jobs:\n      Thief: true\n";
    let equip = "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n    Jobs:\n      Swordman: true\n    Locations:\n      Right_Hand: true\n";
    let out = run("", equip, usable);
    assert_eq!(
        find(&out, 1201).jobs,
        [1 << 6, 0, 0],
        "later Jobs replaces earlier"
    );
}

fn item_with(fields: &str) -> ItemData {
    let equip = format!(
        "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n{fields}    Locations:\n      Right_Hand: true\n"
    );
    find(&run("", &equip, ""), 1201).clone()
}

/// TOOL-ITEM-08: `Classes` groups and the forced `Gender` values.
#[test]
fn tool_item_08_classes_and_gender() {
    assert_eq!(item_with("").classes, ALL_CLASSES, "absent = every class");
    assert_eq!(
        item_with("    Classes:\n      All: true\n").classes,
        ALL_CLASSES
    );
    assert_eq!(
        item_with("    Classes:\n      Normal: true\n      Baby: true\n").classes,
        0x01 | 0x04
    );
    assert_eq!(
        item_with("    Classes:\n      All_Upper: true\n").classes,
        0x02 | 0x10 | 0x40,
        "upper, third upper, fourth"
    );
    assert_eq!(
        item_with("    Classes:\n      All_Third: true\n").classes,
        0x08 | 0x10 | 0x20,
        "All_Third excludes Fourth"
    );
    assert_eq!(
        item_with("    Classes:\n      All_Baby: true\n").classes,
        0x04 | 0x20
    );
    assert_eq!(
        item_with("    Classes:\n      All: true\n      Upper: false\n").classes,
        ALL_CLASSES & !0x02
    );
    assert_eq!(
        item_with("    Classes:\n      Normal: true\n      Bogus: true\n").classes,
        ALL_CLASSES,
        "an unknown name allows every class"
    );
    // Unlike Jobs, an overriding Classes edits the earlier value.
    let equip = "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n    Classes:\n      Normal: true\n    Locations:\n      Right_Hand: true\n";
    let etc = "  - Id: 1201\n    Classes:\n      Baby: true\n";
    assert_eq!(find(&run("", equip, etc), 1201).classes, 0x01 | 0x04);
    let etc_only = "  - Id: 1201\n    Classes:\n      Upper: false\n";
    let plain = "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n    Locations:\n      Right_Hand: true\n";
    assert_eq!(
        find(&run("", plain, etc_only), 1201).classes,
        ALL_CLASSES & !0x02,
        "an override edits the default ITEMJ_ALL"
    );

    assert_eq!(item_with("").gender, Gender::Both);
    assert_eq!(item_with("    Gender: female\n").gender, Gender::Female);
    let equip = "  - Id: 1901
    AegisName: Violin
    Name: Violin
    Type: Weapon
    SubType: Musical
    Gender: Female
    Locations:
      Right_Hand: true
  - Id: 1950
    AegisName: Rope
    Name: Rope
    Type: Weapon
    SubType: Whip
    Locations:
      Right_Hand: true
  - Id: 2634
    AegisName: Bridegroom_Ring
    Name: Wedding Ring
    Type: Armor
    Locations:
      Both_Accessory: true
  - Id: 2635
    AegisName: Bride_Ring
    Name: Wedding Ring
    Type: Armor
    Gender: Male
    Locations:
      Both_Accessory: true
";
    let out = run("", equip, "");
    assert_eq!(
        find(&out, 1901).gender,
        Gender::Male,
        "instruments are male-only"
    );
    assert_eq!(
        find(&out, 1950).gender,
        Gender::Female,
        "whips are female-only"
    );
    assert_eq!(find(&out, 2634).gender, Gender::Male);
    assert_eq!(find(&out, 2635).gender, Gender::Female);
}

fn rathena() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("RATHENA_PATH")?);
    path.join(SOURCES[0].0).is_file().then_some(path)
}

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
}
