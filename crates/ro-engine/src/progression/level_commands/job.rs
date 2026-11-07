//! `@job`: change the job, then fix up the levels, points, skills, gear and looks.

use ro_core::ServerEvent;
use ro_db::Job;
use ro_mechanics::next_stat_cost;

use super::msg_consts::{MSG_JOB_USAGE, MSG_SAME_JOB};
use super::points::gets_transcendent_points;
use crate::ecs::Entity;
use crate::job::{class_flags, job_for_sex, job_mapid, JOBL_2, JOBL_FOURTH, JOBL_THIRD};
use crate::map_instance::{chat_line, MapInstance};
use crate::protocol_consts::{
    LOOK_BASE, LOOK_BODY2, LOOK_CLOTHES_COLOR, LOOK_HAIR, SP_USTR, STAT_COUNT,
};
use crate::stats::FollowUp;

/// Which class-change level a job change records (rAthena `pc_jobchange` step 1): into a 4th
/// class the 4th, else into a 3rd class the 3rd, else when a 2nd-class bit appears the 2nd.
fn change_level_slot(old: u32, new: u32) -> Option<usize> {
    let gains = |bits: u32| new & bits != 0 && old & bits == 0;
    if gains(JOBL_FOURTH) {
        Some(2)
    } else if gains(JOBL_THIRD) {
        Some(1)
    } else if gains(JOBL_2) {
        Some(0)
    } else {
        None
    }
}

impl MapInstance {
    /// The job `@job`'s argument names: an id, else a case-insensitive `JobDb` name.
    fn find_job(&self, params: &str) -> Option<&Job> {
        let word = params.trim();
        match word.parse::<u16>() {
            Ok(id) => self.jobs.get(id),
            Err(_) => self.jobs.by_name(word),
        }
    }

    /// `@job <id|name>` (stats spec §2.6, traits spec §2.7): the minimal `pc_jobchange` step
    /// set, with the transcendent and trait point grants and the resets they can force.
    pub(crate) fn command_job(&mut self, e: Entity, params: &str) {
        if params.trim().is_empty() {
            return self.tell(e, MSG_JOB_USAGE);
        }
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let (current, sex, level) = (info.job, info.sex, info.clevel);
        let Some(job) = self.find_job(params) else {
            return self.tell(e, &format!("No job matches '{}'.", params.trim()));
        };
        let new_job = job_for_sex(job.id, sex);
        // The sex pick can name a job the data lacks; the deny-listed mounts and dummies are
        // not in `JobDb` (the importer skips them).
        let Some(job) = self.jobs.get(new_job) else {
            return self.tell(e, &format!("No job matches '{}'.", params.trim()));
        };
        if new_job == current {
            return self.tell(e, MSG_SAME_JOB);
        }
        let (max_base, name) = (job.max_base_level, job.name.clone());

        // Step 1: the class-change job level (skills spec §2.8). Steps 1-3: the job, job level
        // 1, and the base level cap with its reset and tail.
        let capped = level > max_base;
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if let Some(slot) = change_level_slot(job_mapid(current), job_mapid(new_job)) {
            info.change_level[slot] = info.status.job_level;
        }
        info.job = new_job;
        info.status.job_level = 1;
        info.status.job_exp = 0;
        if capped {
            info.clevel = max_base;
            info.status.base_exp = 0;
            self.reset_stats(e);
            for event in self.base_tail(e) {
                self.emit_self(e, event);
            }
        }

        // The transcendent points, into or out of an upper or primary 4th class.
        let old_class = class_flags(job_mapid(current));
        let new_class = class_flags(job_mapid(new_job));
        match (
            gets_transcendent_points(&old_class),
            gets_transcendent_points(&new_class),
        ) {
            (false, true) => self.add_status_points(e, self.battle.transcendent_status_points),
            (true, false) => self.take_status_points(e, self.battle.transcendent_status_points),
            _ => {}
        }

        // Step 4: the stat costs against the new cap.
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let cap = self.battle.max_stat(&new_class);
        let stats = [
            info.status.str,
            info.status.agi,
            info.status.vit,
            info.status.int,
            info.status.dex,
            info.status.luk,
        ];
        let hair_style = info.hair_style;
        for (offset, stat) in (0..STAT_COUNT).zip(stats) {
            self.emit_self(
                e,
                ServerEvent::StatusCost {
                    var_id: SP_USTR + offset,
                    value: next_stat_cost(stat, cap),
                },
            );
        }

        // The trait points, into or out of a trait job. Step 5: the job level.
        match (old_class.trait_job, new_class.trait_job) {
            (false, true) => self.grant_job_trait_points(e, &new_class),
            (true, false) => self.take_job_trait_points(e, &new_class),
            _ => {}
        }
        self.send_job_level(e);

        // Step 6: gear the new class can't wear comes off. Step 7: the class looks.
        self.unequip_unusable(e);
        for (look_type, value) in [
            (LOOK_BASE, u32::from(new_job)),
            (LOOK_HAIR, u32::from(hair_style)),
            (LOOK_CLOTHES_COLOR, 0),
            (LOOK_BODY2, u32::from(new_job)),
        ] {
            self.send_look(e, look_type, value, 0);
        }
        // Step 8: the new class's skill list, counted on the old class's visible skills
        // (rAthena's first pass). Step 9: the tick's recalc counts again on the new set; when
        // that trims the tree it sends the deletes and a second list after `SP_MAXWEIGHT`
        // (captured Dancer -> Assassin; skills spec §2.8).
        self.refresh_skills(e);
        self.send_skill_list(e);
        self.mark_dirty(e, false);
        self.mark_skills_dirty(e);

        // Steps 10-12, after the recalc: the equip looks, the message, the save.
        self.queue_followup(e, FollowUp::EquipLooks);
        let message = format!("Your job is now {name}.");
        self.queue_followup(e, FollowUp::ToSelf(chat_line(&message)));
        self.queue_followup(e, FollowUp::Save);
    }
}
