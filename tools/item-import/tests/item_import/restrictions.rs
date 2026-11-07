//! Job, class and gender restrictions (TOOL-ITEM-07, 08).

use ro_db::{Gender, ALL_CLASSES, ALL_JOBS};

use crate::common::*;

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
