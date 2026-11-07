//! Job registry YAML (`JOB_DATA`, `STATPOINT_DATA`) loading (spec DB-JOB-01..05).

use std::path::Path;

use ro_db::{
    DbLoadError, JobBonus, JobDb, WeaponType, JOB_DATA_FILE, JOB_EXP_DATA_FILE,
    NEXT_BASE_EXP_AT_MAX, NEXT_JOB_EXP_AT_MAX, STATPOINT_DATA_FILE,
};

const JOBS: &str = r"
Header:
  Type: JOB_DATA
  Version: 2
Body:
  - Id: 0
    Name: Novice
    MaxWeight: 20000
    MaxBaseLevel: 3
    MaxJobLevel: 4
    BonusStats:
      - Level: 2
        Luk: 1
      - Level: 3
        Str: 1
        Dex: 2
    BaseAspd:
      Fist: 40
      Dagger: 55
    ShieldAspd: 10
    BaseHp: [40, 46, 53]
    BaseSp: [11, 13, 15]
  - Id: 7
    Name: Knight
    MaxWeight: 28000
    MaxBaseLevel: 2
    MaxJobLevel: 2
    BonusStats:
      - Level: 1
        Vit: 1
        Pow: 3
    BaseHp: [40, 48]
    BaseSp: [13, 15]
    BaseAp: [0, 200]
";

const POINTS: &str = r"
Header:
  Type: STATPOINT_DATA
  Version: 2
Body:
  - Level: 1
    Points: 48
  - Level: 2
    Points: 51
    TraitPoints: 3
  - Level: 3
    Points: 54
    TraitPoints: 10
";

const EXP: &str = r"
Header:
  Type: JOB_EXP_DATA
  Version: 1
Body:
  - Kind: Base
    Jobs: [0, 7]
    Exp: [548, 894]
  - Kind: Job
    Jobs: [0]
    Exp: [10, 0, 30]
";

fn db() -> JobDb {
    let jobs = JobDb::parse_jobs("jobs", JOBS).expect("fixture jobs parse");
    let points = JobDb::parse_status_points("points", POINTS).expect("fixture points parse");
    let exp = JobDb::parse_exp_tables("exp", EXP).expect("fixture exp parses");
    JobDb::from_parts("fixture", jobs, &points)
        .and_then(|db| db.with_exp_tables("exp", exp))
        .expect("fixture builds")
}

fn schema_error(result: Result<JobDb, DbLoadError>) -> String {
    match result {
        Err(DbLoadError::Schema { message, .. }) => message,
        other => panic!("expected a schema error, got {other:?}"),
    }
}

#[test]
fn job_lookup_by_id_and_name() {
    let db = db();
    assert_eq!(db.len(), 2);
    let novice = db.get(0).expect("Novice by id");
    assert_eq!(novice.name, "Novice");
    assert_eq!(
        (
            novice.max_weight,
            novice.max_base_level,
            novice.max_job_level
        ),
        (20000, 3, 4)
    );
    assert_eq!(
        db.by_name("kNiGhT").map(|j| j.id),
        Some(7),
        "names are case-insensitive"
    );
    assert!(db.get(1).is_none());
    assert!(db.by_name("Swordman").is_none());
}

/// DB-JOB-02 shape: sparse `BonusStats` become one cumulative row per job level.
#[test]
fn job_bonus_accumulates_per_job_level() {
    let db = db();
    let novice = db.get(0).expect("Novice");
    assert_eq!(novice.bonus(1), JobBonus::default());
    assert_eq!(
        novice.bonus(2),
        JobBonus {
            luk: 1,
            ..JobBonus::default()
        }
    );
    let three = JobBonus {
        str: 1,
        dex: 2,
        luk: 1,
        ..JobBonus::default()
    };
    assert_eq!(novice.bonus(3), three);
    assert_eq!(
        novice.bonus(4),
        three,
        "levels without an entry keep the total"
    );
    assert_eq!(
        novice.bonus(0),
        JobBonus::default(),
        "job level 0 reads as 1"
    );
    assert_eq!(novice.bonus(99), three, "past the max reads the last row");
    let knight = db.get(7).expect("Knight");
    assert_eq!(
        knight.bonus(1),
        JobBonus {
            vit: 1,
            pow: 3,
            ..JobBonus::default()
        },
        "trait keys are kept"
    );
}

