//! rAthena job files conversion (spec §1.1).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use job_import::{convert, render_jobs, render_status_points, Converted};
use ro_db::{JobBonus, JobBonusEntry, JobData, JobDb, WeaponType};

fn file(kind: &str, body: &str) -> String {
    format!("Header:\n  Type: {kind}\n  Version: 2\nBody:\n{body}")
}

const STATS: &str = "  - Jobs:
      Novice: true
      Novice_High: true
    BonusStats:
      - Level: 2
        Luk: 1
      - Level: 3
        Dex: 1
  - Jobs:
      Shadow_Cross: true
    MaxWeight: 42000
    HpFactor: 64
    HpIncrease: 5398
    SpFactor: 7
    SpIncrease: 4
    ApFactor: 50
    ApIncrease: 300
    BonusStats:
      - Level: 1
        Pow: 1
  - Jobs:
      Knight: true
      Knight2: true
    MaxWeight: 28000
";

const ASPD: &str = "  - Jobs:
      Novice: true
    BaseASPD:
      Fist: 40
      Dagger: 55
      1hSword: 57
      2hStaff: 65
      Shield: 10
  - Jobs:
      Knight: true
    BaseASPD:
      Fist: 40
      Shield: 5
      Lance: 99
";

const EXP: &str = "  - Jobs:
      Hyper_Novice: true
    MaxBaseLevel: 151
    MaxJobLevel: 1
  - Jobs:
      Novice: true
      Knight: true
    MaxBaseLevel: 3
    BaseExp:
      - Level: 1
        Exp: 548
  - Jobs:
      Novice: true
    MaxJobLevel: 3
    JobExp:
      - Level: 1
        Exp: 10
  - Jobs:
      Knight: true
    MaxJobLevel: 2
  - Jobs:
      Shadow_Cross: true
    MaxBaseLevel: 170
    MaxJobLevel: 2
  - Jobs:
      Rune_Knight: true
    MaxBaseLevel: 201
    MaxJobLevel: 1
  - Jobs:
      Dragon_Knight: true
    MaxBaseLevel: 202
    MaxJobLevel: 1
  - Jobs:
      Novice: false
    MaxBaseLevel: 1
";

const POINTS: &str = "  - Jobs:
      Novice: true
    BaseHp:
      - Level: 1
        Hp: 40
      - Level: 3
        Hp: 53
    BaseSp:
      - Level: 1
        Sp: 11
      - Level: 2
        Sp: 13
      - Level: 3
        Sp: 15
      - Level: 4
        Sp: 17
  - Jobs:
      Knight: true
    BaseHp:
      - Level: 1
        Hp: 40
  - Jobs:
      Knight: true
    BaseHp:
      - Level: 2
        Hp: 48
    BaseAp:
      - Level: 1
        Ap: 1
  - Jobs:
      Dragon_Knight: true
    BaseAp:
      - Level: 200
        Ap: 0
      - Level: 201
        Ap: 250
";

const STATPOINT: &str = "  - Level: 1
    Points: 48
  - Level: 2
    Points: 51
    TraitPoints: 0
  - Level: 3
    Points: 54
    TraitPoints: 3
";

fn run() -> Converted {
    let sources = [
        ("db/re/job_stats.yml", file("JOB_STATS", STATS)),
        ("db/re/job_aspd.yml", file("JOB_STATS", ASPD)),
        ("db/re/job_exp.yml", file("JOB_STATS", EXP)),
        ("db/re/job_basepoints.yml", file("JOB_STATS", POINTS)),
    ];
    let refs: Vec<(&str, &str)> = sources.iter().map(|(l, t)| (*l, t.as_str())).collect();
    convert(&refs, &file("STATPOINT_DB", STATPOINT)).expect("converts")
}

fn job(out: &Converted, id: u16) -> &JobData {
    out.jobs
        .iter()
        .find(|j| j.id == id)
        .unwrap_or_else(|| panic!("job {id} converted"))
}

#[test]
fn names_resolve_to_ids_and_keep_the_file_spelling() {
    let out = run();
    let ids: Vec<u16> = out.jobs.iter().map(|j| j.id).collect();
    assert_eq!(
        ids,
        vec![0, 7, 4001, 4054, 4252, 4254, 4307],
        "sorted by id; Knight2 has no id"
    );
    assert_eq!(job(&out, 4001).name, "Novice_High");
    assert_eq!(job(&out, 4254).name, "Shadow_Cross");
    assert!(
        out.report.unknown_jobs.contains("Knight2"),
        "{:?}",
        out.report.unknown_jobs
    );
}

#[test]
fn keys_merge_across_files_and_entries() {
    let out = run();
    let novice = job(&out, 0);
    assert_eq!(novice.max_weight, 20000, "default MaxWeight");
    assert_eq!((novice.max_base_level, novice.max_job_level), (3, 3));
    assert_eq!(
        novice.bonus_stats,
        vec![
            JobBonusEntry {
                level: 2,
                gain: JobBonus {
                    luk: 1,
                    ..JobBonus::default()
                }
            },
            JobBonusEntry {
                level: 3,
                gain: JobBonus {
                    dex: 1,
                    ..JobBonus::default()
                }
            },
        ]
    );
    let knight = job(&out, 7);
    assert_eq!(knight.max_weight, 28000);
    assert_eq!((knight.max_base_level, knight.max_job_level), (3, 2));
    assert_eq!(
        knight.base_hp[..2],
        [40, 48],
        "two BaseHp entries for one job merge"
    );
    let high = job(&out, 4001);
    assert_eq!(
        (high.max_base_level, high.max_job_level),
        (275, 275),
        "no job_exp entry: rAthena's MAX_LEVEL"
    );
    assert!(out
        .report
        .warnings
        .iter()
        .any(|w| w.contains("Novice_High")));
}

