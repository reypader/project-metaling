//! `@jlvl`: raise or lower the job level.

use ro_core::ServerEvent;

use super::args::level_argument;
use super::msg_consts::{MSG_JLVL_USAGE, MSG_JOB_AT_MAX, MSG_JOB_AT_MIN, MSG_NO_JOB_DATA};
use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::progression::points;
use crate::protocol_consts::{
    EFFECT_JOB_LEVEL_UP, SP_JOBEXP, SP_JOBLEVEL, SP_NEXTJOBEXP, SP_SKILLPOINT,
};
use crate::stats::{exp, param};

impl MapInstance {
    /// `@jlvl <n>` (spec §2.5): everything goes out at once, then the tick's tree re-check and
    /// diff. Up sends the effect to the area first; down sends no effect, and resets the skills
    /// when the free points can't cover the levels removed (skills spec §2.7).
    pub(crate) fn command_job_level(&mut self, e: Entity, params: &str) {
        let Some(n) = level_argument(params) else {
            return self.tell(e, MSG_JLVL_USAGE);
        };
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let (job_id, level) = (info.job, info.status.job_level);
        let Some(max) = self.jobs.get(job_id).map(|job| job.max_job_level) else {
            return self.tell(e, MSG_NO_JOB_DATA);
        };
        let to = if n > 0 {
            if level >= max {
                return self.tell(e, MSG_JOB_AT_MAX);
            }
            level + u16::try_from(n).unwrap_or(u16::MAX).min(max - level)
        } else {
            if level <= 1 {
                return self.tell(e, MSG_JOB_AT_MIN);
            }
            level
                - u16::try_from(n.unsigned_abs())
                    .unwrap_or(u16::MAX)
                    .min(level - 1)
        };
        // Down by more than the free points: the skills are reset first, silently (skills spec
        // §2.7); the tick's tree re-check sends the deletes and the list.
        if to < level
            && self
                .world
                .players
                .get(e)
                .is_some_and(|info| info.status.skill_point < level - to)
        {
            self.reset_skills(e, false);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let status = &mut info.status;
        status.job_level = to;
        status.job_exp = 0;
        status.skill_point = if to > level {
            points(u32::from(status.skill_point) + u32::from(to - level))
        } else {
            status.skill_point.saturating_sub(level - to)
        };
        let skill_point = status.skill_point;
        if to > level {
            let gid = self.gid(e);
            self.emit_aoi(
                e,
                false,
                ServerEvent::SpecialEffect {
                    gid,
                    effect: EFFECT_JOB_LEVEL_UP,
                },
            );
            self.tell(e, &format!("Your job level went up to {to}."));
        } else {
            self.tell(e, &format!("Your job level went down to {to}."));
        }
        self.send_job_level(e);
        self.emit_self(e, param(SP_SKILLPOINT, skill_point.into()));
        self.mark_dirty(e, false);
        self.mark_skills_dirty(e);
    }

    /// `SP_JOBLEVEL`, `SP_JOBEXP` and `SP_NEXTJOBEXP`.
    pub(crate) fn send_job_level(&mut self, e: Entity) {
        for event in self.job_level_tail(e) {
            self.emit_self(e, event);
        }
    }

    /// `SP_JOBLEVEL`, `SP_JOBEXP` and `SP_NEXTJOBEXP` with the player's current values.
    pub(crate) fn job_level_tail(&self, e: Entity) -> Vec<ServerEvent> {
        let Some(info) = self.world.players.get(e) else {
            return Vec::new();
        };
        let (level, job_exp) = (info.status.job_level, info.status.job_exp);
        vec![
            param(SP_JOBLEVEL, level.into()),
            ServerEvent::LongLongParameterChanged {
                var_id: SP_JOBEXP,
                value: exp(job_exp),
            },
            ServerEvent::LongLongParameterChanged {
                var_id: SP_NEXTJOBEXP,
                value: exp(self.jobs.next_job_exp(info.job, level)),
            },
        ]
    }
}
