//! Permanent mob spawn lines (TOOL-MOB-01..05).

use std::path::{Path, PathBuf};

use npc_import::render_map_file;
use ro_db::{MapMobSpawns, MobId, MobSpawn};

use crate::common::*;

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
