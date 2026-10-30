//! Skill registry and skill trees (spec `specs/skills/spec.md` §1, DB-SKILL-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use ro_db::{
    DbLoadError, SkillDb, SkillRequirement, SkillTreeDb, SKILL_DATA_FILE, SKILL_TREE_DATA_FILE,
};

const NOVICE: u16 = 0;
const SWORDMAN: u16 = 1;
const KNIGHT: u16 = 7;
const NOVICE_HIGH: u16 = 4001;
const SWORDMAN_HIGH: u16 = 4002;
const RUNE_KNIGHT: u16 = 4054;
const SUMMONER: u16 = 4218;
const BABY_SUMMONER: u16 = 4220;

const NV_BASIC: u16 = 1;
const SM_SWORD: u16 = 2;
const SM_TWOHAND: u16 = 3;
const SM_BASH: u16 = 5;
const SM_PROVOKE: u16 = 6;
const SM_MAGNUM: u16 = 7;
const SM_ENDURE: u16 = 8;
const KN_SPEARMASTERY: u16 = 55;
const KN_PIERCE: u16 = 56;
const NV_FIRSTAID: u16 = 142;
const NV_TRICKDEAD: u16 = 143;
const WE_CALLBABY: u16 = 410;

fn shipped_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/skill"))
}

fn shipped() -> (SkillDb, SkillTreeDb) {
    let skills = SkillDb::load_dir(shipped_dir()).unwrap_or_else(|e| panic!("{e}"));
    let trees = SkillTreeDb::load_dir(shipped_dir(), &skills).unwrap_or_else(|e| panic!("{e}"));
    (skills, trees)
}

fn ids(trees: &SkillTreeDb, job: u16) -> Vec<u16> {
    trees.tree(job).iter().map(|e| e.skill).collect()
}

