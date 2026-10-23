//! rAthena NPC file conversion (spec TOOL-NPC-01..07) and mob spawn lines (TOOL-MOB-01..05).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use npc_import::{
    records, render_map_file, resolve_manifest, Imported, Importer, LexError, SpriteTable,
};
use ro_db::{MapMobSpawns, MapNpcs, MobId, MobSpawn, NpcKind, NpcSpawn, NpcState, WarpDest};

const NPC_HPP: &str = "enum e_job_types\n{\n\tNPC_RANGE1_START = 44,\n\tJT_WARPNPC,\n\tJT_1_ETC_01,\n\tJT_4_F_KAFRA1 = 117,\n\tJT_HIDDEN_WARP_NPC = 139,\n\tJT_INVISIBLE = 32767,\n\tJT_FAKENPC = -1\n};\n";
const MOB_DB: &str = concat!(
    "Header:\n  Type: MOB_DB\n  Version: 5\nBody:\n",
    "  - Id: 1002\n    AegisName: PORING\n    Name: Poring\n",
    "    RaceGroups:\n      A: true\n      A: true\n",
    "  - Id: 1063\n    AegisName: LUNATIC\n    Name: Lunatic\n    JapaneseName: Lunatic JP\n",
    "  - Id: 20255\n    AegisName: SPIRIT_TEST\n    Name: L Blue Earth Spirit\n",
    "  - Id: 5000\n    AegisName: OUT_OF_RANGE\n    Name: Custom\n",
);

fn importer() -> Importer {
    Importer::new(SpriteTable::from_sources(NPC_HPP, &[MOB_DB]).expect("tables"))
}

/// Converts one file and returns everything it produced.
fn import_one(text: &[u8]) -> Imported {
    let mut imp = importer();
    imp.add_file("npc/test.txt", text);
    imp.finish()
}

/// Converts one file and returns the NPC maps plus the report.
fn convert(text: &[u8]) -> (BTreeMap<String, MapNpcs>, npc_import::Report) {
    let out = import_one(text);
    (out.npcs, out.report)
}

fn only(maps: &BTreeMap<String, MapNpcs>, map: &str) -> Vec<NpcSpawn> {
    maps.get(map).map(|m| m.npcs.clone()).unwrap_or_default()
}

// ---------- TOOL-NPC-01: manifest ----------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("npc-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(path, text).expect("write");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn tool_npc_01_manifest_directives() {
    let t = TempDir::new("manifest");
    for f in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        t.write(&format!("npc/{f}"), "");
    }
    t.write(
        "npc/re/scripts_main.conf",
        "// header comment\n\
         npc: npc/a.txt\n\
         NPC: npc/b.txt // trailing comment\n\
         npc: npc/a.txt\n\
         import: npc/sub.conf\n\
         delnpc: npc/b.txt\n\
         npc: npc/b.txt\n\
         npc: npc/missing.txt\n\
         \x20npc: npc/d.txt\n\
         unknown: npc/d.txt\n",
    );
    t.write("npc/sub.conf", "npc: npc/c.txt\t\n");
    let m = resolve_manifest(&t.0, "npc/re/scripts_main.conf");
    assert_eq!(m.files, ["npc/a.txt", "npc/c.txt", "npc/b.txt"]);
    assert_eq!(m.warnings.len(), 1, "{:?}", m.warnings);
    assert!(m.warnings[0].contains("npc/missing.txt"));
}

#[test]
fn tool_npc_01_clear_and_delete_all() {
    let t = TempDir::new("clear");
    for f in ["a.txt", "b.txt", "c.txt"] {
        t.write(&format!("npc/{f}"), "");
    }
    t.write(
        "main.conf",
        "npc: npc/a.txt\nnpc: clear\nnpc: npc/b.txt\ndelnpc: all\nnpc: npc/c.txt\n",
    );
    assert_eq!(resolve_manifest(&t.0, "main.conf").files, ["npc/c.txt"]);
}

// ---------- TOOL-NPC-02: lexing ----------

