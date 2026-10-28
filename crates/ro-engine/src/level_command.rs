//! The GM level and job commands (spec `specs/stats/spec.md` §2.5-§2.6; catalog
//! `mechanics/atcommands.md` "@baselevelup / @joblevelup", `mechanics/job_change.md`). Reply
//! wording is our own.
//!
//! Each command changes the character at once and marks it dirty. What rAthena sends before
//! its recalc goes out at once; what it sends after is queued as follow-ups, so the wire order
//! matches the captures (spec §2.1).

use ro_core::{CharacterStatus, ServerEvent};
use ro_db::Job;
use ro_mechanics::next_stat_cost;

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::job::{class_flags, job_for_sex, job_mapid};
use crate::settings::nv_basic;
use crate::stats::FollowUp;
use crate::status::{
    param, PLACEHOLDER_NEXT_BASE_EXP, PLACEHOLDER_NEXT_JOB_EXP, SP_BASEEXP, SP_BASELEVEL,
    SP_JOBEXP, SP_JOBLEVEL, SP_NEXTBASEEXP, SP_NEXTJOBEXP, SP_SKILLPOINT, SP_STATUSPOINT,
    SP_TRAITPOINT, SP_UPOW, SP_USTR,
};

/// `0x019b` effects.
const EFFECT_BASE_LEVEL_UP: u32 = 0;
const EFFECT_JOB_LEVEL_UP: u32 = 1;
/// `0x01d7` look types `@job` sends for the new class.
const LOOK_BASE: u8 = 0;
const LOOK_HAIR: u8 = 1;
const LOOK_CLOTHES_COLOR: u8 = 7;
const LOOK_BODY2: u8 = 13;
/// STR..LUK and POW..CRT.
const STAT_COUNT: u16 = 6;

const MSG_BLVL_USAGE: &str = "Usage: @blvl <levels> (a negative number lowers)";
const MSG_JLVL_USAGE: &str = "Usage: @jlvl <levels> (a negative number lowers)";
const MSG_JOB_USAGE: &str = "Usage: @job <job name or id>";
const MSG_BASE_AT_MAX: &str = "Your base level is already the highest your job allows.";
const MSG_BASE_AT_MIN: &str = "Your base level is already 1.";
const MSG_JOB_AT_MAX: &str = "Your job level is already the highest your job allows.";
const MSG_JOB_AT_MIN: &str = "Your job level is already 1.";
const MSG_NO_JOB_DATA: &str = "There is no job data for your job.";
const MSG_SAME_JOB: &str = "You already have that job.";

/// `atoi` of the first word: `None` for nothing, a non-number or 0.
fn level_argument(params: &str) -> Option<i32> {
    params
        .split_whitespace()
        .next()
        .and_then(|word| word.parse::<i32>().ok())
        .filter(|&n| n != 0)
}

fn chat_line(text: &str) -> ServerEvent {
    ServerEvent::SelfChat {
        text: text.as_bytes().to_vec(),
    }
}

fn stats_mut(status: &mut CharacterStatus) -> [&mut u16; 6] {
    [
        &mut status.str,
        &mut status.agi,
        &mut status.vit,
        &mut status.int,
        &mut status.dex,
        &mut status.luk,
    ]
}

/// Status or skill points, capped at what their `smallint` columns hold so a save is never
/// refused (Finish review m-2).
fn points(value: u32) -> u16 {
    const MAX_POINTS: u16 = i16::MAX.unsigned_abs();
    u16::try_from(value).map_or(MAX_POINTS, |v| v.min(MAX_POINTS))
}

impl MapInstance {
    /// `@blvl <n>` (spec §2.5).
    pub(crate) fn command_base_level(&mut self, e: Entity, params: &str) {
        let Some(n) = level_argument(params) else {
            return self.tell(e, MSG_BLVL_USAGE);
        };
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let (job_id, level) = (info.job, info.clevel);
        let Some(max) = self.jobs.get(job_id).map(|job| job.max_base_level) else {
            return self.tell(e, MSG_NO_JOB_DATA);
        };
        if n > 0 {
            if level >= max {
                return self.tell(e, MSG_BASE_AT_MAX);
            }
            let up = u16::try_from(n).unwrap_or(u16::MAX).min(max - level);
            self.raise_base_level(e, level, level + up);
        } else {
            if level <= 1 {
                return self.tell(e, MSG_BASE_AT_MIN);
            }
            let down = u16::try_from(n.unsigned_abs())
                .unwrap_or(u16::MAX)
                .min(level - 1);
            self.lower_base_level(e, level, level - down);
        }
    }