#[test]
fn base_hp_and_sp_by_base_level() {
    let novice = db().get(0).expect("Novice").clone();
    assert_eq!((novice.base_hp(1), novice.base_sp(1)), (40, 11));
    assert_eq!((novice.base_hp(3), novice.base_sp(3)), (53, 15));
    assert_eq!(novice.base_hp(0), 40, "level 0 reads as 1");
    assert_eq!(novice.base_hp(50), 53, "past the max reads the last slot");
}

#[test]
fn aspd_bases_default_to_200() {
    let db = db();
    let novice = db.get(0).expect("Novice");
    assert_eq!(novice.aspd_base(WeaponType::Fist), 40);
    assert_eq!(novice.aspd_base(WeaponType::Dagger), 55);
    assert_eq!(
        novice.aspd_base(WeaponType::Bow),
        200,
        "a missing weapon is 200"
    );
    assert_eq!(novice.shield_aspd(), 10);
    let knight = db.get(7).expect("Knight");
    assert_eq!(knight.aspd_base(WeaponType::Fist), 200);
    assert_eq!(knight.shield_aspd(), 200, "a missing shield entry is 200");
}

#[test]
fn status_points_are_cumulative_by_base_level() {
    let db = db();
    assert_eq!(db.status_points(1), 48);
    assert_eq!(db.status_points(3), 54);
    assert_eq!(db.status_points(0), 48, "level 0 reads as 1");
    assert_eq!(
        db.status_points(300),
        54,
        "past the table reads the last row"
    );
}

#[test]
fn trait_points_are_cumulative_and_optional() {
    let db = db();
    assert_eq!(db.trait_points(1), 0, "an absent TraitPoints is 0");
    assert_eq!(db.trait_points(2), 3);
    assert_eq!(db.trait_points(3), 10);
    assert_eq!(db.trait_points(0), 0, "level 0 reads as 1");
    assert_eq!(
        db.trait_points(300),
        10,
        "past the table reads the last row"
    );
}

#[test]
fn base_ap_by_base_level() {
    let db = db();
    let knight = db.get(7).expect("Knight");
    assert_eq!((knight.base_ap(1), knight.base_ap(2)), (0, 200));
    assert_eq!(knight.base_ap(9), 200, "clamped to MaxBaseLevel");
    let novice = db.get(0).expect("Novice");
    assert_eq!(novice.base_ap(3), 0, "no BaseAp table");
}

#[test]
fn rejects_an_ap_table_of_the_wrong_length() {
    let text = JOBS.replace("BaseAp: [0, 200]", "BaseAp: [0]");
    let jobs = JobDb::parse_jobs("jobs", &text).expect("parse");
    let points = JobDb::parse_status_points("points", POINTS).expect("parse");
    let message = schema_error(JobDb::from_parts("x", jobs, &points));
    assert!(message.contains("BaseAp"), "{message}");
}

#[test]
fn rejects_the_previous_data_version() {
    let old = JOBS.replace("Version: 2", "Version: 1");
    assert!(matches!(
        JobDb::parse_jobs("jobs", &old),
        Err(DbLoadError::Schema { .. })
    ));
    let old = POINTS.replace("Version: 2", "Version: 1");
    assert!(matches!(
        JobDb::parse_status_points("points", &old),
        Err(DbLoadError::Schema { .. })
    ));
}

#[test]
fn round_trips_through_yaml() {
    let jobs = JobDb::parse_jobs("jobs", JOBS).expect("parse");
    let text = JobDb::jobs_to_yaml(&jobs).expect("serialize");
    assert!(
        text.contains("BaseHp: [40, 46, 53]"),
        "HP/SP tables are written as flow lists:\n{text}"
    );
    assert_eq!(JobDb::parse_jobs("again", &text).expect("reparse"), jobs);
    let points = JobDb::parse_status_points("points", POINTS).expect("parse");
    let text = JobDb::status_points_to_yaml(&points).expect("serialize");
    assert_eq!(
        JobDb::parse_status_points("again", &text).expect("reparse"),
        points
    );
}

#[test]
fn rejects_tables_shorter_than_the_max_level() {
    let text = JOBS.replace("BaseHp: [40, 46, 53]", "BaseHp: [40, 46]");
    let jobs = JobDb::parse_jobs("jobs", &text).expect("parse");
    let points = JobDb::parse_status_points("points", POINTS).expect("parse");
    let message = schema_error(JobDb::from_parts("x", jobs, &points));
    assert!(message.contains("BaseHp"), "{message}");
}

