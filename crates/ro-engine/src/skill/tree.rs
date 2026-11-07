//! The skill tree rules: visibility, job resolution, points spent and `range2`.

use ro_core::{Sex, SkillFlag};
use ro_db::{JobDb, SkillData, SkillDb};
use ro_mechanics::BattleConfig;

use crate::job::{
    is_primary_fourth, job_mapid, mapid_job, BASE_MASK, JOBL_2, JOBL_BABY, JOBL_FOURTH, JOBL_THIRD,
    JOBL_UPPER, MAPID_SUMMONER, MAPID_SUPER_NOVICE, SECOND_MASK, THIRD_MASK,
};
use crate::skill::{SkillSet, TreeChange, TreeData, TreeOwner};

/// The Novice job (its max job level sets the first `skillup_limit` threshold).
const NOVICE: u16 = 0;

/// rAthena's Novice max job level when the job data has no Novice.
const NOVICE_MAX_JOB_LEVEL: u16 = 10;

/// Skills whose level widens other skills' `range2`.
const AC_VULTURE: &str = "AC_VULTURE";

const GS_SNAKEEYE: &str = "GS_SNAKEEYE";

const WL_RADIUS: &str = "WL_RADIUS";

const RA_RESEARCHTRAP: &str = "RA_RESEARCHTRAP";

const NJ_SHADOWJUMP: &str = "NJ_SHADOWJUMP";

/// Research Trap's range bonus by its level.
const RESEARCH_TRAP_RANGE: [i32; 11] = [0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5];

/// The skill points counted for the `skillup_limit` rule (rAthena `pc_calc_skillpoint`):
/// visible `Permanent` slots, without quest skills (unless learnable) and wedding skills.
fn points_spent(set: &SkillSet, skills: &SkillDb, config: &BattleConfig) -> u32 {
    set.iter()
        .filter(|slot| slot.visible && slot.level > 0 && slot.flag == SkillFlag::Permanent)
        .filter(|slot| {
            skills.get(slot.id).is_some_and(|data| {
                let flags = data.flags;
                // Not a quest skill (unless learnable), not a wedding skill (unless spirit).
                (!flags.quest || config.quest_skill_learn) && (!flags.wedding || flags.spirit)
            })
        })
        .map(|slot| u32::from(slot.level))
        .sum()
}

/// The max job level of the job with `mapid` (a default class-change level).
fn max_job_level(mapid: u32, sex: Sex, jobs: &JobDb) -> u16 {
    mapid_job(mapid, sex)
        .and_then(|job| jobs.get(job))
        .map_or(0, |job| job.max_job_level)
}

/// The job whose tree the player sees (rAthena `pc_calc_skilltree_normalize_job`; catalog
/// `db/skills.md` "Runtime" step 1). With `player_skillup_limit` a class shows a lower class's
/// tree until enough points are spent: 9 in the Novice tree (not for the Summoner line), then
/// `change_level - 1` per class step. A class-change level that is unset defaults to the lower
/// class's max job level and is stored.
pub(crate) fn tree_job(
    owner: TreeOwner,
    used: u32,
    change_level: &mut [u16; 3],
    jobs: &JobDb,
    config: &BattleConfig,
) -> u16 {
    if !config.player_skillup_limit {
        return owner.class;
    }
    let mapid = job_mapid(owner.class);
    let Some(mut reduced) = reduce(mapid, owner.sex, used, change_level, jobs) else {
        return owner.class;
    };
    if mapid & JOBL_UPPER != 0 && mapid_job(reduced | JOBL_UPPER, owner.sex).is_some() {
        reduced |= JOBL_UPPER;
    }
    if mapid & JOBL_BABY != 0 && mapid_job(reduced | JOBL_BABY, owner.sex).is_some() {
        reduced |= JOBL_BABY;
    }
    if is_primary_fourth(mapid) && !is_primary_fourth(reduced) {
        reduced |= JOBL_UPPER;
    }
    mapid_job(reduced, owner.sex).unwrap_or(owner.class)
}

/// The lower mapid while too few points are spent, or `None` for the class's own tree. Every
/// comparison is a strict `used < threshold`.
fn reduce(
    mapid: u32,
    sex: Sex,
    mut used: u32,
    change_level: &mut [u16; 3],
    jobs: &JobDb,
) -> Option<u32> {
    if mapid & BASE_MASK != MAPID_SUMMONER {
        let novice = jobs
            .get(NOVICE)
            .map_or(NOVICE_MAX_JOB_LEVEL, |job| job.max_job_level);
        let threshold = u32::from(novice.saturating_sub(1));
        if used < threshold {
            return Some(0);
        }
        used -= threshold;
    }
    // (applies, the lower class's mask, the change-level slot)
    let tiers = [
        (
            mapid & JOBL_2 != 0 && mapid & SECOND_MASK != MAPID_SUPER_NOVICE,
            BASE_MASK,
            0,
        ),
        (mapid & JOBL_THIRD != 0, SECOND_MASK, 1),
        (mapid & JOBL_FOURTH != 0, THIRD_MASK, 2),
    ];
    for (applies, mask, slot) in tiers {
        if !applies {
            continue;
        }
        let lower = mapid & mask;
        if change_level[slot] == 0 {
            change_level[slot] = max_job_level(lower, sex, jobs);
        }
        let threshold = u32::from(change_level[slot].saturating_sub(1));
        if used < threshold && mapid_job(lower, sex).is_some() {
            return Some(lower);
        }
        used = used.saturating_sub(threshold);
    }
    None
}