    /// The `statpoint` gains from `low` to `high`, step by step (a step never takes points
    /// away).
    fn status_points_between(&self, low: u16, high: u16) -> u32 {
        (low..high)
            .map(|l| {
                self.jobs
                    .status_points(l + 1)
                    .saturating_sub(self.jobs.status_points(l))
            })
            .sum()
    }

    /// Level up: the recalc, then the heal, the effect, the message, the tail and the `elvmax`
    /// check, all queued (capture 1 lines 401-423, capture 3 lines 825-881).
    fn raise_base_level(&mut self, e: Entity, from: u16, to: u16) {
        let gain = self.status_points_between(from, to);
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.clevel = to;
        info.status.base_exp = 0;
        info.status.status_point = points(u32::from(info.status.status_point) + gain);
        let status_point = info.status.status_point;
        let gid = self.gid(e);
        self.mark_dirty(e, false);
        self.queue_followup(e, FollowUp::HealToMax);
        self.queue_followup(
            e,
            FollowUp::ToArea(ServerEvent::SpecialEffect {
                gid,
                effect: EFFECT_BASE_LEVEL_UP,
            }),
        );
        let message = format!("Your base level went up to {to}.");
        self.queue_followup(e, FollowUp::ToSelf(chat_line(&message)));
        for event in base_tail(status_point, to) {
            self.queue_followup(e, FollowUp::ToSelf(event));
        }
        self.queue_followup(e, FollowUp::LevelGearCheck);
    }

    /// Level down: the reset if the unspent points can't cover the removal, the message, then
    /// the recalc (clamps and diff) and the tail. No heal, no effect (capture 1 lines 679-703,
    /// capture 2 lines 464-527).
    fn lower_base_level(&mut self, e: Entity, from: u16, to: u16) {
        let remove = self.status_points_between(to, from);
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        if u32::from(info.status.status_point) < remove {
            let reset_to = self.jobs.status_points(from);
            self.reset_stats(e, reset_to, true);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.status_point =
            points(u32::from(info.status.status_point).saturating_sub(remove));
        info.clevel = to;
        info.status.base_exp = 0;
        let status_point = info.status.status_point;
        self.tell(e, &format!("Your base level went down to {to}."));
        self.mark_dirty(e, false);
        for event in base_tail(status_point, to) {
            self.queue_followup(e, FollowUp::ToSelf(event));
        }
    }

    /// rAthena `pc_resetstate`: every stat back to 1 and `status_point` to `points`. With
    /// `push`, the costs (stats, then traits), `SP_STATUSPOINT` and `SP_TRAITPOINT` go out at
    /// once; the stats' `0x0141`s come from the tick's diff.
    fn reset_stats(&mut self, e: Entity, status_point: u32, push: bool) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        for stat in stats_mut(&mut info.status) {
            *stat = 1;
        }
        info.status.status_point = points(status_point);
        let cap = self.battle.max_stat(&class_flags(job_mapid(info.job)));
        let status_point = info.status.status_point;
        if !push {
            return;
        }
        let cost = next_stat_cost(1, cap);
        for offset in 0..STAT_COUNT {
            self.emit_self(
                e,
                ServerEvent::StatusCost {
                    var_id: SP_USTR + offset,
                    value: cost,
                },
            );
        }
        for offset in 0..STAT_COUNT {
            self.emit_self(
                e,
                ServerEvent::StatusCost {
                    var_id: SP_UPOW + offset,
                    value: 0,
                },
            );
        }
        self.emit_self(e, param(SP_STATUSPOINT, status_point.into()));
        self.emit_self(e, param(SP_TRAITPOINT, 0));
    }