#[test]
fn tool_npc_02_skips_comments_and_bodies() {
    let src = b"//===== header\n\
/* block\n comment with\tprontera,1,1,1\tscript\tFake\t1,{ */\n\
prontera,10,20,4\tscript\tA\t117,{\n\
\tmes \"} not the end { \\\" still string\";\n\
\t// } comment brace\n\
\t/* } */ if (1) { end; }\n\
}\n\
\t  prontera,11,21,4\tscript\tB\t117,{ end; } prontera,12,22,0\twarp\tw1\t1,1,geffen,5,6\n\
prontera,13,23,2\tshop\tS\t117,501:-1\n";
    let (recs, err) = records(src);
    assert_eq!(err, None);
    let names: Vec<_> = recs.iter().map(|r| r.w3.as_str()).collect();
    assert_eq!(names, ["A", "B", "w1", "S"]);
    assert_eq!(recs[0].line, 4);
    assert_eq!(recs[1].line, 9);
    assert_eq!(recs[2].line, 9, "record after a body on the same line");
    assert_eq!(recs[3].line, 10);
    assert_eq!(recs[3].w4, "117,501:-1");
}

#[test]
fn tool_npc_02_single_quotes_are_not_special() {
    // rAthena counts the brace inside '...', so A's body only closes at the lone '}' line.
    let src = b"m,1,1,0\tscript\tA\t1,{ x = '{'; }\nm,2,2,0\tscript\tB\t1,{ }\n} m,3,3,0\tscript\tC\t1,{ }\n";
    let (recs, err) = records(src);
    assert_eq!(err, None);
    let names: Vec<_> = recs.iter().map(|r| r.w3.as_str()).collect();
    assert_eq!(names, ["A", "C"]);
}

#[test]
fn tool_npc_02_crlf_tab_rest_and_cp949() {
    let mut src = b"m,1,1,0\tshop\tCrLf\t117,501:1\t// tail\r\n".to_vec();
    src.extend_from_slice(b"m,2,2,0\tscript\tManager \xa5\xe1#x\t117,{ }\n");
    let (recs, err) = records(&src);
    assert_eq!(err, None);
    assert_eq!(
        recs[0].w4, "117,501:1\t// tail",
        "w4 keeps further tabs, CR stripped"
    );
    assert_eq!(recs[1].w3, "Manager \u{3b1}#x", "CP949 header decoded");
}

#[test]
fn tool_npc_02_fatal_errors_stop_the_file() {
    let cases: [(&[u8], LexError); 5] = [
        (b"\xEF\xBB\xBFm,1,1,0\twarp\tw\t1,1,m,1,1\n", LexError::Bom),
        (
            b"m,1,1,0\twarp\tw\t1,1,m,1,1\nonly\ttwo\n",
            LexError::TooFewFields { line: 2 },
        ),
        (
            b"m,1,1,0\tscript\tA\t117\n",
            LexError::MissingBody { line: 1 },
        ),
        (
            b"m,1,1,0\tscript\tA\t117,{ {\n}\n",
            LexError::UnterminatedBody { line: 1 },
        ),
        (
            b"m,1,1,0\tscript\tA\t117,{ \"open\n\" }\n",
            LexError::NewlineInString { line: 1 },
        ),
    ];
    for (src, want) in cases {
        let (_, err) = records(src);
        assert_eq!(err, Some(want));
    }
    let (recs, _) = records(b"m,1,1,0\twarp\tw\t1,1,m,1,1\nonly\ttwo\n");
    assert_eq!(recs.len(), 1, "records before the error are kept");
}

#[test]
fn tool_npc_02_file_error_is_reported() {
    let (maps, report) = convert(b"m,1,1,0\twarp\tw\t1,1,m,1,1\nonly\ttwo\n");
    assert_eq!(only(&maps, "m").len(), 1);
    assert_eq!(report.file_errors.len(), 1, "{:?}", report.file_errors);
}

// ---------- TOOL-NPC-03: headers ----------

