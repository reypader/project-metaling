//! Mob registry YAML (`MOB_DATA`) loading (spec DB-MOBDB-01..03).

use std::path::Path;

use ro_core::ItemId;
use ro_db::{
    DbLoadError, Element, MobClass, MobData, MobDb, MobDrop, MobId, MobMode, MobModes, MobRace,
    MobSize, MOB_DATA_FILE,
};

const VALID: &str = r#"
Header:
  Type: MOB_DATA
  Version: 2
Body:
  - Id: 1002
    AegisName: PORING
    Name: Poring
    Level: 1
    Hp: 55
    BaseExp: 150
    JobExp: 40
    Attack: 1
    Defense: 2
    Resistance: 0
    Str: 6
    Agi: 1
    Vit: 1
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
    Class: Normal
    Modes: [CanMove, Looter, CanAttack]
    Drops:
      - { Item: 909, Rate: 7000 }
      - { Item: 1202, Rate: 100 }
    MvpDrops: []
  - Id: 1038
    AegisName: OSIRIS
    Name: Osiris
    Level: 78
    Hp: 1175840
    WalkSpeed: 100
    Class: Boss
    Modes: [Mvp, Detector]
    Drops:
      - { Item: 501, Rate: 10000 }
    MvpDrops:
      - { Item: 909, Rate: 5000 }
  - Id: 1039
    AegisName: BAPHOMET_
    Name: Baphomet Jr.
    Level: 50
    Hp: 8578
    WalkSpeed: 100
    Class: Boss
    Drops: []
    MvpDrops: []
"#;

fn header(body: &str) -> String {
    format!("Header:\n  Type: MOB_DATA\n  Version: 2\nBody:\n{body}")
}

fn load(text: &str) -> Result<MobDb, DbLoadError> {
    let mobs = MobDb::parse_file("mobs.yml", text)?;
    MobDb::from_mobs("mobs.yml", mobs)
}

/// DB-MOBDB-01: every field loads, lookups by id work, drops keep db order.
#[test]
fn db_mobdb_01_loads_every_field() {
    let db = load(VALID).expect("valid file loads");
    assert_eq!(db.len(), 3);
    let poring = db.get(MobId(1002)).expect("Poring");
    assert_eq!(
        poring,
        &MobData {
            id: MobId(1002),
            aegis_name: "PORING".into(),
            name: "Poring".into(),
            level: 1,
            hp: 55,
            base_exp: 150,
            job_exp: 40,
            attack: 1,
            defense: 2,
            resistance: 0,
            str: 6,
            agi: 1,
            vit: 1,
            int: 0,
            dex: 6,
            luk: 5,
            attack_range: 1,
            size: MobSize::Medium,
            race: MobRace::Plant,
            element: Element::Water,
            element_level: 1,
            walk_speed: 400,
            attack_delay: 1872,
            attack_motion: 672,
            damage_motion: 480,
            class: MobClass::Normal,
            modes: [MobMode::CanMove, MobMode::Looter, MobMode::CanAttack]
                .into_iter()
                .collect(),
            drops: vec![
                MobDrop {
                    item: ItemId(909),
                    rate: 7000
                },
                MobDrop {
                    item: ItemId(1202),
                    rate: 100
                },
            ],
            mvp_drops: vec![],
        }
    );
    assert_eq!(db.get(MobId(1038)).expect("Osiris").mvp_drops.len(), 1);
    assert_eq!(db.get(MobId(1001)), None);
}

/// DB-MOBDB-01: `boss_type()` is 2 for MVPs, 1 for `Class: Boss`, else 0.
#[test]
fn db_mobdb_01_boss_type() {
    let db = load(VALID).expect("valid file loads");
    assert_eq!(db.get(MobId(1002)).expect("Poring").boss_type(), 0);
    assert_eq!(db.get(MobId(1038)).expect("Osiris").boss_type(), 2);
    assert_eq!(db.get(MobId(1039)).expect("Baph Jr").boss_type(), 1);
}

/// DB-MOBDB-01: omitted combat fields take rAthena's struct defaults; modes are a bit-set.
#[test]
fn db_mobdb_01_defaults_and_modes() {
    let db = load(VALID).expect("valid file loads");
    let jr = db.get(MobId(1039)).expect("Baph Jr");
    assert_eq!(
        (jr.base_exp, jr.job_exp, jr.attack, jr.defense),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (jr.str, jr.agi, jr.vit, jr.int, jr.dex, jr.luk),
        (1, 1, 1, 1, 1, 1)
    );
    assert_eq!(
        (jr.size, jr.race, jr.element, jr.element_level),
        (MobSize::Small, MobRace::Formless, Element::Neutral, 1)
    );
    assert_eq!(
        (jr.attack_delay, jr.attack_motion, jr.damage_motion),
        (100, 100, 0)
    );
    assert_eq!(jr.modes, MobModes::default());
    let osiris = db.get(MobId(1038)).expect("Osiris");
    assert!(osiris.has_mode(MobMode::Mvp) && osiris.has_mode(MobMode::Detector));
    assert!(!osiris.has_mode(MobMode::IgnoreMelee));
    assert_eq!(MobMode::IgnoreMelee.bit(), 0x10000);
    assert_eq!(osiris.modes.bits(), 0x0208_0000);
}