    /// `@jlvl <n>` (spec §2.5): everything goes out at once, then the tick's diff. Up sends the
    /// effect to the area first; down sends no effect. There are no skills to reset on a
    /// decrease.
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
        self.send_job_level(e, to);
        self.emit_self(e, param(SP_SKILLPOINT, skill_point.into()));
        self.mark_dirty(e, false);
    }

    /// `SP_JOBLEVEL`, `SP_JOBEXP` (0) and `SP_NEXTJOBEXP`.
    fn send_job_level(&mut self, e: Entity, level: u16) {
        self.emit_self(e, param(SP_JOBLEVEL, level.into()));
        self.emit_self(
            e,
            ServerEvent::LongLongParameterChanged {
                var_id: SP_JOBEXP,
                value: 0,
            },
        );
        self.emit_self(
            e,
            ServerEvent::LongLongParameterChanged {
                var_id: SP_NEXTJOBEXP,
                value: PLACEHOLDER_NEXT_JOB_EXP,
            },
        );
    }

    /// The job `@job`'s argument names: an id, else a case-insensitive `JobDb` name.
    fn find_job(&self, params: &str) -> Option<&Job> {
        let word = params.trim();
        match word.parse::<u16>() {
            Ok(id) => self.jobs.get(id),
            Err(_) => self.jobs.by_name(word),
        }
    }

    /// `@job <id|name>` (spec §2.6): the minimal `pc_jobchange` step set.
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

        // Steps 1-3: the job, job level 1, and the base level cap with its reset and tail.
        let capped = level > max_base;
        let reset_points = self.jobs.status_points(max_base);
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.job = new_job;
        info.status.job_level = 1;
        info.status.job_exp = 0;
        if capped {
            info.clevel = max_base;
            info.status.base_exp = 0;
            self.reset_stats(e, reset_points, false);
            let status_point = self
                .world
                .players
                .get(e)
                .map_or(0, |info| info.status.status_point);
            for event in base_tail(status_point, max_base) {
                self.emit_self(e, event);
            }
        }

        // Step 4: the stat costs against the new cap. Step 5: the job level.
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let cap = self.battle.max_stat(&class_flags(job_mapid(new_job)));
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
        self.send_job_level(e, 1);

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
        // Step 8: the skill list again. Step 9: the tick's recalc.
        self.emit_self(
            e,
            ServerEvent::SkillList {
                skills: vec![nv_basic()],
            },
        );
        self.mark_dirty(e, false);

        // Steps 10-12, after the recalc: the equip looks, the message, the save.
        self.queue_followup(e, FollowUp::EquipLooks);
        let message = format!("Your job is now {name}.");
        self.queue_followup(e, FollowUp::ToSelf(chat_line(&message)));
        self.queue_followup(e, FollowUp::Save);
    }
}

/// The base tail of `@blvl` and a capped `@job` (spec §2.5). Trait points are always 0 here.
fn base_tail(status_point: u16, level: u16) -> [ServerEvent; 5] {
    [
        param(SP_STATUSPOINT, status_point.into()),
        param(SP_TRAITPOINT, 0),
        param(SP_BASELEVEL, level.into()),
        ServerEvent::LongLongParameterChanged {
            var_id: SP_BASEEXP,
            value: 0,
        },
        ServerEvent::LongLongParameterChanged {
            var_id: SP_NEXTBASEEXP,
            value: PLACEHOLDER_NEXT_BASE_EXP,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_argument_is_atoi_of_the_first_word() {
        assert_eq!(level_argument("10"), Some(10));
        assert_eq!(level_argument(" -5 extra"), Some(-5));
        for bad in ["", "0", "abc", "+"] {
            assert_eq!(level_argument(bad), None, "{bad:?}");
        }
    }

    /// Finish review m-2: points never outgrow their `smallint` columns.
    #[test]
    fn points_cap_at_the_column_range() {
        assert_eq!(points(48), 48);
        assert_eq!(points(32_767), 32_767);
        assert_eq!(points(32_768), 32_767);
        assert_eq!(points(u32::MAX), 32_767);
    }
}
