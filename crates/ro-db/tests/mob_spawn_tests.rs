//! Mob spawn YAML (`MOB_SPAWN_DB`) loading (spec DB-MOB-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ro_db::{DbLoadError, MapMobSpawns, MobId, MobSpawn};

const VALID: &str = r#"
Header:
  Type: MOB_SPAWN_DB
  Version: 1
Body:
  - Name: Poring
    MobId: 1002
    X: 0
    Y: 0
    Amount: 20
    Delay1: 5000
    Source: npc/re/mobs/fields/prontera.txt:12
  - Name: Green Plant
    MobId: 1080
    X: 285
    Y: 138
    Span: { X: 10, Y: 10 }
    Amount: 5
    Delay1: 360000
    Delay2: 180000
    Source: npc/re/mobs/fields/prontera.txt:30
  - Name: Pharaoh
    MobId: 1157
    X: 0
    Y: 0
    Amount: 1
    Delay1: 3600000
    Delay2: 600000
    Level: 99
    Boss: true
    Event: "ant_d02_i_boss::OnMobDead"
    Source: npc/re/mobs/dungeons/sphinx.txt:7
  - Name: Elusive Wraith
    MobId: 20255
    X: 30
    Y: 40
    Amount: 1000
    Delay1: 5000
    Source: npc/re/mobs/dungeons/ein_dun.txt:57
"#;

fn header(body: &str) -> String {
    format!("Header:\n  Type: MOB_SPAWN_DB\n  Version: 1\nBody:\n{body}")
}

#[test]
fn db_mob_01_loads_every_field() {
    let mobs = MapMobSpawns::from_yaml_str("prt_fild08.yml", VALID).expect("valid file loads");
    assert_eq!(mobs.spawns.len(), 4);

    let poring = &mobs.spawns[0];
    assert_eq!(poring.name, "Poring");
    assert_eq!(poring.mob_id, MobId(1002));
    assert_eq!((poring.x, poring.y), (0, 0));
    assert_eq!(poring.span, (0, 0), "absent span is 0x0");
    assert_eq!(poring.amount, 20);
    assert_eq!(
        (poring.delay1, poring.delay2),
        (5000, 0),
        "absent Delay2 is 0"
    );
    assert_eq!(poring.level, None);
    assert!(!poring.boss);
    assert_eq!(poring.event, None);
    assert_eq!(poring.source, "npc/re/mobs/fields/prontera.txt:12");

    let plant = &mobs.spawns[1];
    assert_eq!((plant.x, plant.y), (285, 138));
    assert_eq!(plant.span, (10, 10));
    assert_eq!((plant.delay1, plant.delay2), (360_000, 180_000));

    let pharaoh = &mobs.spawns[2];
    assert_eq!(pharaoh.level, Some(99));
    assert!(pharaoh.boss);
    assert_eq!(pharaoh.event.as_deref(), Some("ant_d02_i_boss::OnMobDead"));

    let wraith = &mobs.spawns[3];
    assert_eq!(wraith.mob_id, MobId(20255), "upper id range is valid");
    assert_eq!(wraith.amount, 1000);
}

#[test]
fn db_mob_01_round_trips() {
    let mobs = MapMobSpawns::from_yaml_str("prt_fild08.yml", VALID).expect("valid file loads");
    let text = mobs.to_yaml_string().expect("serializes");
    let again = MapMobSpawns::from_yaml_str("again.yml", &text).expect("own output loads");
    assert_eq!(mobs, again);
    assert!(text.contains("Type: MOB_SPAWN_DB"), "{text}");
    assert!(
        text.contains("Delay1: 5000"),
        "Delay1 is always written:\n{text}"
    );
    for omitted in [
        "Delay2: 0",
        "Boss: false",
        "Level: null",
        "Event: null",
        "Span: null",
    ] {
        assert!(!text.contains(omitted), "{omitted} is omitted:\n{text}");
    }
    assert_eq!(
        text.matches("Span:").count(),
        1,
        "a 0x0 span is omitted:\n{text}"
    );
}

#[test]
fn db_mob_01_spawn_fields_are_plain_data() {
    // Builders for other crates' tests construct MobSpawn directly.
    let spawn = MobSpawn {
        name: "Lunatic".into(),
        mob_id: MobId(1063),
        x: 5,
        y: 6,
        span: (0, 3),
        amount: 2,
        delay1: 5000,
        delay2: 0,
        level: Some(3),
        boss: false,
        event: None,
        source: "s:1".into(),
    };
    let mobs = MapMobSpawns {
        spawns: vec![spawn],
    };
    let text = mobs.to_yaml_string().expect("serializes");
    assert!(
        text.contains("Span:"),
        "a span with one non-zero side is kept:\n{text}"
    );
    let again = MapMobSpawns::from_yaml_str("again.yml", &text).expect("own output loads");
    assert_eq!(mobs, again);
}

#[test]
fn db_mob_01_empty_body_is_no_mobs() {
    let mobs = MapMobSpawns::from_yaml_str("e.yml", &header("  []\n")).expect("empty body loads");
    assert!(mobs.spawns.is_empty());
}

fn expect_err(text: &str) -> DbLoadError {
    MapMobSpawns::from_yaml_str("bad.yml", text).expect_err("must be rejected")
}

fn names_path(err: &DbLoadError) -> bool {
    match err {
        DbLoadError::Io { path, .. }
        | DbLoadError::Parse { path, .. }
        | DbLoadError::Schema { path, .. }
        | DbLoadError::DuplicateId { path, .. } => path == "bad.yml",
    }
}

