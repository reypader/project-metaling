use std::path::Path;

use ro_core::{LearnedSkill, Sex, SkillFlag};
use ro_db::{JobDb, SkillDb, SkillTreeDb};
use ro_mechanics::BattleConfig;

use super::tree::tree_job;
use super::*;

const NOVICE: u16 = 0;
const SWORDMAN: u16 = 1;
const ARCHER: u16 = 3;
const KNIGHT: u16 = 7;

const NV_BASIC: u16 = 1;
const SM_SWORD: u16 = 2;
const SM_TWOHAND: u16 = 3;
const SM_RECOVERY: u16 = 4;
const SM_BASH: u16 = 5;
const SM_MAGNUM: u16 = 7;
const AC_VULTURE: u16 = 43;
const AC_DOUBLE: u16 = 46;
const KN_SPEARMASTERY: u16 = 55;
const NV_FIRSTAID: u16 = 142;
const WE_CALLBABY: u16 = 410;
const GATED_BASE: u16 = 900;
const GATED_JOB: u16 = 901;

/// Hand-written skills (values from the capture where there is one).
const SKILLS: &str = "Header: {Type: SKILL_DATA, Version: 2}
Body:
- {Id: 1, Name: NV_BASIC, MaxLevel: 9}
- {Id: 2, Name: SM_SWORD, MaxLevel: 10}
- {Id: 3, Name: SM_TWOHAND, MaxLevel: 10}
- {Id: 4, Name: SM_RECOVERY, MaxLevel: 10}
- Id: 5
  Name: SM_BASH
  MaxLevel: 10
  TargetType: 1
  SpCost: [8, 8, 8, 8, 8, 15, 15, 15, 15, 15]
  Range: [-1, -1, -1, -1, -1, -1, -1, -1, -1, -1]
- {Id: 7, Name: SM_MAGNUM, MaxLevel: 10, TargetType: 4, SpCost: [30, 30, 30, 30, 30, 30, 30, 30, 30, 30]}
- {Id: 43, Name: AC_VULTURE, MaxLevel: 10}
- Id: 46
  Name: AC_DOUBLE
  MaxLevel: 10
  TargetType: 1
  SpCost: [12, 12, 12, 12, 12, 12, 12, 12, 12, 12]
  Range: [-9, -9, -9, -9, -9, -9, -9, -9, -9, -9]
  Flags: {AlterRangeVulture: true}
- {Id: 55, Name: KN_SPEARMASTERY, MaxLevel: 10}
- {Id: 142, Name: NV_FIRSTAID, MaxLevel: 1, TargetType: 4, Flags: {Quest: true}}
- {Id: 410, Name: WE_CALLBABY, MaxLevel: 1, TargetType: 4, Flags: {Wedding: true}}
- {Id: 900, Name: GATED_BASE, MaxLevel: 1}
- {Id: 901, Name: GATED_JOB, MaxLevel: 1}
";

/// Hand-written trees. SM_RECOVERY needs SM_MAGNUM, which comes after it in id order, so a
/// chain needs a second pass.
const TREES: &str = "Header: {Type: SKILL_TREE_DATA, Version: 1}
Body:
- Job: 0
  Name: Novice
  Tree:
  - {Skill: 1, MaxLevel: 9}
  - {Skill: 142, MaxLevel: 1}
  - {Skill: 410, MaxLevel: 1}
- Job: 1
  Name: Swordman
  Tree:
  - {Skill: 1, MaxLevel: 9}
  - {Skill: 2, MaxLevel: 10}
  - {Skill: 3, MaxLevel: 10, Requires: [{Skill: 2, Level: 1}]}
  - {Skill: 4, MaxLevel: 10, Requires: [{Skill: 7, Level: 1}]}
  - {Skill: 5, MaxLevel: 10}
  - {Skill: 7, MaxLevel: 10, Requires: [{Skill: 5, Level: 5}]}
  - {Skill: 142, MaxLevel: 1}
  - {Skill: 410, MaxLevel: 1}
  - {Skill: 900, MaxLevel: 1, BaseLevel: 100}
  - {Skill: 901, MaxLevel: 1, JobLevel: 40}
- Job: 3
  Name: Archer
  Tree:
  - {Skill: 1, MaxLevel: 9}
  - {Skill: 43, MaxLevel: 10}
  - {Skill: 46, MaxLevel: 10}
- Job: 7
  Name: Knight
  Tree:
  - {Skill: 1, MaxLevel: 9}
  - {Skill: 2, MaxLevel: 10}
  - {Skill: 3, MaxLevel: 10, Requires: [{Skill: 2, Level: 1}]}
  - {Skill: 5, MaxLevel: 5}
  - {Skill: 7, MaxLevel: 10, Requires: [{Skill: 5, Level: 5}]}
  - {Skill: 55, MaxLevel: 10}
";