/// DB-SKILL-01: every committed file loads and carries the license header and attribution.
#[test]
fn db_skill_01_shipped_data_loads() {
    for file in [SKILL_DATA_FILE, SKILL_TREE_DATA_FILE] {
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
    let (skills, trees) = shipped();
    assert!(skills.len() > 1000, "found {} skills", skills.len());
    assert!(trees.len() >= 148, "found {} trees", trees.len());
}

/// DB-SKILL-02: the registry rows behind the captured list values.
#[test]
fn db_skill_02_skill_rows() {
    let (skills, _) = shipped();
    let basic = skills.get(NV_BASIC).expect("NV_BASIC");
    assert_eq!((basic.name.as_str(), basic.max_level), ("NV_BASIC", 9));
    assert!(basic.is_passive());
    assert_eq!((basic.sp(0), basic.range(0)), (0, 0));

    let bash = skills.get(SM_BASH).expect("SM_BASH");
    assert_eq!(bash.inf, 1);
    for level in 1..=5 {
        assert_eq!(bash.sp(level), 8, "Bash lv {level}");
    }
    assert_eq!(bash.sp(10), 15);
    assert_eq!(bash.range(1).abs(), 1);

    let provoke = skills.get(SM_PROVOKE).expect("SM_PROVOKE");
    assert_eq!(
        (1..=5).map(|l| provoke.sp(l)).collect::<Vec<_>>(),
        [4, 5, 6, 7, 8]
    );
    assert_eq!(provoke.sp(10), 13);
    assert_eq!(provoke.range(1), 9);

    let magnum = skills.get(SM_MAGNUM).expect("SM_MAGNUM");
    assert_eq!((magnum.inf, magnum.sp(1)), (4, 30));
    let endure = skills.get(SM_ENDURE).expect("SM_ENDURE");
    assert_eq!((endure.inf, endure.sp(1), endure.range(1)), (4, 10, 0));

    assert_eq!(skills.by_name("sm_bash").map(|s| s.id), Some(SM_BASH));
    assert_eq!(skills.by_name("Sm_Bash").map(|s| s.id), Some(SM_BASH));
    assert!(skills.get(NV_FIRSTAID).expect("NV_FIRSTAID").flags.quest);
    assert!(skills.get(WE_CALLBABY).expect("WE_CALLBABY").flags.wedding);
    assert!(
        skills
            .by_name("AC_DOUBLE")
            .expect("AC_DOUBLE")
            .flags
            .alter_range_vulture
    );
}

/// DB-SKILL-03: trees with inheritance resolved.
#[test]
fn db_skill_03_trees() {
    let (skills, trees) = shipped();
    let swordman = ids(&trees, SWORDMAN);
    for id in 1..=8 {
        assert!(swordman.contains(&id), "Swordman lacks {id}: {swordman:?}");
    }
    assert!(
        swordman.windows(2).all(|w| w[0] < w[1]),
        "sorted by skill id"
    );
    let requires = |job, skill| {
        trees
            .entry(job, skill)
            .unwrap_or_else(|| panic!("job {job} lists {skill}"))
            .requires
            .clone()
    };
    let need = |skill, level| SkillRequirement { skill, level };
    assert_eq!(requires(SWORDMAN, SM_TWOHAND), [need(SM_SWORD, 1)]);
    assert_eq!(requires(SWORDMAN, SM_MAGNUM), [need(SM_BASH, 5)]);
    assert_eq!(requires(SWORDMAN, SM_ENDURE), [need(SM_PROVOKE, 5)]);

    let knight = ids(&trees, KNIGHT);
    for id in swordman.iter().chain(&(55..=63).collect::<Vec<_>>()) {
        assert!(knight.contains(id), "Knight lacks {id}");
    }
    assert_eq!(requires(KNIGHT, KN_PIERCE), [need(KN_SPEARMASTERY, 1)]);

    let rune_knight = ids(&trees, RUNE_KNIGHT);
    for id in &knight {
        assert!(rune_knight.contains(id), "Rune Knight lacks {id}");
    }
    assert!(ids(&trees, NOVICE).contains(&NV_TRICKDEAD));
    // Novice and Novice_High each list their own NV_TRICKDEAD with `Exclude`, so no job that
    // inherits from them gets it.
    assert!(ids(&trees, NOVICE_HIGH).contains(&NV_TRICKDEAD));
    assert!(
        !ids(&trees, SWORDMAN_HIGH).contains(&NV_TRICKDEAD),
        "Exclude"
    );
    assert!(!swordman.contains(&NV_TRICKDEAD), "Exclude");
    assert!(ids(&trees, NOVICE).contains(&WE_CALLBABY));
    // Baby_Summoner inherits Summoner's WE_CALLBABY and removes it with `MaxLevel: 0`.
    assert!(ids(&trees, SUMMONER).contains(&WE_CALLBABY));
    assert!(
        !ids(&trees, BABY_SUMMONER).contains(&WE_CALLBABY),
        "MaxLevel 0"
    );

    assert_eq!(trees.max_level(SWORDMAN, SM_BASH, &skills), 10);
    assert_eq!(trees.max_level(NOVICE, NV_BASIC, &skills), 9);
    assert_eq!(
        trees.max_level(NOVICE, KN_PIERCE, &skills),
        10,
        "not in the tree: the skill's max"
    );
    assert_eq!(trees.max_level(NOVICE, 60_000, &skills), 0);
    assert!(trees.tree(60_000).is_empty());
}

#[test]
fn trees_are_checked_against_the_registry() {
    let skills = SkillDb::from_rows(
        "t",
        SkillDb::parse(
            "t",
            "Header: {Type: SKILL_DATA, Version: 1}\nBody:\n- {Id: 1, Name: A, MaxLevel: 5}\n",
        )
        .expect("parse"),
    )
    .expect("skills");
    let tree = |body: &str| {
        let text = format!("Header: {{Type: SKILL_TREE_DATA, Version: 1}}\nBody:\n{body}");
        SkillTreeDb::from_rows("t", SkillTreeDb::parse("t", &text).expect("parse"), &skills)
    };
    assert!(tree("- {Job: 0, Name: N, Tree: [{Skill: 1, MaxLevel: 5}]}\n").is_ok());
    for bad in [
        "- {Job: 0, Name: N, Tree: [{Skill: 2, MaxLevel: 1}]}\n",
        "- {Job: 0, Name: N, Tree: [{Skill: 1, MaxLevel: 6}]}\n",
        "- {Job: 0, Name: N, Tree: [{Skill: 1, MaxLevel: 1, Requires: [{Skill: 1, Level: 9}]}]}\n",
        "- {Job: 0, Name: N}\n- {Job: 0, Name: N}\n",
    ] {
        assert!(tree(bad).is_err(), "{bad}");
    }
    let bad_levels = SkillDb::parse(
        "t",
        "Header: {Type: SKILL_DATA, Version: 1}\nBody:\n- {Id: 1, Name: A, MaxLevel: 2, SpCost: [1]}\n",
    )
    .expect("parse");
    assert!(matches!(
        SkillDb::from_rows("t", bad_levels),
        Err(DbLoadError::Schema { .. })
    ));
}
