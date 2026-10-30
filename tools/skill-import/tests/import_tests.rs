//! rAthena skill and skill-tree conversion (spec `specs/skills/spec.md` §0). Hand-written
//! fixtures.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use ro_db::{SkillDb, SkillRequirement, SkillTreeDb};
use skill_import::{convert, expand, render_skills, render_trees, Converted, MaxLevels};

const SKILLS: &str = "Header:
  Type: SKILL_DB
  Version: 4
Body:
  - Id: 1
    Name: NV_BASIC
    Description: Basic Skill
    MaxLevel: 9
  - Id: 2
    Name: SM_SWORD
    Description: Sword Mastery
    MaxLevel: 10
  - Id: 3
    Name: SM_TWOHAND
    Description: Two-Handed Sword Mastery
    MaxLevel: 10
  - Id: 5
    Name: SM_BASH
    Description: Bash
    MaxLevel: 10
    TargetType: Attack
    Range: -1
    Requires:
      SpCost:
        - Level: 1
          Amount: 8
        - Level: 2
          Amount: 8
        - Level: 3
          Amount: 8
        - Level: 4
          Amount: 8
        - Level: 5
          Amount: 8
        - Level: 6
          Amount: 15
      Weapon:
        Fist: true
  - Id: 6
    Name: SM_PROVOKE
    Description: Provoke
    MaxLevel: 10
    TargetType: Attack
    Range: 9
    Requires:
      SpCost:
        - Level: 1
          Amount: 4
        - Level: 2
          Amount: 5
  - Id: 7
    Name: SM_MAGNUM
    Description: Magnum Break
    MaxLevel: 10
    TargetType: Self
    Requires:
      SpCost: 30
  - Id: 8
    Name: SM_ENDURE
    Description: Endure
    MaxLevel: 10
    TargetType: Self
    Flags:
      IsQuest: false
      AlterRangeVulture: true
  - Id: 142
    Name: NV_FIRSTAID
    Description: First Aid
    MaxLevel: 1
    TargetType: Self
    Flags:
      IsQuest: true
  - Id: 143
    Name: NV_TRICKDEAD
    Description: Play Dead
    MaxLevel: 1
    TargetType: Self
  - Id: 334
    Name: WE_CALLBABY
    Description: Call Baby
    MaxLevel: 1
    TargetType: Self
    Flags:
      IsWedding: true
  - Id: 55
    Name: KN_SPEARMASTERY
    Description: Spear Mastery
    MaxLevel: 10
  - Id: 56
    Name: KN_PIERCE
    Description: Pierce
    MaxLevel: 10
    TargetType: Attack
  - Id: 9999
    Name: UNUSED_SKILL
    Description: Unused
    MaxLevel: 1
";

const TREES: &str = "Header:
  Type: SKILL_TREE_DB
  Version: 1
Body:
  - Job: Novice
    Tree:
      - Name: NV_BASIC
        MaxLevel: 9
      - Name: NV_FIRSTAID
        MaxLevel: 1
      - Name: NV_TRICKDEAD
        MaxLevel: 1
        Exclude: true
      - Name: WE_CALLBABY
        MaxLevel: 1
  - Job: Swordman
    Inherit:
      Novice: true
    Tree:
      - Name: SM_TWOHAND
        MaxLevel: 10
        Requires:
          - Name: SM_SWORD
            Level: 1
      - Name: SM_SWORD
        MaxLevel: 10
      - Name: SM_BASH
        MaxLevel: 10
      - Name: SM_MAGNUM
        MaxLevel: 10
        Requires:
          - Name: SM_BASH
            Level: 5
  - Job: Knight
    Inherit:
      Novice: true
      Swordman: true
    Tree:
      - Name: KN_SPEARMASTERY
        MaxLevel: 10
      - Name: KN_PIERCE
        MaxLevel: 10
        JobLevel: 80
        Requires:
          - Name: KN_SPEARMASTERY
            Level: 1
      - Name: SM_BASH
        MaxLevel: 5
  - Job: Knight2
    Inherit:
      Knight: true
  - Job: Archer
    Inherit:
      Swordman: true
      Knight: true
    Tree:
      - Name: SM_PROVOKE
        MaxLevel: 3
  - Job: Supernovice
    Inherit:
      Novice: true
  - Job: Summoner
    Inherit:
      Novice: true
    Tree:
      - Name: WE_CALLBABY
        MaxLevel: 0
      - Name: SM_ENDURE
        MaxLevel: 20
        BaseLevel: 300
  - Job: Swordman
    Tree:
      - Name: SM_MAGNUM
        Requires:
          - Name: SM_BASH
            Level: 0
          - Name: SM_SWORD
            Level: 2