#[test]
fn tool_npc_03_warps() {
    let (maps, report) = convert(
        b"prontera,156,22,0\twarp\tprt001\t1,2,prt_fild08,170,375\n\
prontera,10,10\twarp2(DISABLED)\tprt002\t0,0,geffen,0,0\n",
    );
    let npcs = only(&maps, "prontera");
    assert_eq!(npcs.len(), 2);
    let w = &npcs[0];
    assert_eq!((w.x, w.y, w.dir, w.sprite), (156, 22, 0, 45));
    assert_eq!(w.trigger, Some((1, 2)));
    assert_eq!(
        w.kind,
        NpcKind::Warp(WarpDest {
            map: "prt_fild08".into(),
            x: 170,
            y: 375,
            trigger_while_hidden: false
        })
    );
    assert_eq!(w.source, "npc/test.txt:1");
    let w2 = &npcs[1];
    assert_eq!(w2.state, NpcState::Disabled);
    assert!(matches!(&w2.kind, NpcKind::Warp(d) if d.trigger_while_hidden));
    assert_eq!(report.converted.get("warp"), Some(&1));
    assert_eq!(report.converted.get("warp2"), Some(&1));
}

#[test]
fn tool_npc_03_scripts_and_shops() {
    let (maps, report) = convert(
        b"prontera,1,2,3\tscript\tKafra\t4_F_KAFRA1,{ end; }\n\
prontera,4,5,6\tscript(CLOAKED)\tToucher\tHIDDEN_WARP_NPC,2,3,{ OnTouch: end; }\n\
prontera,7,8,1\tscript(HIDDEN)\tNoArea\t111,-1,-1,{ end; }\n\
1@mcd,144,212,5,0\tscript\tFive\t117,{ end; }\n\
rgsr_in,135,186, 7\tscript\tSpaced\t117,{ end; }\n\
prontera,9,9,8\tshop\tShop\t117,501:-1\n\
prontera,9,10,2\tmarketshop\tMarket\t117,501:-1:10\n\
prontera\tmapflag\tnomemo\n\
prontera,0,0\tmonster\tPoring\t1002,10,0,0,0\n\
prontera,0,0\tboss_monster\tBoss\t1002,1,0,0,0\n\
function\tscript\tF_Fn\t{ return; }\n\
prontera,1,1,1\tbogus\tX\t1\n",
    );
    let p = only(&maps, "prontera");
    assert_eq!(p.len(), 5);
    assert_eq!(
        (p[0].sprite, p[0].trigger, p[0].kind.clone()),
        (117, None, NpcKind::Script)
    );
    assert_eq!(
        (p[1].sprite, p[1].trigger, p[1].state),
        (139, Some((2, 3)), NpcState::Cloaked)
    );
    assert_eq!((p[2].trigger, p[2].state), (None, NpcState::Hidden));
    assert_eq!((p[3].dir, p[3].kind.clone()), (8, NpcKind::Shop));
    assert_eq!(p[4].kind, NpcKind::Shop);
    assert_eq!(
        (only(&maps, "1@mcd")[0].x, only(&maps, "1@mcd")[0].dir),
        (144, 5)
    );
    assert_eq!(only(&maps, "rgsr_in")[0].dir, 7);
    for (reason, n) in [("mapflag", 1), ("function", 1), ("unknown keyword", 1)] {
        assert_eq!(report.skipped.get(reason), Some(&n), "{reason}: {report:?}");
    }
    assert!(
        !report.skipped.contains_key("monster") && !report.skipped.contains_key("boss_monster"),
        "spawn lines are converted, not skipped: {report:?}"
    );
    assert_eq!(report.mobs.converted.get("monster"), Some(&1));
    assert_eq!(report.mobs.converted.get("boss_monster"), Some(&1));
    assert_eq!(report.converted.get("shop"), Some(&1));
    assert_eq!(report.converted.get("marketshop"), Some(&1));
}

#[test]
fn tool_npc_03_floating_entries_are_not_placed() {
    let (maps, report) =
        convert(b"-\tscript\tFloater\tFAKE_NPC,{ end; }\n-\tshop\tFShop\t-1,501:1\n");
    assert!(maps.is_empty());
    assert_eq!(report.skipped.get("floating"), Some(&2));
    assert!(
        report.warnings.is_empty(),
        "floating sprites are not resolved: {:?}",
        report.warnings
    );
}