struct Data {
    skills: SkillDb,
    trees: SkillTreeDb,
    jobs: JobDb,
    config: BattleConfig,
}

impl Data {
    fn new() -> Self {
        let skills =
            SkillDb::from_rows("t", SkillDb::parse("t", SKILLS).expect("parse")).expect("skills");
        let trees =
            SkillTreeDb::from_rows("t", SkillTreeDb::parse("t", TREES).expect("parse"), &skills)
                .expect("trees");
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        Self {
            skills,
            trees,
            jobs: JobDb::load_dir(&dir).expect("shipped job data"),
            config: BattleConfig::default(),
        }
    }

    fn tree(&self) -> TreeData<'_> {
        TreeData {
            skills: &self.skills,
            trees: &self.trees,
            jobs: &self.jobs,
            config: &self.config,
        }
    }

    /// The visible ids of `class` with `learned` (id, level) and the class-change levels.
    fn visible(
        &self,
        class: u16,
        learned: &[(u16, u8)],
        levels: (u16, u16),
        change_level: &mut [u16; 3],
    ) -> (SkillSet, Vec<u16>) {
        let mut set = set(learned);
        compute_visible(&mut set, change_level, owner(class, levels), self.tree());
        let ids = visible_ids(&set);
        (set, ids)
    }
}

fn set(learned: &[(u16, u8)]) -> SkillSet {
    let rows: Vec<LearnedSkill> = learned
        .iter()
        .map(|&(id, level)| LearnedSkill {
            id,
            level,
            flag: SkillFlag::Permanent,
        })
        .collect();
    SkillSet::from_spawn(&rows, &[])
}

fn owner(class: u16, (base_level, job_level): (u16, u16)) -> TreeOwner {
    TreeOwner {
        class,
        sex: Sex::Male,
        base_level,
        job_level,
    }
}

fn visible_ids(set: &SkillSet) -> Vec<u16> {
    set.iter().filter(|s| s.visible).map(|s| s.id).collect()
}

/// ENG-SKILL-01: prerequisites (a chain resolved in one call), base/job level gates, the
/// quest and wedding filters, `player_skillfree`.
#[test]
fn eng_skill_01_visibility_rules() {
    let mut data = Data::new();
    data.config.player_skillup_limit = false;
    let mut cl = [0; 3];
    // Nothing learned: only the skills without prerequisites; quest/wedding stay hidden at 0.
    let (_, ids) = data.visible(SWORDMAN, &[], (1, 1), &mut cl);
    assert_eq!(ids, [NV_BASIC, SM_SWORD, SM_BASH]);
    // Sword 1 opens Two-Handed; Bash 5 opens Magnum, which opens Recovery (a later id
    // needed by an earlier one): one call resolves the chain.
    let learned = [(SM_SWORD, 1), (SM_BASH, 5), (SM_MAGNUM, 1)];
    let (_, ids) = data.visible(SWORDMAN, &learned, (1, 1), &mut cl);
    assert_eq!(
        ids,
        [
            NV_BASIC,
            SM_SWORD,
            SM_TWOHAND,
            SM_RECOVERY,
            SM_BASH,
            SM_MAGNUM
        ]
    );
    // Bash 4 is not enough, and a hidden Magnum keeps Recovery hidden too.
    let (set, ids) = data.visible(SWORDMAN, &[(SM_BASH, 4), (SM_MAGNUM, 1)], (1, 1), &mut cl);
    assert_eq!(ids, [NV_BASIC, SM_SWORD, SM_BASH]);
    assert_eq!(set.level(SM_MAGNUM), 1, "a hidden slot keeps its level");
    // Base and job level gates.
    let (_, ids) = data.visible(SWORDMAN, &[], (100, 39), &mut cl);
    assert!(
        ids.contains(&GATED_BASE) && !ids.contains(&GATED_JOB),
        "{ids:?}"
    );
    let (_, ids) = data.visible(SWORDMAN, &[], (99, 40), &mut cl);
    assert!(
        !ids.contains(&GATED_BASE) && ids.contains(&GATED_JOB),
        "{ids:?}"
    );
    // A learned quest skill stays visible; quest_skill_learn shows it at 0.
    let (_, ids) = data.visible(SWORDMAN, &[(NV_FIRSTAID, 1)], (1, 1), &mut cl);
    assert!(ids.contains(&NV_FIRSTAID));
    data.config.quest_skill_learn = true;
    let (_, ids) = data.visible(SWORDMAN, &[], (1, 1), &mut cl);
    assert!(ids.contains(&NV_FIRSTAID) && !ids.contains(&WE_CALLBABY));
    // skillfree: every entry but the level-0 filtered ones, with no checks.
    data.config.quest_skill_learn = false;
    data.config.player_skillfree = true;
    let (_, ids) = data.visible(SWORDMAN, &[], (1, 1), &mut cl);
    assert_eq!(
        ids,
        [
            NV_BASIC,
            SM_SWORD,
            SM_TWOHAND,
            SM_RECOVERY,
            SM_BASH,
            SM_MAGNUM,
            GATED_BASE,
            GATED_JOB
        ]
    );
}

