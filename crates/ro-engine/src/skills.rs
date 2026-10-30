//! Skills, learn only (spec `specs/skills/spec.md` §2; catalog `db/skills.md` "Runtime",
//! `mechanics/skills.md` §2-§3): the learned set, which of it the client sees, and the
//! `0x010f` list.
//!
//! A skill's effects are not modelled yet: the learned set is what casting and passive bonuses
//! will read (DESIGN.md §9.9).

use std::sync::Arc;

use ro_core::{LearnedSkill, ServerEvent, Sex, SkillFlag, SkillInfo};
use ro_db::{JobDb, SkillData, SkillDb, SkillTreeDb};
use ro_mechanics::BattleConfig;
use smallvec::SmallVec;

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::job::{
    is_primary_fourth, job_mapid, mapid_job, BASE_MASK, JOBL_2, JOBL_BABY, JOBL_FOURTH, JOBL_THIRD,
    JOBL_UPPER, MAPID_SUMMONER, MAPID_SUPER_NOVICE, SECOND_MASK, THIRD_MASK,
};
use crate::stats::FollowUp;
use crate::status::{param, SP_SKILLPOINT};

/// The Novice job (its max job level sets the first `skillup_limit` threshold).
const NOVICE: u16 = 0;
/// rAthena's Novice max job level when the job data has no Novice.
const NOVICE_MAX_JOB_LEVEL: u16 = 10;
/// Entries that fit rAthena's 8192-byte `0x010f` (4-byte header, 37 bytes each).
const MAX_LIST_ENTRIES: usize = 221;
/// Skills whose level widens other skills' `range2`.
const AC_VULTURE: &str = "AC_VULTURE";
const GS_SNAKEEYE: &str = "GS_SNAKEEYE";
const WL_RADIUS: &str = "WL_RADIUS";
const RA_RESEARCHTRAP: &str = "RA_RESEARCHTRAP";
const NJ_SHADOWJUMP: &str = "NJ_SHADOWJUMP";
/// Skills the resets and `@allskill` treat specially.
const NV_BASIC: &str = "NV_BASIC";
const NV_TRICKDEAD: &str = "NV_TRICKDEAD";
const SG_DEVIL: &str = "SG_DEVIL";
/// Our wording (rAthena msgs 206, 208, 76).
const MSG_SKILLS_RESET: &str = "Your skills have been reset.";
const MSG_ALL_RESET: &str = "Your stats and skills have been reset.";
const MSG_ALL_SKILLS: &str = "You have learned every skill of your job.";
/// Research Trap's range bonus by its level.
const RESEARCH_TRAP_RANGE: [i32; 11] = [0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5];

fn chat(text: &str) -> ServerEvent {
    ServerEvent::SelfChat {
        text: text.as_bytes().to_vec(),
    }
}

/// One skill a player has or can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SkillSlot {
    pub(crate) id: u16,
    pub(crate) level: u8,
    pub(crate) flag: SkillFlag,
    /// In the client's list (rAthena `skill[].id != 0`). A hidden slot with a level is one from
    /// a previous class; it counts toward nothing and is not saved.
    pub(crate) visible: bool,
    /// `visible` before the current `compute_visible`, to report what changed.
    was_visible: bool,
}

impl SkillSlot {
    fn new(id: u16, level: u8, flag: SkillFlag) -> Self {
        Self {
            id,
            level,
            flag,
            visible: false,
            was_visible: false,
        }
    }
}

/// A player's skills: a slot for every visible tree skill (level 0 allowed) and for every
/// skill with a level. Sorted by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SkillSet {
    slots: Vec<SkillSlot>,
    /// Bumped by every change to a level or flag, so a save writes the rows only when they
    /// changed (spec §2.9).
    revision: u32,
}

impl SkillSet {
    /// The set of the stored rows, all visible as rAthena loads them (`id` set), so the first
    /// `compute_visible` counts them; then a transfer's hidden levels, hidden.
    pub(crate) fn from_spawn(rows: &[LearnedSkill], hidden: &[LearnedSkill]) -> Self {
        let mut slots: Vec<SkillSlot> = rows
            .iter()
            .map(|row| (row, true))
            .chain(hidden.iter().map(|row| (row, false)))
            .filter(|(row, _)| row.level > 0)
            .map(|(row, visible)| SkillSlot {
                visible,
                ..SkillSlot::new(row.id, row.level, row.flag)
            })
            .collect();
        // Stable: a visible row wins over a hidden one with the same id.
        slots.sort_by_key(|slot| slot.id);
        slots.dedup_by_key(|slot| slot.id);
        Self { slots, revision: 0 }
    }

    pub(crate) fn revision(&self) -> u32 {
        self.revision
    }