// ---------- TOOL-NPC-04: names ----------

#[test]
fn tool_npc_04_names() {
    let long = "N".repeat(60);
    let text = format!(
        "m,1,1,0\tscript\tKafra Employee::kafra_1\t117,{{ }}\n\
m,2,2,0\tscript\tGuide#prt\t117,{{ }}\n\
m,3,3,0\tscript\t{long}\t117,{{ }}\n\
m,4,4,0\tscript\tEmpty::\t117,{{ }}\n\
m,5,5,0\tscript\tGuide#prt\t117,{{ }}\n"
    );
    let (maps, report) = convert(text.as_bytes());
    let m = only(&maps, "m");
    assert_eq!(
        (m[0].name.as_str(), m[0].unique_name.as_str()),
        ("Kafra Employee", "kafra_1")
    );
    assert_eq!(
        (m[1].name.as_str(), m[1].unique_name.as_str()),
        ("Guide#prt", "Guide#prt")
    );
    assert_eq!(m[2].name.len(), 50);
    assert_eq!(
        (m[3].name.as_str(), m[3].unique_name.as_str()),
        ("Empty", "0_m_4_4")
    );
    assert_eq!(
        (m[4].name.as_str(), m[4].unique_name.as_str()),
        ("Guide#prt", "0_m_5_5")
    );
    assert_eq!(
        report.warnings.len(),
        2,
        "truncation and collision warn: {:?}",
        report.warnings
    );
    assert!(
        report.warnings[0].contains("truncated"),
        "{:?}",
        report.warnings
    );
    assert!(
        report.warnings[1].contains("taken"),
        "{:?}",
        report.warnings
    );
}

#[test]
fn tool_npc_04_rejected_npc_is_not_a_duplicate_source() {
    let (maps, report) =
        convert(b"m,-1,5\twarp\tbadwarp\t1,1,g,1,1\nm,2,2,0\tduplicate(badwarp)\tcopy\t1,1\n");
    assert!(maps.is_empty());
    assert_eq!(report.skipped.get("negative coordinates"), Some(&1));
    assert_eq!(report.skipped.get("duplicate of unknown source"), Some(&1));
}

// ---------- TOOL-NPC-05: sprites ----------

#[test]
fn tool_npc_05_sprite_resolution() {
    let (maps, report) = convert(
        b"m,1,1,0\tscript\tA\t0x75,{ }\n\
m,2,2,0\tscript\tB\twarpnpc,{ }\n\
m,3,3,0\tscript\tC\tPoring,{ }\n\
m,4,4,0\tscript\tD\t1002,{ }\n\
m,5,5,0\tscript\tE\tNO_SUCH_SPRITE,{ }\n\
m,6,6,0\tscript\tF\t\tCLEAR_NPC,{ }\n\
m,7,7,0\tscript\tG\t117// comment,{ }\n\
m,8,8,0\tscript\tH\t-1,{ }\n",
    );
    let m = only(&maps, "m");
    let got: Vec<_> = m.iter().map(|n| (n.sprite, n.sprite_is_mob)).collect();
    assert_eq!(
        got,
        [
            (117, false),
            (45, false),
            (1002, true),
            (1002, true),
            (32767, false),
            (0, false),
            (117, false),
            (-1, false)
        ]
    );
    assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
    assert!(report.warnings[0].contains("NO_SUCH_SPRITE"));
}

// ---------- TOOL-NPC-06: duplicates ----------