#[test]
fn granted_slots_stay_visible_and_hidden_zero_slots_are_dropped() {
    let data = Data::new();
    let mut cl = [0; 3];
    let mut set = SkillSet::from_spawn(
        &[LearnedSkill {
            id: KN_SPEARMASTERY,
            level: 1,
            flag: SkillFlag::Granted,
        }],
        &[],
    );
    compute_visible(&mut set, &mut cl, owner(NOVICE, (1, 1)), data.tree());
    assert!(set.get(KN_SPEARMASTERY).is_some_and(|s| s.visible));
    // A granted prerequisite counts (rAthena refuses only temporary and copied ones).
    let mut set = SkillSet::from_spawn(
        &[
            LearnedSkill {
                id: NV_BASIC,
                level: 9,
                flag: SkillFlag::Permanent,
            },
            LearnedSkill {
                id: SM_SWORD,
                level: 1,
                flag: SkillFlag::Granted,
            },
        ],
        &[],
    );
    compute_visible(&mut set, &mut cl, owner(SWORDMAN, (1, 1)), data.tree());
    assert!(set.get(SM_TWOHAND).is_some_and(|s| s.visible));
    // A Swordman whose Two-Handed (level 0) loses Sword Mastery: the slot is dropped.
    let mut set = set_visible(&data, SWORDMAN, &[(NV_BASIC, 9), (SM_SWORD, 1)]);
    assert!(set.get(SM_TWOHAND).is_some());
    set.get_mut(SM_SWORD).expect("sword").level = 0;
    let change = compute_visible(&mut set, &mut cl, owner(SWORDMAN, (1, 1)), data.tree());
    assert!(set.get(SM_TWOHAND).is_none());
    assert!(change.changed);
    assert_eq!(change.vanished.as_slice(), [SM_TWOHAND]);
    // Nothing changes on a second call.
    let change = compute_visible(&mut set, &mut cl, owner(SWORDMAN, (1, 1)), data.tree());
    assert_eq!(change, TreeChange::default());
}

fn set_visible(data: &Data, class: u16, learned: &[(u16, u8)]) -> SkillSet {
    let mut set = set(learned);
    compute_visible(&mut set, &mut [0; 3], owner(class, (1, 1)), data.tree());
    set
}

/// ENG-SKILL-02: `player_skillup_limit`.
#[test]
fn eng_skill_02_skillup_limit() {
    let data = Data::new();
    let mut cl = [0; 3];
    // A Swordman with NV_BASIC below 9 sees only the Novice tree.
    let (_, ids) = data.visible(SWORDMAN, &[(NV_BASIC, 8)], (1, 1), &mut cl);
    assert_eq!(ids, [NV_BASIC]);
    // At 9 (the Novice's max job level 10 - 1) the Swordman tree appears.
    let (_, ids) = data.visible(SWORDMAN, &[(NV_BASIC, 9)], (1, 1), &mut cl);
    assert_eq!(ids, [NV_BASIC, SM_SWORD, SM_BASH]);
    // A Knight whose 1st-to-2nd change was at job level 10 needs 9 more points.
    let mut cl = [10, 0, 0];
    let swordman_17 = [(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 7)];
    let (_, ids) = data.visible(KNIGHT, &swordman_17, (1, 1), &mut cl);
    assert!(!ids.contains(&KN_SPEARMASTERY), "17 points: {ids:?}");
    assert!(ids.contains(&SM_TWOHAND), "the Swordman tree");
    let swordman_18 = [(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 8)];
    let (_, ids) = data.visible(KNIGHT, &swordman_18, (1, 1), &mut cl);
    assert!(ids.contains(&KN_SPEARMASTERY), "18 points: {ids:?}");
    // Hidden levels count for nothing: Spear Mastery from the Knight tree doesn't help.
    let spear = LearnedSkill {
        id: KN_SPEARMASTERY,
        level: 5,
        flag: SkillFlag::Permanent,
    };
    let mut set = SkillSet::from_spawn(set(&swordman_17).saved_rows().as_slice(), &[spear]);
    compute_visible(&mut set, &mut cl, owner(KNIGHT, (1, 1)), data.tree());
    assert!(!visible_ids(&set).contains(&KN_SPEARMASTERY));
    assert_eq!(set.level(KN_SPEARMASTERY), 5);
    // Unset: defaults to the Swordman's max job level (50) and is stored.
    let mut cl = [0; 3];
    let (_, ids) = data.visible(KNIGHT, &swordman_18, (1, 1), &mut cl);
    assert_eq!(cl, [50, 0, 0]);
    assert!(!ids.contains(&KN_SPEARMASTERY));
    // A 1st class doesn't set it.
    let mut cl = [0; 3];
    data.visible(SWORDMAN, &[(NV_BASIC, 9)], (1, 1), &mut cl);
    assert_eq!(cl, [0; 3]);
}