#[test]
fn aspd_names_map_to_weapon_types() {
    let out = run();
    let novice = job(&out, 0);
    assert_eq!(novice.base_aspd.get(&WeaponType::Fist), Some(&40));
    assert_eq!(novice.base_aspd.get(&WeaponType::OneHandSword), Some(&57));
    assert_eq!(novice.base_aspd.get(&WeaponType::TwoHandStaff), Some(&65));
    assert_eq!(novice.shield_aspd, Some(10));
    assert!(
        out.report.warnings.iter().any(|w| w.contains("Lance")),
        "an unknown weapon name is reported: {:?}",
        out.report.warnings
    );
}

#[test]
fn missing_table_slots_use_the_fallback_formula() {
    let out = run();
    let novice = job(&out, 0);
    // Slot 2 is missing: 35 + 2*500/100 + round(0*2) = 45 (defaults HpFactor 0, HpIncrease 500).
    assert_eq!(novice.base_hp, vec![40, 45, 53]);
    assert_eq!(
        novice.base_sp,
        vec![11, 13, 15],
        "longer tables are cut at MaxBaseLevel"
    );
    // No tables: every slot from the formula. Level 170 matches the captured Shadow Cross
    // (MaxHP 24066 = 18513 * 1.04 * 1.25, MaxSP 1331 = 1034 * 1.03 * 1.25).
    let cross = job(&out, 4254);
    assert_eq!(cross.base_hp.len(), 170);
    assert_eq!((cross.base_hp[0], cross.base_sp[0]), (35 + 53, 10));
    assert_eq!((cross.base_hp[169], cross.base_sp[169]), (18513, 1034));
    assert!(out
        .report
        .fallback
        .iter()
        .any(|(name, hp, sp)| name == "Shadow_Cross" && (*hp, *sp) == (170, 170)));
}

/// The Super Novice line's fallback HP gets +2000 from base 99 and +2000 more from 150 (catalog
/// `mechanics/stats.md` "Hyper Novice"); SP doesn't.
#[test]
fn super_novice_line_fallback_hp_adjustment() {
    let out = run();
    let hyper = job(&out, 4307);
    let plain = |level: u32| 35 + level * 500 / 100;
    assert_eq!(hyper.base_hp[97], plain(98), "level 98: no adjustment");
    assert_eq!(hyper.base_hp[98], plain(99) + 2000);
    assert_eq!(hyper.base_hp[148], plain(149) + 2000);
    assert_eq!(hyper.base_hp[149], plain(150) + 4000);
    assert_eq!(hyper.base_sp[149], 10 + 150, "SP is not adjusted");
    let cross = job(&out, 4254);
    assert_eq!(
        cross.base_hp[169], 18513,
        "other jobs keep the plain formula"
    );
}

#[test]
fn status_points_keep_trait_points() {
    let out = run();
    let rows: Vec<(u16, u32, u32)> = out
        .points
        .iter()
        .map(|r| (r.level, r.points, r.trait_points))
        .collect();
    assert_eq!(rows, vec![(1, 48, 0), (2, 51, 0), (3, 54, 3)]);
}

/// The AP table is `BaseAp` where it has a non-zero value, else rAthena's fallback (catalog
/// `mechanics/trait_stats.md` §2): 200 from base level 200, plus `floor(lv * ApIncrease / 100)`,
/// plus `round(ApFactor / 100 * i)` for i = 2..=lv. The fallback has no job guard.
#[test]
fn ap_tables_from_base_ap_and_the_fallback_formula() {
    let out = run();
    // ApFactor 50, ApIncrease 300, no BaseAp. Level 3: 9 + round(1.0) + round(1.5) = 12.
    // Level 170: 510 + (1 + 2 * (2 + ... + 85)) = 510 + 7309.
    let cross = job(&out, 4254);
    assert_eq!(cross.base_ap.len(), 170);
    assert_eq!(cross.base_ap[..3], [3, 7, 12]);
    assert_eq!(cross.base_ap[169], 7819);
    // No AP keys: 0 below 200, then the constant 200.
    let rune = job(&out, 4054);
    assert_eq!(rune.base_ap.len(), 201);
    assert_eq!(
        (rune.base_ap[198], rune.base_ap[199], rune.base_ap[200]),
        (0, 200, 200)
    );
    // A BaseAp of 0 falls back too; a non-zero one wins.
    let dk = job(&out, 4252);
    assert_eq!(dk.base_ap[199..], [200, 250, 200]);
    // Only the given slot is non-zero: the table is kept.
    assert_eq!(job(&out, 7).base_ap, vec![1, 0, 0]);
    // All zero: omitted.
    assert!(job(&out, 0).base_ap.is_empty());
    assert!(job(&out, 4307).base_ap.is_empty(), "max base level 151");
}

#[test]
fn rendered_files_carry_the_header_and_load() {
    let out = run();
    let jobs = render_jobs(&out.jobs, "abc123").expect("render jobs");
    let points = render_status_points(&out.points, "abc123").expect("render points");
    for text in [&jobs, &points] {
        assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
        assert!(text.contains("commit abc123"));
        assert!(text.contains("tools/job-import"));
        assert!(text.contains("crates/ro-db/data/NOTICE.md"));
    }
    let parsed = JobDb::parse_jobs("jobs", &jobs).expect("parses");
    let rows = JobDb::parse_status_points("points", &points).expect("parses");
    let db = JobDb::from_parts("x", parsed, &rows).expect("loads");
    assert_eq!(
        db.by_name("shadow_cross").map(|j| j.max_weight),
        Some(42000)
    );
}