/// DB-MOBDB-02: an attack delay below the attack motion, a zero motion, or an element level
/// outside 1..=4 is rejected.
#[test]
fn db_mobdb_02_rejects_bad_combat_values() {
    for extra in [
        "    AttackDelay: 100\n    AttackMotion: 200\n",
        "    AttackMotion: 0\n",
        "    ElementLevel: 0\n",
        "    ElementLevel: 5\n",
    ] {
        assert!(
            matches!(
                expect_err(&header(&(mob(1002, "A", "[]") + extra))),
                DbLoadError::Schema { .. }
            ),
            "{extra}"
        );
    }
    assert!(matches!(
        expect_err(&header(&(mob(1002, "A", "[]") + "    Modes: [Flying]\n"))),
        DbLoadError::Parse { .. }
    ));
}

/// DB-MOBDB-01: write then read gives the same mobs.
#[test]
fn db_mobdb_01_round_trips() {
    let mobs = MobDb::parse_file("mobs.yml", VALID).expect("parses");
    let text = MobDb::file_to_yaml(&mobs).expect("serializes");
    assert_eq!(
        MobDb::parse_file("again.yml", &text).expect("reparses"),
        mobs
    );
}

fn expect_err(text: &str) -> DbLoadError {
    load(text).expect_err("must be rejected")
}

fn mob(id: u32, aegis: &str, drops: &str) -> String {
    format!(
        "  - Id: {id}\n    AegisName: {aegis}\n    Name: x\n    Level: 1\n    Hp: 1\n    WalkSpeed: 150\n    Class: Normal\n    Drops: {drops}\n    MvpDrops: []\n"
    )
}

/// DB-MOBDB-02: bad headers, duplicates, unknown keys and out-of-range values are rejected.
#[test]
fn db_mobdb_02_rejects_bad_entries() {
    assert!(matches!(
        expect_err(&VALID.replace("Type: MOB_DATA", "Type: MOB_DB")),
        DbLoadError::Schema { .. }
    ));
    assert!(matches!(
        expect_err(&header(&(mob(1002, "A", "[]") + &mob(1002, "B", "[]")))),
        DbLoadError::DuplicateId { id: 1002, .. }
    ));
    assert!(matches!(
        expect_err(&header(
            &(mob(1002, "Poring", "[]") + &mob(1003, "PORING", "[]"))
        )),
        DbLoadError::Schema { .. }
    ));
    assert!(matches!(
        expect_err(&header(&(mob(1002, "A", "[]") + "    Ai: '02'\n"))),
        DbLoadError::Parse { .. }
    ));
    assert!(matches!(
        expect_err(&header(&mob(999, "A", "[]"))),
        DbLoadError::Schema { .. }
    ));
    let eleven = format!("[{}]", ["{ Item: 909, Rate: 1 }"; 11].join(", "));
    assert!(matches!(
        expect_err(&header(&mob(1002, "A", &eleven))),
        DbLoadError::Schema { .. }
    ));
    for rate in [0, 10001] {
        let drops = format!("[{{ Item: 909, Rate: {rate} }}]");
        assert!(matches!(
            expect_err(&header(&mob(1002, "A", &drops))),
            DbLoadError::Schema { .. }
        ));
    }
    let four_mvp = header(&mob(1002, "A", "[]").replace(
        "MvpDrops: []",
        "MvpDrops: [{ Item: 1, Rate: 1 }, { Item: 1, Rate: 1 }, { Item: 1, Rate: 1 }, { Item: 1, Rate: 1 }]",
    ));
    assert!(matches!(expect_err(&four_mvp), DbLoadError::Schema { .. }));
}

/// DB-MOBDB-02: a directory load needs the file.
#[test]
fn db_mobdb_02_load_dir() {
    let dir = std::env::temp_dir().join(format!("ro-db-mob-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    assert!(matches!(MobDb::load_dir(&dir), Err(DbLoadError::Io { .. })));
    std::fs::write(dir.join(MOB_DATA_FILE), VALID).expect("write");
    assert_eq!(MobDb::load_dir(&dir).expect("loads").len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

/// DB-MOBDB-03, DB-CMB-01 (mob part): the shipped data loads and carries the license header.
#[test]
fn db_mobdb_03_shipped_data_loads() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/mob");
    let path = dir.join(MOB_DATA_FILE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
    assert!(
        text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"),
        "lacks the SPDX header"
    );
    assert!(
        text.contains("rAthena") && text.contains("crates/ro-db/data/NOTICE.md"),
        "lacks attribution"
    );
    let db = MobDb::load_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        db.len() > 2_500,
        "expected the full import, found {}",
        db.len()
    );
    let poring = db.get(MobId(1002)).expect("Poring");
    assert_eq!(
        poring.drops.first().map(|d| (d.item, d.rate)),
        Some((ItemId(909), 7000))
    );
}

/// DB-CMB-02: the combat fields of the shipped Poring and Green Plant.
#[test]
fn db_cmb_02_shipped_combat_fields() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/mob");
    let db = MobDb::load_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
    let poring = db.get(MobId(1002)).expect("Poring");
    assert_eq!(
        (poring.hp, poring.base_exp, poring.job_exp, poring.defense),
        (55, 150, 40, 2)
    );
    assert_eq!(
        (
            poring.attack_motion,
            poring.damage_motion,
            poring.attack_delay
        ),
        (672, 480, 1872)
    );
    assert_eq!(
        (poring.size, poring.element),
        (MobSize::Medium, Element::Water)
    );
    assert!(!poring.has_mode(MobMode::IgnoreMelee));
    let plant = db.get(MobId(1080)).expect("Green Plant");
    assert!(plant.has_mode(MobMode::IgnoreMelee));
    assert_eq!((plant.agi, plant.vit, plant.dex, plant.luk), (0, 0, 0, 0));
    let osiris = db.get(MobId(1038)).expect("Osiris");
    assert_eq!(osiris.boss_type(), 2);
    assert!(
        osiris.has_mode(MobMode::Detector),
        "Class: Boss adds Detector"
    );
}