/// The normalised job for the catalog's worked examples (`db/skills.md` "Runtime" step 1).
#[test]
fn tree_job_normalisation_examples() {
    let data = Data::new();
    let job = |class: u16, used: u32, cl: [u16; 3], sex: Sex| {
        let mut cl = cl;
        let owner = TreeOwner {
            class,
            sex,
            base_level: 1,
            job_level: 1,
        };
        tree_job(owner, used, &mut cl, &data.jobs, &data.config)
    };
    let m = Sex::Male;
    // Lord Knight: Novice_High, Swordman_High, itself.
    assert_eq!(job(4008, 0, [0; 3], m), 4001);
    assert_eq!(job(4008, 9, [0; 3], m), 4002);
    assert_eq!(job(4008, 9 + 49, [0; 3], m), 4008);
    // Baby Knight: Baby, Baby Swordman.
    assert_eq!(job(4030, 0, [0; 3], m), 4023);
    assert_eq!(job(4030, 9, [0; 3], m), 4024);
    // Rune Knight: Knight below its third-class threshold (2nd var 50, 3rd default 50).
    assert_eq!(job(4054, 9 + 49, [0; 3], m), 7);
    assert_eq!(job(4054, 9 + 49 + 49, [0; 3], m), 4054);
    // Rune Knight_T: Lord Knight at that tier.
    assert_eq!(job(4060, 9 + 49, [0; 3], m), 4008);
    // Dragon Knight: UPPER forced on the reduced classes (Swordman_High, Rune Knight_T).
    assert_eq!(job(4252, 9, [0; 3], m), 4002);
    assert_eq!(job(4252, 9 + 49 + 49, [0; 3], m), 4060);
    // Super Novice: Novice, then itself (no 2nd-class tier).
    assert_eq!(job(23, 0, [0; 3], m), 0);
    assert_eq!(job(23, 9, [0; 3], m), 23);
    // Summoner skips the Novice tier.
    assert_eq!(job(4218, 0, [0; 3], m), 4218);
    // Dancer: the female Archer line keeps the sex pick at its own tier.
    assert_eq!(job(20, 9, [0; 3], Sex::Female), 3);
    assert_eq!(job(20, 9 + 49, [0; 3], Sex::Female), 20);
    // Off: always the real class.
    let mut off = Data::new();
    off.config.player_skillup_limit = false;
    let mut cl = [0; 3];
    assert_eq!(
        tree_job(owner(KNIGHT, (1, 1)), 0, &mut cl, &off.jobs, &off.config),
        KNIGHT
    );
}

/// ENG-SKILL-03 (hand data): ascending ids, level-0 entries with the real `inf` and sp/range
/// 0, `upFlag` 0 only at the class's tree max, Vulture's Eye widening Double Strafe.
#[test]
fn eng_skill_03_list_entries() {
    let data = Data::new();
    let set = set_visible(&data, KNIGHT, &[(NV_BASIC, 9), (SM_BASH, 5), (SM_SWORD, 1)]);
    let list = skill_list(&set, KNIGHT, &data.skills, &data.trees);
    let ids: Vec<u16> = list.iter().map(|s| s.id).collect();
    // A Knight with 15 points and no change level sees the Swordman tree (the default
    // change level is 50); `upFlag` still uses the Knight tree's max.
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
    let entry = |id: u16| list.iter().find(|s| s.id == id).expect("listed").clone();
    let bash = entry(SM_BASH);
    assert_eq!((bash.inf, bash.level, bash.sp, bash.range), (1, 5, 8, 1));
    assert!(!bash.upgradable, "Knight's tree max for Bash is 5");
    let magnum = entry(SM_MAGNUM);
    assert_eq!(
        (magnum.inf, magnum.level, magnum.sp, magnum.range),
        (4, 0, 0, 0)
    );
    assert!(magnum.upgradable);
    let basic = entry(NV_BASIC);
    assert!(!basic.upgradable && basic.sp == 0 && basic.range == 0);
    assert_eq!(&*basic.name, "NV_BASIC");

    // Archer: Double Strafe's range 9 plus Vulture's Eye 3.
    let set = set_visible(
        &data,
        ARCHER,
        &[(NV_BASIC, 9), (AC_VULTURE, 3), (AC_DOUBLE, 1)],
    );
    let list = skill_list(&set, ARCHER, &data.skills, &data.trees);
    let double = list.iter().find(|s| s.id == AC_DOUBLE).expect("listed");
    assert_eq!((double.sp, double.range), (12, 12));
}
