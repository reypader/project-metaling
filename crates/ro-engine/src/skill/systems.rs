//! Skill systems: refresh, list, level-up and tree diffs.

use ro_core::{ServerEvent, SkillFlag};
use smallvec::SmallVec;

use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::protocol_consts::SP_SKILLPOINT;
use crate::skill::{compute_visible, skill_info, skill_list, TreeChange, TreeData, TreeOwner};
use crate::stats::param;

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