";

fn max_levels() -> MaxLevels {
    HashMap::from([
        (0, (99, 10)),
        (1, (99, 50)),
        (7, (99, 50)),
        (4218, (200, 60)),
    ])
}

fn converted() -> Converted {
    convert(SKILLS, TREES, &max_levels()).expect("fixtures convert")
}

fn tree(c: &Converted, job: u16) -> Vec<(u16, u8)> {
    c.trees
        .iter()
        .find(|t| t.job == job)
        .unwrap_or_else(|| panic!("job {job} has a tree"))
        .tree
        .iter()
        .map(|e| (e.skill, e.max_level))
        .collect()
}

fn entry(c: &Converted, job: u16, skill: u16) -> &ro_db::TreeEntry {
    c.trees
        .iter()
        .find(|t| t.job == job)
        .and_then(|t| t.tree.iter().find(|e| e.skill == skill))
        .unwrap_or_else(|| panic!("job {job} lists skill {skill}"))
}

#[test]
fn expansion_linear_period_one() {
    // SM_PROVOKE-like: 4, 5 continue by +1.
    let v = expand(&[(1, 4), (2, 5)]).expect("valid");
    assert_eq!(v, [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
}

#[test]
fn expansion_linear_longer_period() {
    // SM_BASH: 8 x5, 15 x5 has period 5 with +7.
    let v = expand(&[
        (1, 8),
        (2, 8),
        (3, 8),
        (4, 8),
        (5, 8),
        (6, 15),
        (7, 15),
        (8, 15),
        (9, 15),
        (10, 15),
    ])
    .expect("valid");
    assert_eq!(v, [8, 8, 8, 8, 8, 15, 15, 15, 15, 15, 22, 22, 22]);
}

#[test]
fn expansion_repeats_when_not_linear() {
    // A single entry: no period is tried, the value repeats.
    assert_eq!(expand(&[(1, 7)]).expect("valid"), [7; 13]);
    // 1, 2, 4 has no period: 4 repeats.
    let v = expand(&[(1, 1), (2, 2), (3, 4)]).expect("valid");
    assert_eq!(v, [1, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4]);
    // A gap reads as 0, so 10, 0, 30 is not linear; unlisted lower slots stay 0.
    let v = expand(&[(1, 10), (3, 30)]).expect("valid");
    assert_eq!(v, [10, 0, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30]);
    let v = expand(&[(3, 30)]).expect("valid");
    assert_eq!(v, [0, 0, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30]);
}

#[test]
fn expansion_clamps_to_one() {
    // Falling by 1 from 10: reaches 1, then stays 1.
    let v = expand(&[(1, 10), (2, 9)]).expect("valid");
    assert_eq!(v, [10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 1, 1, 1]);
    // Falling by 20 from 100: 40, 20, then 0 is clamped to 1.
    let v = expand(&[(1, 100), (2, 80), (3, 60)]).expect("valid");
    assert_eq!(v, [100, 80, 60, 40, 20, 1, 1, 1, 1, 1, 1, 1, 1]);
    // A flat zero list still turns into 1 above the listed levels.
    let v = expand(&[(1, 0), (2, 0)]).expect("valid");
    assert_eq!(v, [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    // Already negative: no clamp.
    let v = expand(&[(1, -1), (2, -2)]).expect("valid");
    assert_eq!(v[12], -13);
}

#[test]
fn expansion_refuses_bad_levels() {
    assert!(expand(&[(14, 1)]).is_err());
    assert!(expand(&[(0, 1)]).is_err());
}

#[test]
fn skills_are_trimmed_and_expanded() {
    let c = converted();
    let ids: Vec<u16> = c.skills.iter().map(|s| s.id).collect();
    // Only skills a tree lists or requires; UNUSED_SKILL is dropped.
    assert_eq!(ids, [1, 2, 3, 5, 6, 7, 8, 55, 56, 142, 143, 334]);
    let db = SkillDb::from_rows("test", c.skills.clone()).expect("valid registry");
    let bash = db.by_name("sm_bash").expect("bash");
    assert_eq!(bash.inf, 1);
    assert_eq!(bash.sp_cost, [8, 8, 8, 8, 8, 15, 15, 15, 15, 15]);
    assert_eq!(bash.range, [-1; 10]);
    let provoke = db.get(6).expect("provoke");
    assert_eq!((provoke.sp(1), provoke.sp(5), provoke.sp(10)), (4, 8, 13));
    assert_eq!(provoke.range(3), 9);
    let magnum = db.get(7).expect("magnum");
    assert_eq!((magnum.inf, magnum.sp(1), magnum.sp(10)), (4, 30, 30));
    let basic = db.get(1).expect("basic");
    assert!(basic.is_passive() && basic.sp_cost.is_empty() && basic.range.is_empty());
    assert!(db.get(142).expect("first aid").flags.quest);
    assert!(db.get(334).expect("call baby").flags.wedding);
    let endure = db.get(8).expect("endure");
    assert!(!endure.flags.quest && endure.flags.alter_range_vulture);
}

#[test]
fn inheritance_is_resolved() {
    let c = converted();
    // Novice keeps its own Exclude'd entry.
    assert_eq!(tree(&c, 0), [(1, 9), (142, 1), (143, 1), (334, 1)]);
    // Swordman: Novice's entries but not the Exclude'd one; sorted by id; the second Swordman
    // entry merged into the first (rule 5).
    assert_eq!(
        tree(&c, 1),
        [
            (1, 9),
            (2, 10),
            (3, 10),
            (5, 10),
            (7, 10),
            (142, 1),
            (334, 1)
        ]
    );
    // Rule 5: SM_MAGNUM keeps MaxLevel, Level 0 removed SM_BASH, SM_SWORD 2 was added.
    assert_eq!(
        entry(&c, 1, 7).requires,
        [SkillRequirement { skill: 2, level: 2 }]
    );
    // Knight: the child's own SM_BASH (max 5) wins over Swordman's; JobLevel capped to 50.
    assert_eq!(
        tree(&c, 7),
        [
            (1, 9),
            (2, 10),
            (3, 10),
            (5, 5),
            (7, 10),
            (55, 10),
            (56, 10),
            (142, 1),
            (334, 1)
        ]
    );
    assert_eq!(entry(&c, 7, 56).job_level, 50);
    assert_eq!(
        entry(&c, 7, 56).requires,
        [SkillRequirement {
            skill: 55,
            level: 1
        }]
    );
}

#[test]
fn inheritance_is_flat_and_later_parent_wins() {
    let c = converted();
    // Archer lists Swordman then Knight: Knight's own SM_BASH 5 wins over Swordman's 10, and
    // inheritance is not transitive (no Novice skills: neither parent's own tree has them).
    assert_eq!(
        tree(&c, 3),
        [
            (2, 10),
            (3, 10),
            (5, 5),
            (6, 3),
            (7, 10),
            (55, 10),
            (56, 10)
        ]
    );
}

#[test]
fn max_level_zero_removes_and_levels_are_capped() {
    let c = converted();
    // Summoner: WE_CALLBABY removed although Novice passes it down; SM_ENDURE MaxLevel capped
    // to the skill's 10, BaseLevel to the job's 200.
    assert_eq!(tree(&c, 4218), [(1, 9), (8, 10), (142, 1)]);
    assert_eq!(entry(&c, 4218, 8).base_level, 200);
}

#[test]
fn jobs_without_an_id_are_skipped() {
    let c = converted();
    assert!(c.report.skipped_jobs.contains("Knight2"));
    // `Supernovice` is the tree file's name for Super Novice (23).
    assert_eq!(tree(&c, 23), [(1, 9), (142, 1), (334, 1)]);
    assert_eq!(c.trees.len(), 6);
    assert!(c.report.warnings.is_empty(), "{:?}", c.report.warnings);
}

#[test]
fn rendered_files_load_back() {
    let c = converted();
    let skills_text = render_skills(&c.skills, "abc123").expect("render skills");
    let trees_text = render_trees(&c.trees, "abc123").expect("render trees");
    for text in [&skills_text, &trees_text] {
        assert!(text.starts_with("# SPDX-License-Identifier: GPL-3.0-only\n"));
        assert!(text.contains("abc123") && text.contains("crates/ro-db/data/NOTICE.md"));
    }
    let skills = SkillDb::from_rows(
        "skills",
        SkillDb::parse("skills", &skills_text).expect("parse skills"),
    )
    .expect("skills load");
    assert_eq!(skills.iter().cloned().collect::<Vec<_>>(), c.skills);
    let trees = SkillTreeDb::from_rows(
        "trees",
        SkillTreeDb::parse("trees", &trees_text).expect("parse trees"),
        &skills,
    )
    .expect("trees load");
    assert_eq!(trees.tree(7).len(), 9);
    assert_eq!(trees.max_level(7, 5, &skills), 5);
    assert_eq!(
        trees.max_level(0, 5, &skills),
        10,
        "falls back to the skill's"
    );
}