#[test]
fn db_mob_02_rejects_wrong_header() {
    let wrong_type = VALID.replace("Type: MOB_SPAWN_DB", "Type: NPC_SPAWN_DB");
    let wrong_version = VALID.replace("Version: 1", "Version: 2");
    for text in [wrong_type, wrong_version] {
        let err = expect_err(&text);
        assert!(matches!(err, DbLoadError::Schema { .. }), "{err:?}");
        assert!(names_path(&err), "{err:?}");
    }
}

#[test]
fn db_mob_02_rejects_bad_entries() {
    let entry = |mob_id: &str, amount: &str, extra: &str| {
        header(&format!(
            "  - Name: a\n    MobId: {mob_id}\n    X: 1\n    Y: 2\n    Amount: {amount}\n    Delay1: 5000\n    Source: s:1\n{extra}"
        ))
    };
    let cases = [
        (
            "missing MobId",
            header("  - Name: a\n    X: 1\n    Y: 2\n    Amount: 1\n    Delay1: 5000\n    Source: s:1\n"),
        ),
        (
            "missing Source",
            header("  - Name: a\n    MobId: 1002\n    X: 1\n    Y: 2\n    Amount: 1\n    Delay1: 5000\n"),
        ),
        ("unknown field", entry("1002", "1", "    Speed: 150\n")),
        ("unknown span field", entry("1002", "1", "    Span: { X: 1, Y: 1, Z: 1 }\n")),
        ("amount 0", entry("1002", "0", "")),
        ("amount 1001", entry("1002", "1001", "")),
        ("mob id 1000", entry("1000", "1", "")),
        ("mob id 3999", entry("3999", "1", "")),
        ("mob id 20020", entry("20020", "1", "")),
        ("mob id 31999", entry("31999", "1", "")),
        ("negative x", entry("1002", "1", "").replace("X: 1", "X: -1")),
        ("level 0", entry("1002", "1", "    Level: 0\n")),
        ("level 276", entry("1002", "1", "    Level: 276\n")),
        ("span above i16", entry("1002", "1", "    Span: { X: 32768, Y: 1 }\n")),
        ("syntax error", header("  - Name: [unclosed\n")),
    ];
    for (what, text) in cases {
        let err = expect_err(&text);
        assert!(matches!(err, DbLoadError::Parse { .. }), "{what}: {err:?}");
        assert!(names_path(&err), "{what}: {err:?}");
    }
}

#[test]
fn db_mob_02_accepts_range_edges() {
    for (mob_id, amount) in [
        ("1001", "1"),
        ("3998", "1000"),
        ("20021", "1"),
        ("31998", "1"),
    ] {
        let text = header(&format!(
            "  - Name: a\n    MobId: {mob_id}\n    X: 1\n    Y: 2\n    Amount: {amount}\n    Delay1: 5000\n    Source: s:1\n"
        ));
        MapMobSpawns::from_yaml_str("edge.yml", &text)
            .unwrap_or_else(|e| panic!("MobId {mob_id} Amount {amount} is valid: {e}"));
    }
}

#[test]
fn db_mob_02_accepts_level_and_span_edges() {
    for extra in [
        "    Level: 1\n",
        "    Level: 275\n",
        "    Span: { X: 32767, Y: 32767 }\n",
    ] {
        let text = header(&format!(
            "  - Name: a\n    MobId: 1002\n    X: 1\n    Y: 2\n    Amount: 1\n    Delay1: 5000\n    Source: s:1\n{extra}"
        ));
        MapMobSpawns::from_yaml_str("edge.yml", &text)
            .unwrap_or_else(|e| panic!("{extra:?} is valid: {e}"));
    }
}

#[test]
fn db_mob_02_syntax_error_reports_line() {
    let err = expect_err(&header("  - Name: a\n    X: [1\n"));
    let DbLoadError::Parse { line, .. } = err else {
        panic!("expected a parse error, got {err:?}");
    };
    assert!(line.is_some(), "line is reported");
}

#[test]
fn db_mob_02_missing_file_is_none() {
    let dir = std::env::temp_dir().join(format!("ro-db-mob-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    assert_eq!(
        MapMobSpawns::load_yaml(&dir.join("nowhere.yml")).expect("missing is not an error"),
        None
    );
    let file = dir.join("prt_fild08.yml");
    std::fs::write(&file, VALID).expect("write fixture");
    let loaded = MapMobSpawns::load_yaml(&file)
        .expect("loads")
        .expect("present");
    assert_eq!(loaded.spawns.len(), 4);

    let bad = dir.join("bad.yml");
    std::fs::write(&bad, VALID.replace("Amount: 20", "Amount: 0")).expect("write fixture");
    let err = MapMobSpawns::load_yaml(&bad).expect_err("bad file is an error");
    assert!(
        err.to_string().contains(&bad.display().to_string()),
        "error names the path: {err}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// DB-MOB-03: every committed file loads and carries the license header.
#[test]
fn db_mob_03_shipped_data_loads() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/mob_spawn");
    let entries = std::fs::read_dir(&dir).expect("crates/ro-db/data/mob_spawn is committed");
    let mut files = 0;
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("yml") {
            continue;
        }
        files += 1;
        let text = std::fs::read_to_string(&path).expect("readable");
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
        MapMobSpawns::from_yaml_str(&path.display().to_string(), &text)
            .unwrap_or_else(|e| panic!("{e}"));
    }
    assert!(
        files > 400,
        "expected the full rAthena import, found {files} files"
    );
    let data = dir.parent().expect("data dir");
    let notice = std::fs::read_to_string(data.join("NOTICE.md")).expect("data/NOTICE.md");
    assert!(
        notice.contains("## Mob spawn lists"),
        "NOTICE.md lacks the mob spawn dataset section"
    );
}