#[test]
fn rejects_bonus_levels_past_the_max_job_level() {
    let text = JOBS.replace(
        "      - Level: 3\n        Str: 1",
        "      - Level: 5\n        Str: 1",
    );
    let jobs = JobDb::parse_jobs("jobs", &text).expect("parse");
    let points = JobDb::parse_status_points("points", POINTS).expect("parse");
    let message = schema_error(JobDb::from_parts("x", jobs, &points));
    assert!(message.contains("BonusStats"), "{message}");
}

#[test]
fn rejects_duplicate_ids_and_names() {
    let points = JobDb::parse_status_points("points", POINTS).expect("parse");
    let dup_id = JOBS.replace("Id: 7", "Id: 0");
    let jobs = JobDb::parse_jobs("jobs", &dup_id).expect("parse");
    assert!(matches!(
        JobDb::from_parts("x", jobs, &points),
        Err(DbLoadError::DuplicateId { id: 0, .. })
    ));
    let dup_name = JOBS.replace("Name: Knight", "Name: NOVICE");
    let jobs = JobDb::parse_jobs("jobs", &dup_name).expect("parse");
    let message = schema_error(JobDb::from_parts("x", jobs, &points));
    assert!(message.contains("NOVICE"), "{message}");
}

#[test]
fn rejects_gaps_in_the_status_point_table() {
    let jobs = JobDb::parse_jobs("jobs", JOBS).expect("parse");
    let text = POINTS.replace("Level: 2", "Level: 4");
    let points = JobDb::parse_status_points("points", &text).expect("parse");
    let message = schema_error(JobDb::from_parts("x", jobs, &points));
    assert!(message.contains("Level"), "{message}");
}

#[test]
fn rejects_unknown_fields_and_wrong_headers() {
    let text = JOBS.replace("MaxWeight: 28000", "MaxWeight: 28000\n    HpFactor: 150");
    assert!(matches!(
        JobDb::parse_jobs("jobs", &text),
        Err(DbLoadError::Parse { .. })
    ));
    let text = POINTS.replace("STATPOINT_DATA", "JOB_DATA");
    assert!(matches!(
        JobDb::parse_status_points("points", &text),
        Err(DbLoadError::Schema { .. })
    ));
}

