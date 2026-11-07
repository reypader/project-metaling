//! Skill resets and `@allskill`.

use ro_core::SkillFlag;

use crate::ecs::Entity;
use crate::job::{job_mapid, SECOND_MASK};
use crate::map_instance::{chat_line, MapInstance};
use crate::protocol_consts::SP_SKILLPOINT;
use crate::skill::skill_consts::{NV_BASIC, NV_TRICKDEAD, SG_DEVIL};
use crate::stats::{param, FollowUp};

/// Our wording (rAthena msgs 206, 208, 76).
const MSG_SKILLS_RESET: &str = "Your skills have been reset.";

const MSG_ALL_RESET: &str = "Your stats and skills have been reset.";

const MSG_ALL_SKILLS: &str = "You have learned every skill of your job.";

impl MapInstance {
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
        self.queue_followup(e, FollowUp::ToSelf(chat_line(MSG_SKILLS_RESET)));
    }

    /// `@reset` (spec §2.7; captured 859-892): the stat reset's pushes, the skill reset's, the
    /// recalc (deletes, list, diff), then our message.
    pub(crate) fn command_reset_all(&mut self, e: Entity) {
        self.reset_stats(e);
        self.reset_skills(e, true);
        self.queue_followup(e, FollowUp::ToSelf(chat_line(MSG_ALL_RESET)));
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
        self.queue_followup(e, FollowUp::ToSelf(chat_line(MSG_ALL_SKILLS)));
    }
}