    /// Records a level or flag change.
    pub(crate) fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn get(&self, id: u16) -> Option<&SkillSlot> {
        self.index(id).ok().map(|i| &self.slots[i])
    }

    pub(crate) fn get_mut(&mut self, id: u16) -> Option<&mut SkillSlot> {
        self.index(id).ok().map(|i| &mut self.slots[i])
    }

    /// The learned level of `id`; 0 when absent.
    pub(crate) fn level(&self, id: u16) -> u8 {
        self.get(id).map_or(0, |slot| slot.level)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &SkillSlot> {
        self.slots.iter()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut SkillSlot> {
        self.slots.iter_mut()
    }

    /// The rows a save writes: visible slots with a level (spec §2.9). Hidden levels are not
    /// saved, so they are lost at the next save (captured).
    pub(crate) fn saved_rows(&self) -> Vec<LearnedSkill> {
        self.rows(|slot| slot.visible)
    }

    /// Hidden slots with a level: what a cross-map transfer carries besides the saved rows.
    pub(crate) fn hidden_rows(&self) -> Vec<LearnedSkill> {
        self.rows(|slot| !slot.visible)
    }

    fn rows(&self, keep: impl Fn(&SkillSlot) -> bool) -> Vec<LearnedSkill> {
        self.slots
            .iter()
            .filter(|slot| slot.level > 0 && keep(slot))
            .map(|slot| LearnedSkill {
                id: slot.id,
                level: slot.level,
                flag: slot.flag,
            })
            .collect()
    }

    fn index(&self, id: u16) -> Result<usize, usize> {
        self.slots.binary_search_by_key(&id, |slot| slot.id)
    }

    /// The slot for `id`, inserted (level 0, `Permanent`, hidden) if missing.
    pub(crate) fn ensure(&mut self, id: u16) -> &mut SkillSlot {
        let i = match self.index(id) {
            Ok(i) => i,
            Err(i) => {
                self.slots
                    .insert(i, SkillSlot::new(id, 0, SkillFlag::Permanent));
                i
            }
        };
        &mut self.slots[i]
    }
}

/// The read-only data the tree needs.
#[derive(Clone, Copy)]
pub(crate) struct TreeData<'a> {
    pub(crate) skills: &'a SkillDb,
    pub(crate) trees: &'a SkillTreeDb,
    pub(crate) jobs: &'a JobDb,
    pub(crate) config: &'a BattleConfig,
}

/// Who the tree is computed for.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TreeOwner {
    pub(crate) class: u16,
    pub(crate) sex: Sex,
    pub(crate) base_level: u16,
    pub(crate) job_level: u16,
}