/// rAthena's learnability filter: a skill still at level 0 never becomes visible if it is a
/// quest skill (unless learnable), a wedding skill or a spirit skill.
fn hidden_at_zero(data: &SkillData, config: &BattleConfig) -> bool {
    let flags = data.flags;
    (flags.quest && !config.quest_skill_learn) || flags.wedding || flags.spirit
}

/// Rebuilds which slots are visible (rAthena `pc_calc_skilltree`; spec §2.2). Pure apart from
/// the stored default class-change levels.
///
/// 1. The tree job from the points counted on the current visible set.
/// 2. Every `Permanent` slot is hidden; `Granted` slots stay.
/// 3. Tree entries become visible, to a fixed point, when their prerequisites are visible and
///    learned high enough and the base/job level gates are met (`player_skillfree` skips the
///    checks); the learnability filter applies to level-0 skills.
/// 4. Hidden level-0 slots are dropped.
pub(crate) fn compute_visible(
    set: &mut SkillSet,
    change_level: &mut [u16; 3],
    owner: TreeOwner,
    data: TreeData<'_>,
) -> TreeChange {
    let used = points_spent(set, data.skills, data.config);
    let job = tree_job(owner, used, change_level, data.jobs, data.config);
    for slot in set.iter_mut() {
        slot.was_visible = slot.visible;
        if slot.flag == SkillFlag::Permanent {
            slot.visible = false;
        }
    }
    let tree = data.trees.tree(job);
    loop {
        let mut grew = false;
        for entry in tree {
            if set.get(entry.skill).is_some_and(|slot| slot.visible) {
                continue;
            }
            let Some(skill) = data.skills.get(entry.skill) else {
                continue;
            };
            if set.level(entry.skill) == 0 && hidden_at_zero(skill, data.config) {
                continue;
            }
            let open = data.config.player_skillfree
                || (entry.requires.iter().all(|need| {
                    // Learned or granted (rAthena refuses only temporary and copied skills).
                    set.get(need.skill)
                        .is_some_and(|slot| slot.visible && slot.level >= need.level)
                }) && owner.base_level >= entry.base_level
                    && owner.job_level >= entry.job_level);
            if open {
                set.ensure(entry.skill).visible = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut change = TreeChange::default();
    let mut saved_rows_changed = false;
    for slot in set.iter() {
        if slot.visible != slot.was_visible {
            change.changed = true;
            saved_rows_changed |= slot.level > 0;
            if slot.was_visible {
                change.vanished.push(slot.id);
            }
        }
    }
    if saved_rows_changed {
        // A learned level hidden or shown again changes the rows a save writes.
        set.touch();
    }
    set.slots.retain(|slot| slot.visible || slot.level > 0);
    change
}

/// `range2` of a learned skill (catalog `db/skills.md` "range2"): the absolute range at
/// `level`, plus the bonuses from the player's Vulture's Eye, Snake Eye, Radius and Research
/// Trap; Shadow Jump's flag replaces it with Shadow Jump's range. 0 at level 0.
pub(crate) fn range2(data: &SkillData, level: u8, set: &SkillSet, skills: &SkillDb) -> u16 {
    if level == 0 {
        return 0;
    }
    let learned = |name: &str| skills.by_name(name).map_or(0, |skill| set.level(skill.id));
    let flags = data.flags;
    let mut range = i32::from(data.range(level)).abs();
    if flags.alter_range_vulture {
        range += i32::from(learned(AC_VULTURE));
    }
    if flags.alter_range_snake_eye {
        range += i32::from(learned(GS_SNAKEEYE));
    }
    if flags.alter_range_radius {
        range += i32::from(learned(WL_RADIUS));
    }
    if flags.alter_range_research_trap {
        let level = usize::from(learned(RA_RESEARCHTRAP)).min(RESEARCH_TRAP_RANGE.len() - 1);
        range += RESEARCH_TRAP_RANGE[level];
    }
    if flags.alter_range_shadow_jump {
        range = skills
            .by_name(NJ_SHADOWJUMP)
            .map_or(0, |jump| i32::from(jump.range(set.level(jump.id))));
    }
    u16::try_from(range.max(0)).unwrap_or(u16::MAX)
}
