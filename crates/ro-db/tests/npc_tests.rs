//! NPC spawn YAML (`NPC_SPAWN_DB`) loading (spec DB-NPC-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use ro_db::{DbLoadError, MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};

const VALID: &str = r#"
Header:
  Type: NPC_SPAWN_DB
  Version: 1
Body:
  - Name: "Kafra Employee#prt1"
    UniqueName: kafra_prt1
    X: 152
    Y: 326
    Dir: 4
    Sprite: 117
    Kind: Script
    Source: npc/re/merchants/kafra.txt:42
  - Name: prt001
    X: 156
    Y: 22
    Dir: 0
    Sprite: 45
    Trigger: { X: 1, Y: 2 }
    Kind: Warp
    Warp: { Map: prt_fild08, X: 170, Y: 375, TriggerWhileHidden: true }
    Source: npc/re/warps/cities/prontera.txt:12
  - Name: "Tool Dealer#dup"
    X: 10
    Y: 11
    Dir: 55
    Sprite: 1002
    SpriteIsMob: true
    State: Cloaked
    DuplicateOf: "Tool Dealer"
    Kind: Shop
    Source: npc/re/merchants/shops.txt:7
"#;

fn header(body: &str) -> String {
    format!("Header:\n  Type: NPC_SPAWN_DB\n  Version: 1\nBody:\n{body}")
}

#[test]
fn db_npc_01_loads_every_field() {
    let npcs = MapNpcs::from_yaml_str("prontera.yml", VALID).expect("valid file loads");
    assert_eq!(npcs.npcs.len(), 3);

    let kafra = &npcs.npcs[0];
    assert_eq!(kafra.name, "Kafra Employee#prt1");
    assert_eq!(kafra.unique_name, "kafra_prt1");
    assert_eq!(
        (kafra.x, kafra.y, kafra.dir, kafra.sprite),
        (152, 326, 4, 117)
    );
    assert!(!kafra.sprite_is_mob);
    assert_eq!(kafra.state, NpcState::Normal);
    assert_eq!(kafra.trigger, None);
    assert_eq!(kafra.duplicate_of, None);
    assert_eq!(kafra.kind, NpcKind::Script);
    assert_eq!(kafra.source, "npc/re/merchants/kafra.txt:42");

    let warp = &npcs.npcs[1];
    assert_eq!(
        warp.unique_name, "prt001",
        "unique name defaults to the name"
    );
    assert_eq!(warp.trigger, Some((1, 2)));
    assert_eq!(
        warp.kind,
        NpcKind::Warp(WarpDest {
            map: "prt_fild08".to_string(),
            x: 170,
            y: 375,
            trigger_while_hidden: true,
        })
    );

    let shop = &npcs.npcs[2];
    assert_eq!(shop.dir, 55, "dir is kept raw");
    assert!(shop.sprite_is_mob);
    assert_eq!(shop.state, NpcState::Cloaked);
    assert_eq!(shop.duplicate_of.as_deref(), Some("Tool Dealer"));
    assert_eq!(shop.kind, NpcKind::Shop);
}

#[test]
fn db_npc_01_round_trips() {
    let npcs = MapNpcs::from_yaml_str("prontera.yml", VALID).expect("valid file loads");
    let text = npcs.to_yaml_string().expect("serializes");
    let again = MapNpcs::from_yaml_str("again.yml", &text).expect("own output loads");
    assert_eq!(npcs, again);
    assert!(
        !text.contains("UniqueName: prt001"),
        "unique name equal to the name is omitted:\n{text}"
    );
    assert!(!text.contains("SpriteIsMob: false"), "{text}");
    assert!(!text.contains("State: Normal"), "{text}");
}

#[test]
fn db_npc_01_empty_body_is_no_npcs() {
    let npcs = MapNpcs::from_yaml_str("e.yml", &header("  []\n")).expect("empty body loads");
    assert!(npcs.npcs.is_empty());
}