#[test]
fn tool_npc_06_duplicates() {
    let mut imp = importer();
    imp.add_file(
        "npc/a.txt",
        b"-\tscript(CLOAKED)\tdummy_cloaked\tFAKE_NPC,{ end; }\n\
src_map,1,1,0\twarp2\tsrcwarp\t1,1,dest_map,7,8\n\
src_map,2,2,4\tshop\tsrcshop\t117,501:1\n",
    );
    imp.add_file(
        "npc/b.txt",
        b"m,10,10,2\tduplicate(dummy_cloaked)\tCopy#1\t117,2,2\n\
m,11,11,0\tduplicate(srcwarp)\twarpcopy\t3,4\n\
m,12,12,6\tduplicate(srcshop)\tshopcopy\tPORING\n\
m,13,13,6\tduplicate(Copy#1)\tcopycopy\t117\n\
m,14,14,6\tduplicate(nowhere)\torphan\t117\n\
-\tduplicate(srcshop)\tfloatcopy\t-1\n",
    );
    let Imported {
        npcs: maps, report, ..
    } = imp.finish();
    let m = only(&maps, "m");
    assert_eq!(m.len(), 4);
    assert_eq!(m[0].duplicate_of.as_deref(), Some("dummy_cloaked"));
    assert_eq!(
        (m[0].state, m[0].trigger, m[0].kind.clone()),
        (NpcState::Cloaked, Some((2, 2)), NpcKind::Script)
    );
    assert_eq!((m[1].sprite, m[1].trigger), (45, Some((3, 4))));
    assert_eq!(
        m[1].kind,
        NpcKind::Warp(WarpDest {
            map: "dest_map".into(),
            x: 7,
            y: 8,
            trigger_while_hidden: true
        })
    );
    assert_eq!(
        (m[2].kind.clone(), m[2].sprite_is_mob),
        (NpcKind::Shop, true)
    );
    assert_eq!(
        (m[3].kind.clone(), m[3].state),
        (NpcKind::Script, NpcState::Cloaked)
    );
    assert_eq!(m[3].duplicate_of.as_deref(), Some("Copy#1"));
    assert_eq!(report.skipped.get("duplicate of unknown source"), Some(&1));
    assert_eq!(report.converted.get("duplicate"), Some(&4));
}

// ---------- TOOL-NPC-07: output ----------

#[test]
fn tool_npc_07_rendered_file_loads_with_header() {
    let (maps, _) = convert(b"prontera,156,22,0\twarp\tprt001\t1,1,prt_fild08,170,375\n");
    let text = render_map_file(&maps["prontera"], "abc123").expect("renders");
    assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
    assert!(text.contains("rAthena (https://github.com/rathena/rathena) commit abc123"));
    let loaded = MapNpcs::from_yaml_str("prontera.yml", &text).expect("loads");
    assert_eq!(loaded, maps["prontera"]);
}

#[test]
fn tool_npc_07_write_output_replaces_only_yml() {
    let t = TempDir::new("out");
    t.write("stale.yml", "old");
    t.write("LICENSE", "keep");
    let (maps, _) = convert(b"geffen,1,1,0\twarp\tg\t1,1,prontera,1,1\n");
    let written = npc_import::write_output(&t.0, &maps, "c").expect("writes");
    assert_eq!(written, 1);
    assert!(!t.0.join("stale.yml").exists());
    assert!(t.0.join("LICENSE").exists());
    assert!(t.0.join("geffen.yml").exists());
}