/// What a `compute_visible` call changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TreeChange {
    /// Some slot appeared or vanished.
    pub(crate) changed: bool,
    /// Ids that went from visible to hidden, ascending.
    pub(crate) vanished: SmallVec<[u16; 8]>,
}

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
fn range2(data: &SkillData, level: u8, set: &SkillSet, skills: &SkillDb) -> u16 {
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

/// One list entry: the `0x010f` / `0x010e` values of a slot (spec §2.3).
pub(crate) fn skill_info(
    slot: &SkillSlot,
    data: &SkillData,
    set: &SkillSet,
    class: u16,
    skills: &SkillDb,
    trees: &SkillTreeDb,
) -> SkillInfo {
    SkillInfo {
        id: slot.id,
        inf: i32::from(data.inf),
        level: u16::from(slot.level),
        sp: data.sp(slot.level),
        range: range2(data, slot.level, set, skills),
        name: Arc::from(data.name.as_str()),
        upgradable: slot.flag == SkillFlag::Permanent
            && slot.level < trees.max_level(class, slot.id, skills),
    }
}

/// The `0x010f` list: every visible slot, ascending id. `upFlag` uses the real class's max.
pub(crate) fn skill_list(
    set: &SkillSet,
    class: u16,
    skills: &SkillDb,
    trees: &SkillTreeDb,
) -> Vec<SkillInfo> {
    let list: Vec<SkillInfo> = set
        .iter()
        .filter(|slot| slot.visible)
        .filter_map(|slot| {
            let data = skills.get(slot.id)?;
            Some(skill_info(slot, data, set, class, skills, trees))
        })
        .collect();
    // No tree comes near rAthena's one-packet cap (it would overflow into `0x0111`).
    debug_assert!(list.len() <= MAX_LIST_ENTRIES, "{} entries", list.len());
    list
}

impl MapInstance {
    /// Recomputes `e`'s visible skills; `None` for a non-player.
    pub(crate) fn refresh_skills(&mut self, e: Entity) -> Option<TreeChange> {
        let info = self.world.players.get_mut(e)?;
        let owner = TreeOwner {
            class: info.job,
            sex: info.sex,
            base_level: info.clevel,
            job_level: info.status.job_level,
        };
        let data = TreeData {
            skills: &self.skills,
            trees: &self.skill_tree,
            jobs: &self.jobs,
            config: &self.battle,
        };
        Some(compute_visible(
            &mut info.skills,
            &mut info.change_level,
            owner,
            data,
        ))
    }

    /// `0x010f` with `e`'s visible skills.
    pub(crate) fn send_skill_list(&mut self, e: Entity) {
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let skills = skill_list(&info.skills, info.job, &self.skills, &self.skill_tree);
        self.emit_self(e, ServerEvent::SkillList { skills });
    }

    /// `0x0112` (spec §2.5; catalog `mechanics/skills.md` §2, §10.3). Refused silently unless
    /// the skill is known (not a guild skill), a point is free, the slot is visible and
    /// `Permanent`, and below the real class's tree max. On success the tree is re-checked at
    /// once, so later requests in the same burst see this one's unlocks. Then, at once: for a
    /// passive skill that changed the tree, the deletes and list first (rAthena sends them from
    /// its recalc); `0x010e`; `SP_SKILLPOINT`; the list. No recalc (passives give nothing
    /// yet), no save, nothing to the area.
    pub(crate) fn upgrade_skill(&mut self, e: Entity, id: u16) {
        let Some(data) = self.skills.get(id) else {
            return;
        };
        let passive = data.is_passive();
        if data.flags.guild {
            return;
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let max = self.skill_tree.max_level(info.job, id, &self.skills);
        let Some(slot) = info.skills.get_mut(id) else {
            return;
        };
        if info.status.skill_point == 0
            || !slot.visible
            || slot.flag != SkillFlag::Permanent
            || slot.level >= max
        {
            return;
        }
        slot.level += 1;
        info.status.skill_point -= 1;
        let skill_point = info.status.skill_point;
        info.skills.touch();
        let Some(change) = self.refresh_skills(e) else {
            return;
        };
        // A passive's tree change goes first (rAthena sends it from its recalc); an active
        // skill's deletes go before the trailing list.
        let vanished = if passive {
            if change.changed {
                self.send_tree_change(e, change);
            }
            SmallVec::new()
        } else {
            change.vanished
        };
        if let Some(event) = self.skill_update(e, id) {
            self.emit_self(e, event);
        }
        self.emit_self(e, param(SP_SKILLPOINT, skill_point.into()));
        // rAthena's active path only adds to the tree, so it never deletes. Our full re-check
        // can also hide slots a same-tick reset left listed; the client needs their deletes.
        for id in vanished {
            self.emit_self(e, ServerEvent::SkillDelete { id });
        }
        self.send_skill_list(e);
    }

    /// rAthena `pc_resetskill` (spec §2.7; catalog `mechanics/skills.md` §6): every slot with a
    /// level goes to 0 and `Permanent`, its levels refunded, except wedding and spirit skills,
    /// `Granted` slots and NV_BASIC outside the Novice line. NV_TRICKDEAD outside the Novice
    /// line goes to 0 without a refund. Quest skills (unless learnable) are kept, or wiped
    /// without a refund with `quest_skill_reset`. Nothing happens when nothing is refunded.
    ///
    /// With `send`: `SP_SKILLPOINT` and the list of the current set (zeroed, nothing removed
    /// yet) at once, and a recalc whose tree re-check sends the deletes and the new list.
    /// Without (`@jlvl -n`): only the re-check is marked. Returns the refund.
    pub(crate) fn reset_skills(&mut self, e: Entity, send: bool) -> u32 {
        let novice_line = |job: u16| job_mapid(job) & SECOND_MASK == 0;
        let named = |name: &str| self.skills.by_name(name).map(|skill| skill.id);
        let (basic, trick_dead) = (named(NV_BASIC), named(NV_TRICKDEAD));
        let Some(info) = self.world.players.get_mut(e) else {
            return 0;
        };
        let novice = novice_line(info.job);
        let config = &self.battle;
        let mut refund = 0u32;
        for slot in info.skills.iter_mut().filter(|slot| slot.level > 0) {
            let Some(data) = self.skills.get(slot.id) else {
                continue;
            };
            let flags = data.flags;
            if flags.wedding || flags.spirit {
                continue;
            }
            if !novice && Some(slot.id) == trick_dead {
                slot.level = 0;
                slot.flag = SkillFlag::Permanent;
                continue;
            }
            if (!novice && Some(slot.id) == basic) || slot.flag == SkillFlag::Granted {
                continue;
            }
            if flags.quest && !config.quest_skill_learn {
                if config.quest_skill_reset {
                    slot.level = 0;
                    slot.flag = SkillFlag::Permanent;
                }
                continue;
            }
            refund += u32::from(slot.level);
            slot.level = 0;
            slot.flag = SkillFlag::Permanent;
        }
        info.skills.touch();
        // rAthena returns here too: with nothing refunded (only NV_TRICKDEAD or quest skills
        // zeroed) there are no pushes and no tree re-check.
        if refund == 0 {
            return 0;
        }
        let total = u32::from(info.status.skill_point) + refund;
        info.status.skill_point = u16::try_from(total).unwrap_or(u16::MAX);
        let skill_point = info.status.skill_point;
        if send {
            self.emit_self(e, param(SP_SKILLPOINT, skill_point.into()));
            self.send_skill_list(e);
            self.mark_dirty(e, false);
        }
        self.mark_skills_dirty(e);
        refund
    }

    /// `@skreset` / `@resetskill` (spec §2.7; captured 766-773): the reset, the recalc's deletes
    /// and list, then our message.
    pub(crate) fn command_reset_skills(&mut self, e: Entity) {
        self.reset_skills(e, true);
        self.queue_followup(e, FollowUp::ToSelf(chat(MSG_SKILLS_RESET)));
    }

    /// `@reset` (spec §2.7; captured 859-892): the stat reset's pushes, the skill reset's, the
    /// recalc (deletes, list, diff), then our message.
    pub(crate) fn command_reset_all(&mut self, e: Entity) {
        self.reset_stats(e);
        self.reset_skills(e, true);
        self.queue_followup(e, FollowUp::ToSelf(chat(MSG_ALL_RESET)));
    }

    /// `@allskill` (spec §2.8; captured 895-898; rAthena `pc_allskillup`): every entry of the
    /// real class's tree at its tree max (not quest skills unless learnable, wedding, spirit or
    /// SG_DEVIL), no free points, the tree computed now. At once: the list, `SP_SKILLPOINT 0`;
    /// then our message.
    pub(crate) fn command_all_skills(&mut self, e: Entity) {
        let devil = self.skills.by_name(SG_DEVIL).map(|skill| skill.id);
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        for entry in self.skill_tree.tree(info.job) {
            let Some(data) = self.skills.get(entry.skill) else {
                continue;
            };
            let flags = data.flags;
            if (flags.quest && !self.battle.quest_skill_learn)
                || flags.wedding
                || flags.spirit
                || Some(entry.skill) == devil
            {
                continue;
            }
            let slot = info.skills.ensure(entry.skill);
            slot.level = entry.max_level;
            // rAthena sets the id too, so the tree's normalisation counts these points.
            slot.visible = true;
        }
        info.skills.touch();
        info.status.skill_point = 0;
        self.refresh_skills(e);
        self.send_skill_list(e);
        self.emit_self(e, param(SP_SKILLPOINT, 0));
        self.queue_followup(e, FollowUp::ToSelf(chat(MSG_ALL_SKILLS)));
    }

    /// `0x010e` for `id`, with the same values as its list entry.
    fn skill_update(&self, e: Entity, id: u16) -> Option<ServerEvent> {
        let info = self.world.players.get(e)?;
        let slot = info.skills.get(id)?;
        let data = self.skills.get(id)?;
        let entry = skill_info(
            slot,
            data,
            &info.skills,
            info.job,
            &self.skills,
            &self.skill_tree,
        );
        Some(ServerEvent::SkillUpdate {
            id,
            level: entry.level,
            sp: entry.sp,
            range: entry.range,
            upgradable: entry.upgradable,
        })
    }

    /// The deletes and the list for a changed tree.
    fn send_tree_change(&mut self, e: Entity, change: TreeChange) {
        for id in change.vanished {
            self.emit_self(e, ServerEvent::SkillDelete { id });
        }
        self.send_skill_list(e);
    }

    /// The tree re-check (spec §2.6): when the visible set changed, `0x0441` for each id that
    /// vanished, then the list. Nothing when it didn't change.
    pub(crate) fn send_tree_diff(&mut self, e: Entity) {
        let Some(change) = self.refresh_skills(e) else {
            return;
        };
        if change.changed {
            self.send_tree_change(e, change);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

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
    const SKILLS: &str = "Header: {Type: SKILL_DATA, Version: 1}
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
            let skills = SkillDb::from_rows("t", SkillDb::parse("t", SKILLS).expect("parse"))
                .expect("skills");
            let trees = SkillTreeDb::from_rows(
                "t",
                SkillTreeDb::parse("t", TREES).expect("parse"),
                &skills,
            )
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
}