#[test]
fn load_dir_needs_all_three_files() {
    let dir = std::env::temp_dir().join(format!("ro-db-job-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join(JOB_DATA_FILE), JOBS).expect("write jobs");
    assert!(matches!(JobDb::load_dir(&dir), Err(DbLoadError::Io { .. })));
    std::fs::write(dir.join(STATPOINT_DATA_FILE), POINTS).expect("write points");
    assert!(matches!(JobDb::load_dir(&dir), Err(DbLoadError::Io { .. })));
    std::fs::write(dir.join(JOB_EXP_DATA_FILE), EXP).expect("write exp");
    let db = JobDb::load_dir(&dir).expect("all files load");
    assert_eq!(db.len(), 2);
    assert_eq!(db.next_base_exp(0, 1), 548);
    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn next_exp_reads_the_row_of_the_current_level() {
    let db = db();
    assert_eq!(db.next_base_exp(0, 0), 0, "level 0");
    assert_eq!(db.next_base_exp(0, 1), 548);
    assert_eq!(db.next_base_exp(0, 2), 894);
    assert_eq!(db.next_base_exp(0, 3), NEXT_BASE_EXP_AT_MAX, "at max");
    assert_eq!(db.next_base_exp(0, 200), NEXT_BASE_EXP_AT_MAX);
    assert_eq!(db.next_job_exp(0, 1), 10);
    assert_eq!(db.next_job_exp(0, 2), 0, "a missing row stops the level-up");
    assert_eq!(db.next_job_exp(0, 3), 30);
    assert_eq!(db.next_job_exp(0, 4), NEXT_JOB_EXP_AT_MAX, "at max");
    assert_eq!(db.next_base_exp(7, 1), 548, "groups share a table");
    assert_eq!(db.next_base_exp(7, 2), NEXT_BASE_EXP_AT_MAX, "Knight max 2");
    assert_eq!(db.next_job_exp(7, 1), NEXT_JOB_EXP_AT_MAX, "no job table");
    assert_eq!(db.next_base_exp(99, 1), NEXT_BASE_EXP_AT_MAX, "unknown job");
    assert_eq!(db.next_job_exp(99, 0), 0);
}

#[test]
fn exp_tables_reject_unknown_and_repeated_jobs() {
    let jobs = || JobDb::parse_jobs("jobs", JOBS).expect("parses");
    let points = JobDb::parse_status_points("points", POINTS).expect("parses");
    let base = || JobDb::from_parts("x", jobs(), &points).expect("builds");
    let unknown = EXP.replace("Jobs: [0, 7]", "Jobs: [0, 8]");
    let groups = JobDb::parse_exp_tables("exp", &unknown).expect("parses");
    let message = schema_error(base().with_exp_tables("exp", groups));
    assert!(message.contains("job 8"), "{message}");
    let twice = EXP.replace("Kind: Job", "Kind: Base");
    let groups = JobDb::parse_exp_tables("exp", &twice).expect("parses");
    let message = schema_error(base().with_exp_tables("exp", groups));
    assert!(message.contains("job 0"), "{message}");
}

fn shipped_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/job"))
}

fn shipped() -> JobDb {
    JobDb::load_dir(shipped_dir()).unwrap_or_else(|e| panic!("{e}"))
}

/// DB-JOB-01: every committed file loads and carries the license header and attribution.
#[test]
fn db_job_01_shipped_data_loads() {
    for file in [JOB_DATA_FILE, STATPOINT_DATA_FILE] {
        let path = shipped_dir().join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
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
    }
    let db = shipped();
    assert!(db.len() >= 148, "expected every job, found {}", db.len());
}

/// DB-JOB-02: Novice job 10 is +1 to all six; Swordman job 50 sums the catalog's 18 gains.
#[test]
fn db_job_02_job_bonus_rows() {
    let db = shipped();
    let novice = db.by_name("novice").expect("Novice");
    let one = JobBonus {
        str: 1,
        agi: 1,
        vit: 1,
        int: 1,
        dex: 1,
        luk: 1,
        ..JobBonus::default()
    };
    assert_eq!(novice.bonus(10), one);
    // Swordman gains (catalog db/job_stats.md): Str at 2, 14, 33, 40, 47, 49, 50; Vit at 6,
    // 18, 38, 42; Dex at 10, 22, 36; Luk at 26, 44; Agi at 30, 46.
    let swordman = db.get(1).expect("Swordman");
    assert_eq!(
        swordman.bonus(50),
        JobBonus {
            str: 7,
            agi: 2,
            vit: 4,
            dex: 3,
            luk: 2,
            ..JobBonus::default()
        }
    );
}

/// DB-JOB-03: HP/SP tables have `MaxBaseLevel` entries.
#[test]
fn db_job_03_tables_cover_every_base_level() {
    let db = shipped();
    for job in db.iter() {
        let max = job.max_base_level;
        assert!(job.base_hp(max) > 0, "{} HP at {max}", job.name);
        assert!(job.base_sp(max) > 0, "{} SP at {max}", job.name);
    }
    let novice = db.get(0).expect("Novice");
    assert_eq!((novice.base_hp(10), novice.base_hp(99)), (85, 530));
    let knight = db.get(7).expect("Knight");
    assert_eq!((knight.base_hp(99), knight.base_sp(99)), (7978, 307));
}

/// DB-JOB-04: ASPD bases and the status point table.
#[test]
fn db_job_04_aspd_and_status_points() {
    let db = shipped();
    let novice = db.get(0).expect("Novice");
    assert_eq!(novice.aspd_base(WeaponType::Fist), 40);
    assert_eq!(novice.aspd_base(WeaponType::Bow), 200);
    assert_eq!(novice.shield_aspd(), 10);
    assert_eq!((db.status_points(1), db.status_points(99)), (48, 1273));
    assert_eq!(db.status_points(275), 4099);
}

/// DB-JOB-05: the rows the captured status windows need (catalog mechanics/stats.md
/// "Captured status values").
#[test]
fn db_job_05_rows_behind_the_capture_vectors() {
    let db = shipped();
    let novice = db.get(0).expect("Novice");
    assert_eq!((novice.base_hp(1), novice.base_sp(1)), (40, 11));
    assert_eq!(novice.base_hp(11), 90, "MaxHP 90 at VIT 1, 91 at VIT 2");
    assert_eq!(
        novice.aspd_base(WeaponType::Dagger),
        55,
        "Knife amotion 590"
    );
    assert_eq!(novice.max_weight, 20000);
    assert_eq!(db.get(1).expect("Swordman").max_weight, 28000);
    assert_eq!(db.get(1).expect("Swordman").shield_aspd(), 5);
    let knight = db.get(7).expect("Knight");
    assert_eq!(
        (knight.max_weight, knight.aspd_base(WeaponType::Fist)),
        (28000, 40)
    );
    assert_eq!(db.get(19).expect("Bard").max_weight, 27000);
    assert_eq!(db.by_name("lord_knight").map(|j| j.id), Some(4008));
    assert_eq!(db.by_name("Dragon_Knight").map(|j| j.id), Some(4252));
}

/// DB-TRAIT-01: the cumulative trait points (catalog `db/exp_tables.md`, corrected 2026-10-29:
/// 3, 6, 9, 12, 19 for B201..205); the status points are unchanged.
#[test]
fn db_trait_01_trait_points() {
    let db = shipped();
    assert_eq!(db.trait_points(200), 0);
    assert_eq!(db.trait_points(201), 3);
    assert_eq!(db.trait_points(205), 19);
    assert_eq!(db.trait_points(210), 38);
    assert_eq!(db.trait_points(275), 285, "the source total");
    assert_eq!(db.trait_points(99), 0);
    assert_eq!((db.status_points(200), db.status_points(205)), (4099, 4099));
}

/// DB-TRAIT-02: base AP is 200 from base level 200 and 0 below it. rAthena's fallback has no
/// job guard, so a 3rd class at B200 gets 200 too (catalog `trait_stats.md` §2).
#[test]
fn db_trait_02_base_ap() {
    let db = shipped();
    let dk = db.by_name("dragon_knight").expect("Dragon Knight");
    assert_eq!(dk.base_ap(199), 0);
    assert_eq!(dk.base_ap(200), 200);
    assert_eq!(dk.base_ap(275), 200);
    assert_eq!(
        db.by_name("hyper_novice")
            .expect("Hyper Novice")
            .base_ap(201),
        200
    );
    let rune = db.by_name("rune_knight").expect("Rune Knight");
    assert_eq!((rune.base_ap(199), rune.base_ap(200)), (0, 200));
    assert_eq!(db.by_name("knight").expect("Knight").base_ap(99), 0);
    assert_eq!(db.by_name("novice").expect("Novice").base_ap(99), 0);
}

/// DB-CMB-01 (job part): the new exp file loads and carries the header.
#[test]
fn db_cmb_01_shipped_exp_files_load() {
    {
        let path = shipped_dir().join(JOB_EXP_DATA_FILE);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
        assert!(
            text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"),
            "{} lacks the SPDX header",
            path.display()
        );
        assert!(
            text.contains("rAthena")
                && text.contains("tools/job-import")
                && text.contains("crates/ro-db/data/NOTICE.md"),
            "{} lacks attribution",
            path.display()
        );
    }
    assert_eq!(shipped().next_base_exp(0, 1), 548);
}

/// DB-CMB-03: NEXT exp from the shipped tables.
#[test]
fn db_cmb_03_next_exp() {
    let db = shipped();
    assert_eq!(db.next_base_exp(0, 1), 548);
    assert_eq!(db.next_base_exp(0, 2), 894);
    assert_eq!(db.next_base_exp(0, 99), NEXT_BASE_EXP_AT_MAX);
    assert_eq!(db.next_job_exp(0, 1), 10);
    assert_eq!(db.next_job_exp(0, 9), 340);
    assert_eq!(db.next_job_exp(0, 10), NEXT_JOB_EXP_AT_MAX);
    assert_eq!(db.next_job_exp(3, 1), 60, "Archer: 1st-class table");
}

/// DB-CMB-03 (final review finding 7): no shipped job sends a NEXT of 0 below its max level,
/// which the client's exp bar would divide by. A job without a table gets the max constants.
#[test]
fn db_cmb_03_shipped_bars_are_never_zero_below_the_max() {
    let db = shipped();
    let mut empty = Vec::new();
    for job in db.iter() {
        for level in 1..job.max_base_level {
            if db.next_base_exp(job.id, level) == 0 {
                empty.push((job.id, "base", level));
            }
        }
        for level in 1..job.max_job_level {
            if db.next_job_exp(job.id, level) == 0 {
                empty.push((job.id, "job", level));
            }
        }
    }
    assert!(empty.is_empty(), "zero bars: {empty:?}");
}