/// Full run against a real rAthena checkout; counts from `npc_files.md` §6 (commit d4b8e7b).
/// Skipped when `RATHENA_PATH` is unset.
#[test]
fn tool_npc_07_counts_match_catalog() {
    let Some(root) = std::env::var_os("RATHENA_PATH").map(PathBuf::from) else {
        return;
    };
    if !Path::new(&root).join("npc/re/scripts_main.conf").is_file() {
        return;
    }
    let Imported {
        npcs: maps, report, ..
    } = npc_import::import(&root).expect("import runs");
    assert_eq!(report.files, 835);
    for (kind, n) in [
        ("warp", 3876),
        ("warp2", 229),
        ("script", 8635),
        ("duplicate", 10377),
        ("shop", 270),
    ] {
        assert_eq!(report.converted.get(kind), Some(&n), "{kind}");
    }
    assert_eq!(maps.len(), 838);
    assert!(report.file_errors.is_empty(), "{:?}", report.file_errors);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

// ---------- TOOL-MOB-01..05: mob spawn lines ----------

fn mobs_of(out: &Imported, map: &str) -> Vec<MobSpawn> {
    out.mobs
        .get(map)
        .map(|m| m.spawns.clone())
        .unwrap_or_default()
}

#[test]
fn tool_mob_01_spawn_lines_are_converted() {
    let out = import_one(
        b"prt_fild08,0,0\tmonster\tPoring\t1002,20,5000\n\
prt_fild00,285,138,10,10\tMONSTER\tGreen Plant\t1063,5,360000,180000\n\
in_sphinx5,0,0,0,0\tBoss_Monster\tPharaoh\t1002,1,3600000,600000,1\n\
ein_dun01,30,40\tmonster\tSpaced\t 20255,2\n\
tra_fild,0,0\tmonster\tCommented\t1002,3,5000\t// a comment, 7, 8\n",
    );
    let poring = &mobs_of(&out, "prt_fild08")[0];
    assert_eq!(
        *poring,
        MobSpawn {
            name: "Poring".into(),
            mob_id: MobId(1002),
            x: 0,
            y: 0,
            span: (0, 0),
            amount: 20,
            delay1: 5000,
            delay2: 0,
            level: None,
            boss: false,
            event: None,
            source: "npc/test.txt:1".into(),
        }
    );
    let plant = &mobs_of(&out, "prt_fild00")[0];
    assert_eq!(
        (plant.x, plant.y, plant.span, plant.amount),
        (285, 138, (10, 10), 5)
    );
    assert_eq!((plant.delay1, plant.delay2), (360_000, 180_000));
    assert!(!plant.boss, "keyword match is case-insensitive");
    let pharaoh = &mobs_of(&out, "in_sphinx5")[0];
    assert!(pharaoh.boss);
    assert_eq!(pharaoh.event, None, "event '1' is dropped");
    let spaced = &mobs_of(&out, "ein_dun01")[0];
    assert_eq!(spaced.mob_id, MobId(20255), "leading space is trimmed");
    assert_eq!(spaced.delay1, 5000, "absent delay1 is 5000");
    let commented = &mobs_of(&out, "tra_fild")[0];
    assert_eq!(
        (commented.amount, commented.delay1, commented.event.clone()),
        (3, 5000, None),
        "trailing tab comment ignored"
    );
    assert_eq!(out.report.mobs.converted.get("monster"), Some(&4));
    assert_eq!(out.report.mobs.converted.get("boss_monster"), Some(&1));
    assert!(out.report.mobs.skipped.is_empty(), "{:?}", out.report.mobs);
    assert!(
        out.report.mobs.warnings.is_empty(),
        "{:?}",
        out.report.mobs.warnings
    );
    assert!(out.npcs.is_empty(), "spawn lines are not NPCs");
}

#[test]
fn tool_mob_02_location() {
    let out = import_one(
        b"bare\tmonster\tA\t1002,1\n\
partial,7\tmonster\tB\t1002,1\n\
neg_x,-1,5\tmonster\tC\t1002,1\n\
neg_y,5,-1\tmonster\tD\t1002,1\n\
neg_span,10,10,-3,2\tmonster\tE\t1002,1\n",
    );
    let bare = &mobs_of(&out, "bare")[0];
    assert_eq!(
        (bare.x, bare.y, bare.span),
        (0, 0, (0, 0)),
        "bare map is map-wide"
    );
    let partial = &mobs_of(&out, "partial")[0];
    assert_eq!((partial.x, partial.y), (7, 0), "missing numbers are 0");
    assert!(mobs_of(&out, "neg_x").is_empty());
    assert!(mobs_of(&out, "neg_y").is_empty());
    assert_eq!(
        out.report.mobs.skipped.get("negative coordinates"),
        Some(&2)
    );
    let neg_span = &mobs_of(&out, "neg_span")[0];
    assert_eq!(neg_span.span, (0, 2), "negative span becomes 0");
    let warnings = &out.report.mobs.warnings;
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(warnings[2].contains("span"), "{warnings:?}");
}

#[test]
fn tool_mob_03_names_and_levels() {
    let long = "W".repeat(31);
    let text = format!(
        "m,1,1\tmonster\t{long}\t1002,1\n\
m,2,2\tmonster\t--en--\t1063,1\n\
m,3,3\tmonster\t--ja--\t1063,1\n\
m,4,4\tmonster\t--ja--\t20255,1\n\
m,5,5\tmonster\tLeveled,50\t1002,1\n\
m,6,6\tmonster\tMax,275\t1002,1\n\
m,7,7\tmonster\tMinusOne,-1\t1002,1\n\
m,8,8\tmonster\tMinusFive,-5\t1002,1\n\
m,9,9\tmonster\tText,abc\t1002,1\n\
z,1,1\tmonster\tZero,0\t1002,1\n\
z,2,2\tmonster\tHigh,276\t1002,1\n"
    );
    let out = import_one(text.as_bytes());
    let m = mobs_of(&out, "m");
    let got: Vec<_> = m.iter().map(|s| (s.name.as_str(), s.level)).collect();
    assert_eq!(
        got,
        [
            ("WWWWWWWWWWWWWWWWWWWWWWW", None),
            ("Lunatic", None),
            ("Lunatic JP", None),
            ("L Blue Earth Spirit", None),
            ("Leveled", Some(50)),
            ("Max", Some(275)),
            ("MinusOne", None),
            ("MinusFive", None),
            ("Text", None),
        ]
    );
    assert!(mobs_of(&out, "z").is_empty());
    assert_eq!(out.report.mobs.skipped.get("bad level"), Some(&2));
    let warnings = &out.report.mobs.warnings;
    assert!(warnings[0].contains("name cut"), "{warnings:?}");
    assert_eq!(
        warnings.len(),
        3,
        "cut name plus two bad levels: {warnings:?}"
    );
}

#[test]
fn tool_mob_04_mob_ref_amount_and_extras() {
    let out = import_one(
        b"m,1,1\tmonster\tHex\t0x3EA,1\n\
m,2,2\tmonster\tAegis\tlunatic,1\n\
m,3,3\tmonster\tMaxAmount\t1002,1000\n\
m,4,4\tmonster\tLabel\t1002,1,5000,0,\"ant_d02_i_boss::OnMobDead\"\n\
m,5,5\tmonster\tShortLabel\t1002,1,5000,0,abc\n\
m,6,6\tmonster\tBig\t1002,1,5000,0,\"x::OnDead\",2\n\
m,7,7\tmonster\tAi\t1002,1,5000,0,\"x::OnDead\",-1,1\n\
m,8,8\tmonster\tIgnoredExtras\t1002,1,5000,0,\"x::OnDead\",-1,0\n\
bad,1,1\tmonster\tUnknownNum\t1001,1\n\
bad,2,2\tmonster\tUnknownName\tNO_SUCH_MOB,1\n\
bad,3,3\tmonster\tTrailing\t1002x,1\n\
bad,4,4\tmonster\tZero\t1002,0\n\
bad,5,5\tmonster\tTooMany\t1002,1001\n",
    );
    let m = mobs_of(&out, "m");
    let ids: Vec<_> = m.iter().map(|s| s.mob_id.0).collect();
    assert_eq!(ids, [1002, 1063, 1002, 1002, 1002, 1002, 1002, 1002]);
    assert_eq!(m[2].amount, 1000);
    assert_eq!(m[3].event.as_deref(), Some("ant_d02_i_boss::OnMobDead"));
    assert_eq!(m[4].event, None, "under 4 chars is dropped");
    assert_eq!(m[5].event.as_deref(), Some("x::OnDead"));
    assert!(mobs_of(&out, "bad").is_empty());
    let skipped = &out.report.mobs.skipped;
    assert_eq!(skipped.get("unknown mob"), Some(&3), "{skipped:?}");
    assert_eq!(skipped.get("bad amount"), Some(&2), "{skipped:?}");
    let extras: Vec<_> = out
        .report
        .mobs
        .warnings
        .iter()
        .filter(|w| w.contains("not supported"))
        .collect();
    assert_eq!(extras.len(), 2, "size 2 and ai 1 warn: {extras:?}");
}

#[test]
fn tool_mob_04_bad_lines_are_mob_bad_headers() {
    let out = import_one(b"m,1,1\tmonster\t\t1002,1\nm,1,1\tBOSS_MONSTER\tNoAmount\t1002\n");
    let mobs = &out.report.mobs;
    assert_eq!(mobs.skipped.get("bad header"), Some(&2), "{mobs:?}");
    assert_eq!(mobs.warnings.len(), 2, "{mobs:?}");
    assert!(
        out.report.skipped.is_empty() && out.report.warnings.is_empty(),
        "mob lines stay out of the NPC report: {:?}",
        out.report
    );
}

#[test]
fn tool_mob_04_out_of_range_ids_never_resolve() {
    let out = import_one(b"m,1,1\tmonster\tNum\t5000,1\nm,2,2\tmonster\tName\tout_of_range,1\n");
    assert!(out.mobs.is_empty());
    assert_eq!(out.report.mobs.skipped.get("unknown mob"), Some(&2));
}

#[test]
fn tool_mob_04_scan_stops_like_scanf() {
    let long = "W".repeat(31);
    let text = format!(
        "a,1,1\tmonster\tJunk\t1002,5x,60000,0,\"a::OnX\"\n\
b,1,1\tmonster\t{long},50\t1002,1\n\
c,1,1\tmonster\tDelayJunk\t1002,1,6000 ms,7000,\"a::OnX\"\n"
    );
    let out = import_one(text.as_bytes());
    let a = &mobs_of(&out, "a")[0];
    assert_eq!(
        (a.amount, a.delay1, a.event.clone()),
        (5, 5000, None),
        "text after the amount stops the scan"
    );
    let b = &mobs_of(&out, "b")[0];
    assert_eq!(
        b.level, None,
        "a name over 23 bytes leaves the level unread"
    );
    let c = &mobs_of(&out, "c")[0];
    assert_eq!(
        (c.delay1, c.delay2, c.event.clone()),
        (6000, 0, None),
        "text after a delay stops the scan"
    );
}

#[test]
fn tool_mob_05_rendered_file_loads_and_report_totals() {
    let out = import_one(
        b"a,0,0\tmonster\tPoring\t1002,20\n\
b,0,0\tmonster\tPoring\t1002,30\n\
b,5,5\tboss_monster\tLunatic\t1063,1,5000,600000\n",
    );
    let text = render_map_file(&out.mobs["b"], "abc123").expect("renders");
    assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
    assert!(text.contains("rAthena (https://github.com/rathena/rathena) commit abc123"));
    let loaded = MapMobSpawns::from_yaml_str("b.yml", &text).expect("loads");
    assert_eq!(loaded, out.mobs["b"]);
    assert_eq!(out.report.mobs.total_amount, 51);
    assert_eq!(out.report.mobs.largest_map, Some(("b".to_string(), 31)));

    let t = TempDir::new("mob-out");
    t.write("stale.yml", "old");
    let written = npc_import::write_output(&t.0, &out.mobs, "c").expect("writes");
    assert_eq!(written, 2);
    assert!(!t.0.join("stale.yml").exists());
    assert!(t.0.join("a.yml").exists() && t.0.join("b.yml").exists());
}

/// Full run against a real rAthena checkout; counts from `mechanics/mobs.md` §1.6 (commit
/// d4b8e7b). Skipped when `RATHENA_PATH` is unset.
#[test]
fn tool_mob_05_counts_match_catalog() {
    let Some(root) = std::env::var_os("RATHENA_PATH").map(PathBuf::from) else {
        return;
    };
    if !Path::new(&root).join("npc/re/scripts_main.conf").is_file() {
        return;
    }
    let out = npc_import::import(&root).expect("import runs");
    let mobs = &out.report.mobs;
    assert_eq!(mobs.converted.get("monster"), Some(&3814));
    assert_eq!(mobs.converted.get("boss_monster"), Some(&75));
    assert_eq!(mobs.total_amount, 66040);
    assert_eq!(out.mobs.len(), 416);
    assert!(mobs.skipped.is_empty(), "{:?}", mobs.skipped);
    assert!(
        mobs.warnings.iter().all(|w| w.contains("name cut")),
        "only long names warn: {:?}",
        mobs.warnings
    );
}
