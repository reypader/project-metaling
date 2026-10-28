//! Refine bonus table (`REFINE_DATA`) loading (spec DB-REF-01).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use ro_db::{DbLoadError, RefineBonus, RefineDb, RefineGroup, REFINE_DATA_FILE};

const TABLE: &str = r"
Header:
  Type: REFINE_DATA
  Version: 1
Body:
  - Group: Weapon
    Level: 1
    Bonus: [200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600, 2800, 3000, 4800, 5100, 5400, 5700, 6000]
    RandomBonus: [0, 0, 0, 0, 0, 0, 0, 300, 600, 900, 1200, 1500, 1800, 2100, 2400, 2700, 3000, 3300, 3600, 3900]
  - Group: Armor
    Level: 1
    Bonus: [100, 200, 300, 400, 600, 800, 1000, 1200, 1500, 1800, 2100, 2400, 2800, 3200, 3600, 4000, 4500, 5000, 5500, 6000]
";

fn db() -> RefineDb {
    let rows = RefineDb::parse("table", TABLE).expect("fixture parses");
    RefineDb::from_rows("fixture", rows).expect("fixture builds")
}

#[test]
fn bonus_is_the_entry_for_the_current_refine() {
    let db = db();
    assert_eq!(
        db.bonus(RefineGroup::Weapon, 1, 10),
        RefineBonus {
            bonus: 2000,
            random_bonus: 900
        }
    );
    assert_eq!(db.bonus(RefineGroup::Armor, 1, 10).bonus, 1800);
    assert_eq!(
        db.bonus(RefineGroup::Armor, 1, 10).random_bonus,
        0,
        "a missing RandomBonus list is all zero"
    );
}

#[test]
fn no_bonus_at_refine_zero_or_for_a_missing_level() {
    let db = db();
    assert_eq!(db.bonus(RefineGroup::Weapon, 1, 0), RefineBonus::default());
    assert_eq!(db.bonus(RefineGroup::Weapon, 2, 5), RefineBonus::default());
    assert_eq!(db.bonus(RefineGroup::Armor, 0, 5), RefineBonus::default());
}

#[test]
fn refine_above_the_max_is_clamped_to_20() {
    let db = db();
    assert_eq!(
        db.bonus(RefineGroup::Weapon, 1, 25),
        db.bonus(RefineGroup::Weapon, 1, 20)
    );
    assert_eq!(db.bonus(RefineGroup::Weapon, 1, 20).bonus, 6000);
}

#[test]
fn rejects_short_lists_and_duplicate_rows() {
    let short = TABLE.replace("[200, 400, 600, ", "[400, 600, ");
    let rows = RefineDb::parse("table", &short).expect("parse");
    assert!(matches!(
        RefineDb::from_rows("x", rows),
        Err(DbLoadError::Schema { .. })
    ));
    let dup = TABLE.replace("Group: Armor", "Group: Weapon");
    let rows = RefineDb::parse("table", &dup).expect("parse");
    assert!(matches!(
        RefineDb::from_rows("x", rows),
        Err(DbLoadError::Schema { .. })
    ));
}

#[test]
fn round_trips_through_yaml() {
    let rows = RefineDb::parse("table", TABLE).expect("parse");
    let text = RefineDb::to_yaml(&rows).expect("serialize");
    assert_eq!(RefineDb::parse("again", &text).expect("reparse"), rows);
}

/// DB-REF-01: the shipped table loads with its header; spot values from catalog db/refine.md.
#[test]
fn db_ref_01_shipped_data_loads() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/refine"));
    let path = dir.join(REFINE_DATA_FILE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
    assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
    assert!(text.contains("rAthena") && text.contains("crates/ro-db/data/NOTICE.md"));
    let db = RefineDb::load_dir(dir).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(db.bonus(RefineGroup::Weapon, 1, 10).bonus, 2000);
    assert_eq!(db.bonus(RefineGroup::Armor, 1, 10).bonus, 1800);
    assert_eq!(db.bonus(RefineGroup::Weapon, 1, 8).random_bonus, 300);
    assert_eq!(
        db.bonus(RefineGroup::Weapon, 1, 6).bonus,
        1200,
        "Blade +6 adds 12"
    );
    assert_eq!(db.bonus(RefineGroup::Armor, 2, 1).bonus, 120);
    assert_eq!(db.bonus(RefineGroup::Weapon, 5, 20).bonus, 16000);
    assert_eq!(db.bonus(RefineGroup::Weapon, 6, 1), RefineBonus::default());
}