fn expect_err(text: &str) -> DbLoadError {
    MapNpcs::from_yaml_str("bad.yml", text).expect_err("must be rejected")
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
fn db_npc_02_rejects_wrong_header() {
    let wrong_type = VALID.replace("Type: NPC_SPAWN_DB", "Type: MOB_DB");
    let wrong_version = VALID.replace("Version: 1", "Version: 2");
    for text in [wrong_type, wrong_version] {
        let err = expect_err(&text);
        assert!(matches!(err, DbLoadError::Schema { .. }), "{err:?}");
        assert!(names_path(&err), "{err:?}");
    }
}

#[test]
fn db_npc_02_rejects_bad_entries() {
    let entry = |extra: &str| {
        header(&format!(
            "  - Name: a\n    X: 1\n    Y: 2\n    Dir: 0\n    Sprite: 45\n    Source: s:1\n{extra}"
        ))
    };
    let cases = [
        ("missing kind", entry("")),
        ("warp without destination", entry("    Kind: Warp\n")),
        (
            "destination on a script",
            entry(
                "    Kind: Script\n    Warp: { Map: m, X: 1, Y: 1, TriggerWhileHidden: false }\n",
            ),
        ),
        ("unknown field", entry("    Kind: Script\n    Speed: 3\n")),
        ("unknown kind", entry("    Kind: Monster\n")),
        ("syntax error", header("  - Name: [unclosed\n")),
    ];
    for (what, text) in cases {
        let err = expect_err(&text);
        assert!(matches!(err, DbLoadError::Parse { .. }), "{what}: {err:?}");
        assert!(names_path(&err), "{what}: {err:?}");
    }
}

#[test]
fn db_npc_02_syntax_error_reports_line() {
    let err = expect_err(&header("  - Name: a\n    X: [1\n"));
    let DbLoadError::Parse { line, .. } = err else {
        panic!("expected a parse error, got {err:?}");
    };
    assert!(line.is_some(), "line is reported");
}

#[test]
fn db_npc_02_missing_file_is_none() {
    let dir = std::env::temp_dir().join(format!("ro-db-npc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    assert_eq!(
        MapNpcs::load_yaml(&dir.join("nowhere.yml")).expect("missing is not an error"),
        None
    );
    let file = dir.join("prontera.yml");
    std::fs::write(&file, VALID).expect("write fixture");
    let loaded = MapNpcs::load_yaml(&file).expect("loads").expect("present");
    assert_eq!(loaded.npcs.len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn db_npc_01_spawn_fields_are_plain_data() {
    // Builders for other crates' tests construct NpcSpawn directly.
    let npc = NpcSpawn {
        name: "n".into(),
        unique_name: "n".into(),
        x: 1,
        y: 1,
        dir: 0,
        sprite: 111,
        sprite_is_mob: false,
        state: NpcState::Disabled,
        trigger: None,
        duplicate_of: None,
        kind: NpcKind::Script,
        source: "s:1".into(),
    };
    let text = MapNpcs { npcs: vec![npc] }
        .to_yaml_string()
        .expect("serializes");
    assert!(text.contains("State: Disabled"), "{text}");
}

fn shipped_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/npc")
}

/// DB-NPC-03: every committed file loads and carries the license header.
#[test]
fn db_npc_03_shipped_data_loads() {
    let dir = shipped_dir();
    let entries = std::fs::read_dir(&dir).expect("crates/ro-db/data/npc is committed");
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
            text.contains("rAthena"),
            "{} lacks attribution",
            path.display()
        );
        MapNpcs::from_yaml_str(&path.display().to_string(), &text)
            .unwrap_or_else(|e| panic!("{e}"));
    }
    assert!(
        files > 800,
        "expected the full rAthena import, found {files} files"
    );
    assert!(dir.join("LICENSE").is_file(), "data/npc/LICENSE missing");
    assert!(
        dir.join("NOTICE.md").is_file(),
        "data/npc/NOTICE.md missing"
    );
}
