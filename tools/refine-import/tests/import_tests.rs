//! rAthena refine table conversion (spec §1.2).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use refine_import::{convert, render};
use ro_db::{RefineDb, RefineGroup};

const SOURCE: &str = "Header:
  Type: REFINE_DB
  Version: 2
Body:
  - Group: Armor
    Levels:
      - Level: 1
        RefineLevels:
          - Level: 1
            Bonus: 100
            Chances:
              - Type: Normal
                Rate: 10000
          - Level: 2
            Bonus: 200
  - Group: Weapon
    Levels:
      - Level: 1
        RefineLevels:
          - Level: 2
            Bonus: 400
            RandomBonus: 50
          - Level: 21
            Bonus: 9
  - Group: Shadow_Weapon
    Levels:
      - Level: 1
        RefineLevels:
          - Level: 1
            Chances:
              - Type: Normal
                Rate: 10000
";

#[test]
fn keeps_bonus_tables_and_drops_shadow_groups() {
    let out = convert(SOURCE).expect("converts");
    assert_eq!(
        out.rows.len(),
        2,
        "Armor 1 and Weapon 1; the shadow group has no bonus"
    );
    let armor = &out.rows[0];
    assert_eq!((armor.group, armor.level), (RefineGroup::Armor, 1));
    assert_eq!(
        armor.bonus[..3],
        [100, 200, 0],
        "missing refine levels are 0"
    );
    assert_eq!(armor.bonus.len(), 20);
    assert!(
        armor.random_bonus.is_empty(),
        "an all-zero RandomBonus is left out"
    );
    let weapon = &out.rows[1];
    assert_eq!(weapon.bonus[..2], [0, 400]);
    assert_eq!(weapon.random_bonus[..2], [0, 50]);
    assert!(
        out.warnings.iter().any(|w| w.contains("21")),
        "refine levels past 20 are reported: {:?}",
        out.warnings
    );
    assert!(out.warnings.iter().any(|w| w.contains("Shadow_Weapon")));
}

#[test]
fn rendered_file_has_the_header_and_loads() {
    let out = convert(SOURCE).expect("converts");
    let text = render(&out.rows, "abc123").expect("renders");
    assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
    assert!(text.contains("commit abc123") && text.contains("tools/refine-import"));
    assert!(text.contains("crates/ro-db/data/NOTICE.md"));
    let rows = RefineDb::parse("x", &text).expect("parses");
    let db = RefineDb::from_rows("x", rows).expect("loads");
    assert_eq!(db.bonus(RefineGroup::Weapon, 1, 2).random_bonus, 50);
}
