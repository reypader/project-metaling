//! rAthena skill and skill-tree conversion (spec `specs/skills/spec.md` §0). Hand-written
//! fixtures.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use ro_db::{SkillDb, SkillElement, SkillRequirement, SkillTreeDb, WeaponType};
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
    Element: Weapon
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
        Dagger: true
        Bow: false
        Double_DD: true
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
      Weapon:
        All: true
      ItemCost:
        - Item: Test_Gem
          Amount: 2
        - Item: Test_Dust
          Amount: 1
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
    DamageFlags:
      NoDamage: true
    Requires:
      State: None
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
    HitCount: -3
    Element:
      - Level: 1
        Element: Fire
      - Level: 3
        Element: Holy
    DamageFlags:
      Critical: true
      IgnoreFlee: true
      Shiny: true
    CastTimeFlags:
      IgnoreDex: true
    CastTime:
      - Level: 1
        Time: 1000
      - Level: 2
        Time: 1200
    FixedCastTime: -1
    AfterCastActDelay: 700
    AfterCastWalkDelay: 300
    Cooldown:
      - Level: 1
        Time: 0
      - Level: 2
        Time: 5000
    Requires:
      HpCost: 15
      ZenyCost:
        - Level: 1
          Amount: 10
        - Level: 2
          Amount: 20
      ItemCost:
        - Item: Test_Gem
          Amount: 1
          Level: 2
        - Item: Test_Dust
          Amount: 3
          Level: 4
        - Item: Test_Bad
          Amount: 1
          Level: 11
      State: Shield
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
    // The only warnings are the casting fixture's deliberate bad entries.
    let tree_warnings: Vec<_> = c
        .report
        .warnings
        .iter()
        .filter(|w| w.starts_with("job "))
        .collect();
    assert!(tree_warnings.is_empty(), "{tree_warnings:?}");
    assert_eq!(c.report.warnings.len(), 3, "{:?}", c.report.warnings);
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

/// Spec `specs/effects/spec.md` §0.1: the casting fields, from our own fixture rows.
#[test]
fn casting_fields_are_imported() {
    let c = converted();
    let db = SkillDb::from_rows("test", c.skills.clone()).expect("valid registry");

    let pierce = db.get(56).expect("pierce");
    assert_eq!(pierce.hit_count, [-3; 10]);
    // Listed levels 1 and 3; the gap stays Neutral, later levels repeat the last one.
    assert_eq!(pierce.element(1), SkillElement::Fire);
    assert_eq!(pierce.element(2), SkillElement::Neutral);
    assert_eq!(pierce.element(10), SkillElement::Holy);
    assert!(pierce.damage_flags.critical && pierce.damage_flags.ignore_flee);
    assert!(!pierce.damage_flags.no_damage);
    assert!(pierce.cast_time_flags.ignore_dex);
    // Per-level lists use the series expansion like SpCost.
    assert_eq!(
        pierce.cast_time,
        [1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600, 2800]
    );
    assert_eq!(pierce.fixed_cast_time(4), -1);
    assert_eq!(pierce.after_cast_act_delay(10), 700);
    assert_eq!(pierce.after_cast_walk_delay(1), 300);
    assert_eq!((pierce.cooldown(1), pierce.cooldown(3)), (0, 10000));
    assert_eq!(pierce.hp_cost(7), 15);
    assert_eq!((pierce.zeny_cost(1), pierce.zeny_cost(10)), (10, 100));
    assert_eq!(pierce.state.as_deref(), Some("Shield"));
    // Level-dependent item costs: only that level's entry; the bad level was dropped.
    let at = |lv| {
        pierce
            .item_cost(lv)
            .map(|c| c.item.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (at(1), at(2), at(4)),
        (vec![], vec!["Test_Gem"], vec!["Test_Dust"])
    );
    assert_eq!(pierce.item_cost.len(), 2);

    let bash = db.get(5).expect("bash");
    assert_eq!(bash.element(5), SkillElement::Weapon);
    assert_eq!(bash.element(5).code(), 12);
    assert_eq!(bash.weapons, [WeaponType::Fist, WeaponType::Dagger]);
    assert!(bash.allows_weapon(WeaponType::Dagger) && !bash.allows_weapon(WeaponType::Bow));

    let magnum = db.get(7).expect("magnum");
    assert!(magnum.weapons.is_empty() && magnum.allows_weapon(WeaponType::Bow));
    assert_eq!(magnum.item_cost(3).count(), 2);

    let first_aid = db.get(142).expect("first aid");
    assert!(first_aid.damage_flags.no_damage);
    assert_eq!(first_aid.state, None);

    let provoke = db.get(6).expect("provoke");
    assert_eq!(provoke.element(1), SkillElement::Neutral);
    assert_eq!((provoke.hit_count(1), provoke.cast_time(1)), (0, 0));
    assert!(provoke.cast_time.is_empty() && provoke.element.is_empty());

    let warned = |needle: &str| c.report.warnings.iter().any(|w| w.contains(needle));
    assert!(warned("Double_DD") && warned("Shiny") && warned("Test_Bad"));
}
