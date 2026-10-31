//! rAthena mob database conversion (spec TOOL-MOBDB-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use mob_import::{convert, render_file, Converted, SOURCE};
use ro_core::ItemId;
use ro_db::{
    Element, ItemData, ItemDb, ItemType, MobClass, MobData, MobDb, MobDrop, MobId, MobMode,
    MobModes, MobRace, MobSize,
};

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
    assert_eq!((osiris.class, osiris.is_mvp()), (MobClass::Boss, true));
    assert_eq!(osiris.mvp_drops.len(), 3);
    assert_eq!(osiris.boss_type(), 2);
    let jr = mob(&out, 1039);
    assert_eq!(
        (jr.class, jr.is_mvp(), jr.level),
        (MobClass::Boss, false, 50)
    );
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
    assert_eq!(db.iter().filter(|m| m.is_mvp()).count(), 190);
    assert_eq!(
        (
            poring.base_exp,
            poring.job_exp,
            poring.defense,
            poring.attack_delay
        ),
        (150, 40, 2, 1872)
    );
    assert_eq!(poring.modes.bits(), 0x83, "Ai 02");
    let plant = db.get(MobId(1080)).expect("Green Plant");
    assert!(plant.has_mode(MobMode::IgnoreMelee));
    let ignore_melee = db
        .iter()
        .filter(|m| m.has_mode(MobMode::IgnoreMelee))
        .count();
    assert_eq!(ignore_melee, 145, "catalog Modes: IgnoreMelee count");
}

fn modes(list: &[MobMode]) -> MobModes {
    list.iter().copied().collect()
}

/// TOOL-MOBDB-04: combat fields. `Ai` sets the mode bits, `Modes` toggles them, `Class` adds
/// its modes last; defaults, clamps and case-insensitive names follow catalog `db/mob_db.md`.
#[test]
fn tool_mobdb_04_combat_fields() {
    let out = run("  - Id: 1002
    AegisName: PORING
    Name: Poring
    Level: 1
    Hp: 55
    BaseExp: 150
    JobExp: 40
    Attack: 1
    Defense: 2
    Str: 6
    Int: 0
    Dex: 6
    Luk: 5
    AttackRange: 1
    Size: Medium
    Race: Plant
    Element: Water
    ElementLevel: 1
    WalkSpeed: 400
    AttackDelay: 1872
    AttackMotion: 672
    DamageMotion: 480
    Ai: 02
  - Id: 1080
    AegisName: GREEN_PLANT
    Name: Green Plant
    Agi: 0
    Vit: 0
    Dex: 0
    Luk: 0
    Defense: 100
    Ai: 06
    Modes:
      IgnoreMelee: true
  - Id: 1100
    AegisName: BOSSY
    Name: Bossy
    Class: Boss
    Ai: 04
    Size: large
    Race: Player_Human
    Element: GHOST
    ElementLevel: 7
    Defense: 40000
    Resistance: 50
    AttackDelay: 50
    AttackMotion: 300
    Modes:
      canmove: false
      KnockBackImmune: true
  - Id: 1101
    AegisName: ODD
    Name: Odd
    Ai: 14
    AttackMotion: 9000
    AttackDelay: 9000
  - Id: 1102
    AegisName: BARE
    Name: Bare
  - Id: 1102
    Modes:
      Looter: true
  - Id: 1103
    AegisName: RESET
    Name: Reset
    Ai: 01
    Modes:
      Aggressive: true
  - Id: 1103
    Ai: 02
");
    let poring = mob(&out, 1002);
    assert_eq!(
        (
            poring.base_exp,
            poring.job_exp,
            poring.attack,
            poring.defense
        ),
        (150, 40, 1, 2)
    );
    assert_eq!(
        (poring.str, poring.agi, poring.vit, poring.int, poring.dex, poring.luk),
        (6, 1, 1, 0, 6, 5),
        "unlisted stats default to 1; an explicit 0 stays"
    );
    assert_eq!(
        (
            poring.size,
            poring.race,
            poring.element,
            poring.element_level
        ),
        (MobSize::Medium, MobRace::Plant, Element::Water, 1)
    );
    assert_eq!(
        (
            poring.attack_delay,
            poring.attack_motion,
            poring.damage_motion
        ),
        (1872, 672, 480)
    );
    assert_eq!(poring.attack_range, 1);
    assert_eq!(
        poring.modes.bits(),
        0x83,
        "Ai 02 = CanMove|Looter|CanAttack"
    );

    let plant = mob(&out, 1080);
    assert_eq!(plant.modes, modes(&[MobMode::IgnoreMelee]), "Ai 06 = 0");
    assert_eq!((plant.agi, plant.vit, plant.dex, plant.luk), (0, 0, 0, 0));
    assert_eq!(plant.defense, 100);
    assert_eq!(
        (plant.attack_delay, plant.attack_motion),
        (100, 100),
        "MAX_ASPD_NOPC defaults"
    );

    let boss = mob(&out, 1100);
    // Ai 04 = 0x3885, CanMove cleared by Modes, then Boss adds Detector|StatusImmune|
    // KnockbackImmune.
    assert_eq!(
        boss.modes.bits(),
        0x3884 | 0x200_0000 | 0x400_0000 | 0x20_0000
    );
    assert_eq!(
        (boss.size, boss.race, boss.element),
        (MobSize::Large, MobRace::PlayerHuman, Element::Ghost),
        "names match case-insensitively"
    );
    assert_eq!(boss.element_level, 1, "an invalid level falls back to 1");
    assert_eq!(boss.defense, i16::MAX, "DEFTYPE_MAX");
    assert_eq!(boss.resistance, 50);
    assert_eq!(
        (boss.attack_delay, boss.attack_motion),
        (300, 300),
        "delay clamped to 100, then raised to the motion"
    );

    let odd = mob(&out, 1101);
    assert_eq!(odd.modes, MobModes::default(), "unknown Ai: 06");
    assert_eq!(
        (odd.attack_delay, odd.attack_motion),
        (8000, 8000),
        "MIN_ASPD cap"
    );

    assert_eq!(
        mob(&out, 1102).modes,
        modes(&[MobMode::Looter]),
        "no Ai: mask 0; a re-listed id's Modes still apply"
    );
    assert_eq!(
        mob(&out, 1103).modes.bits(),
        0x83,
        "a re-listed Ai replaces the mask and the earlier Modes"
    );
    for needle in ["1100: ElementLevel", "1101: unknown Ai"] {
        assert!(
            out.report.warnings.iter().any(|w| w.contains(needle)),
            "{needle}: {:?}",
            out.report.warnings
        );
    }
}
