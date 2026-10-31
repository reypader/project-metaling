//! Kill exp and natural level-ups (spec `specs/combat/spec.md` §2.9; catalog
//! `mechanics/experience.md` "pc_gainexp flow", "Natural base level-up" and "Natural job
//! level-up", checked against `combat_sequence_1.txt`).
//!
//! rAthena sends each level-up burst around its own recalc. Ours runs once at tick end, so the
//! first burst's pushes go out at once and everything after them is queued, in rAthena's order.
//! A job level-up in the same kill therefore shares the base burst's diff (spec §2.9). Queued
//! tails and exp pushes read the player's values when they are sent (`LivePush`), so a later
//! push of the same var in the tick is never overwritten by a stale one.

use ro_core::ServerEvent;
use ro_db::{NEXT_BASE_EXP_AT_MAX, NEXT_JOB_EXP_AT_MAX};

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::level_command::{points, EFFECT_BASE_LEVEL_UP, EFFECT_JOB_LEVEL_UP};
use crate::stats::FollowUp;
use crate::status::{exp, param, SP_BASEEXP, SP_JOBEXP, SP_SKILLPOINT};

/// rAthena `MAX_EXP`: exp totals saturate here.
const MAX_EXP: u64 = i64::MAX.unsigned_abs();

/// Pushes whose values are read when they are sent (a `FollowUp::Live`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LivePush {
    /// `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `SP_BASEEXP`, `SP_NEXTBASEEXP`.
    BaseTail,
    /// `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`.
    JobTail,
    /// `0x0acb` of `SP_BASEEXP` or `SP_JOBEXP`.
    Exp(u16),
}

/// rAthena's level-up loop (`pc_checkbaselevelup` / `pc_checkjoblevelup`): the level reached
/// and the exp left, or `None` when no level is gained. `bar(level)` is the exp from `level` to
/// the next; `clamps(level)` says whether the leftover is cut to one bar - 1 (`multi_level_up`
/// off); at `max` the exp stops at `at_max`.
fn level_up(
    mut level: u16,
    mut exp: u64,
    max: u16,
    at_max: u64,
    clamps: impl Fn(u16) -> bool,
    bar: impl Fn(u16) -> u64,
) -> Option<(u16, u64)> {
    let mut next = bar(level);
    if next == 0 || exp < next || level >= max {
        return None;
    }
    loop {
        exp -= next;
        if clamps(level) {
            exp = exp.min(next - 1);
        }
        level += 1;
        if level >= max {
            return Some((level, exp.min(at_max)));
        }
        next = bar(level);
        if next == 0 || exp < next {
            return Some((level, exp));
        }
    }
}

/// `current + gain`, saturating; at the max level the total stops at `cap` instead.
fn add_exp(current: u64, gain: u64, at_max: bool, cap: u64) -> u64 {
    let gain = if at_max {
        gain.min(cap.saturating_sub(current))
    } else {
        gain
    };
    current.saturating_add(gain).min(MAX_EXP)
}

impl MapInstance {
    /// rAthena `pc_gainexp` for a kill, solo and without bonuses (spec §2.9): base first, then
    /// job, each with its level-up burst or its exp push; then `0x0acc` for each non-zero gain
    /// (0 shown at the max level).
    pub(crate) fn gain_exp(&mut self, e: Entity, base: u64, job: u64) {
        if self.is_dead(e) {
            return;
        }
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let (max_base, max_job) = self
            .jobs
            .get(info.job)
            .map_or((0, 0), |data| (data.max_base_level, data.max_job_level));
        let base_at_max = info.clevel >= max_base;
        let job_at_max = info.status.job_level >= max_job;
        let gid = self.gid(e);
        // Once something is queued, the rest is queued too, so the order holds. A kill that finds
        // follow-ups already waiting (an earlier kill's level-up this tick) queues behind them.
        let mut queued = !info.followups.is_empty();

        if base > 0 {
            let Some(info) = self.world.players.get_mut(e) else {
                return;
            };
            let status = &mut info.status;
            status.base_exp = add_exp(status.base_exp, base, base_at_max, NEXT_BASE_EXP_AT_MAX);
            if self.base_level_up(e) {
                self.send_base_burst(e, queued);
                queued = true;
            } else {
                self.push_live(e, queued, LivePush::Exp(SP_BASEEXP));
            }
        }
        if job > 0 {
            let Some(info) = self.world.players.get_mut(e) else {
                return;
            };
            let status = &mut info.status;
            status.job_exp = add_exp(status.job_exp, job, job_at_max, NEXT_JOB_EXP_AT_MAX);
            if self.job_level_up(e) {
                self.send_job_burst(e, queued);
                queued = true;
            } else {
                self.push_live(e, queued, LivePush::Exp(SP_JOBEXP));
            }
        }
        for (gained, var_id, at_max) in [
            (base, SP_BASEEXP, base_at_max),
            (job, SP_JOBEXP, job_at_max),
        ] {
            if gained > 0 {
                let amount = if at_max { 0 } else { exp(gained) };
                let event = ServerEvent::ExpGained {
                    gid,
                    var_id,
                    amount,
                };
                self.push_exp_event(e, queued, event);
            }
        }
    }

    /// `event` to the player now, or after the tick's recalc once a level-up has queued. For
    /// the `0x0acc` amounts, which are this kill's own.
    fn push_exp_event(&mut self, e: Entity, queued: bool, event: ServerEvent) {
        if queued {
            self.queue_followup(e, FollowUp::ToSelf(event));
        } else {
            self.emit_self(e, event);
        }
    }

    /// `push` now, or queued to be read when it is sent.
    fn push_live(&mut self, e: Entity, queued: bool, push: LivePush) {
        if queued {
            self.queue_followup(e, FollowUp::Live(push));
        } else {
            for event in self.live_push_events(e, push) {
                self.emit_self(e, event);
            }
        }
    }

    /// `push`'s packets with the player's current values.
    pub(crate) fn live_push_events(&self, e: Entity, push: LivePush) -> Vec<ServerEvent> {
        match push {
            LivePush::BaseTail => self.base_tail(e),
            LivePush::JobTail => {
                let mut tail = self.job_level_tail(e);
                if let Some(info) = self.world.players.get(e) {
                    tail.push(param(SP_SKILLPOINT, info.status.skill_point.into()));
                }
                tail
            }
            LivePush::Exp(var_id) => vec![self.exp_push(e, var_id)],
        }
    }

    /// `0x0acb` with the player's current base or job exp.
    fn exp_push(&self, e: Entity, var_id: u16) -> ServerEvent {
        let status = self.world.players.get(e).map(|info| &info.status);
        let value = status.map_or(0, |s| {
            if var_id == SP_BASEEXP {
                s.base_exp
            } else {
                s.job_exp
            }
        });
        ServerEvent::LongLongParameterChanged {
            var_id,
            value: exp(value),
        }
    }

    /// Applies the base loop: level, leftover exp, status and trait points. `false` when no
    /// level is gained.
    fn base_level_up(&mut self, e: Entity) -> bool {
        let Some(info) = self.world.players.get(e) else {
            return false;
        };
        let (job, from, base_exp) = (info.job, info.clevel, info.status.base_exp);
        let max = self.jobs.get(job).map_or(0, |data| data.max_base_level);
        let (multi, multi_from) = (self.battle.multi_level_up, self.battle.multi_level_up_base);
        let clamps = |level: u16| !multi || (multi_from > 0 && u32::from(level) >= multi_from);
        let jobs = &self.jobs;
        let Some((to, left)) = level_up(from, base_exp, max, NEXT_BASE_EXP_AT_MAX, clamps, |l| {
            jobs.next_base_exp(job, l)
        }) else {
            return false;
        };
        let gain = self.status_points_between(from, to);
        let trait_gain = self.trait_points_between(from, to);
        let Some(info) = self.world.players.get_mut(e) else {
            return false;
        };
        info.clevel = to;
        info.status.base_exp = left;
        info.status.status_point = points(u32::from(info.status.status_point) + gain);
        info.status.trait_point = points(u32::from(info.status.trait_point) + trait_gain);
        true
    }

    /// Applies the job loop: level, leftover exp and one skill point per level. `false` when no
    /// level is gained.
    fn job_level_up(&mut self, e: Entity) -> bool {
        let Some(info) = self.world.players.get(e) else {
            return false;
        };
        let (job, from, job_exp) = (info.job, info.status.job_level, info.status.job_exp);
        let max = self.jobs.get(job).map_or(0, |data| data.max_job_level);
        let (multi, multi_from) = (self.battle.multi_level_up, self.battle.multi_level_up_job);
        let clamps = |level: u16| !multi || (multi_from > 0 && u32::from(level) >= multi_from);
        let jobs = &self.jobs;
        let Some((to, left)) = level_up(from, job_exp, max, NEXT_JOB_EXP_AT_MAX, clamps, |l| {
            jobs.next_job_exp(job, l)
        }) else {
            return false;
        };
        let Some(info) = self.world.players.get_mut(e) else {
            return false;
        };
        let status = &mut info.status;
        status.job_level = to;
        status.job_exp = left;
        status.skill_point = points(u32::from(status.skill_point) + u32::from(to - from));
        true
    }

    /// The natural base burst (captured): the tail (now, unless something queued first), then
    /// the recalc diff, the heal to full, `0x019b` effect 0 and the `elvmax` check. The tree is
    /// re-checked with the recalc.
    fn send_base_burst(&mut self, e: Entity, queued: bool) {
        self.push_live(e, queued, LivePush::BaseTail);
        let gid = self.gid(e);
        self.mark_dirty(e, false);
        self.mark_skills_dirty(e);
        self.queue_followup(e, FollowUp::HealToMax);
        self.queue_followup(
            e,
            FollowUp::ToArea(ServerEvent::SpecialEffect {
                gid,
                effect: EFFECT_BASE_LEVEL_UP,
            }),
        );
        self.queue_followup(e, FollowUp::LevelGearCheck);
    }

    /// The natural job burst (captured): `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP` and
    /// `SP_SKILLPOINT` (now, unless a base burst queued first), the recalc diff, then `0x019b`
    /// effect 1. No heal.
    fn send_job_burst(&mut self, e: Entity, queued: bool) {
        self.push_live(e, queued, LivePush::JobTail);
        let gid = self.gid(e);
        self.mark_dirty(e, false);
        self.mark_skills_dirty(e);
        self.queue_followup(
            e,
            FollowUp::ToArea(ServerEvent::SpecialEffect {
                gid,
                effect: EFFECT_JOB_LEVEL_UP,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Novice job bars 1..=4 (`job_exp.yml`), max job level 10.
    fn novice_job(level: u16) -> u64 {
        [10, 18, 28, 40, 91]
            .get(usize::from(level) - 1)
            .copied()
            .unwrap_or(0)
    }

    /// Captured clamp (`experience.md` "Job-exp leftover clamp"): 0 + 40 at job 1 leaves 9, not
    /// 30; 27 + 40 at job 4 leaves 27 (below the clamp).
    #[test]
    fn leftover_clamps_to_the_consumed_bar() {
        let clamp = |_| true;
        assert_eq!(level_up(1, 40, 10, 999, clamp, novice_job), Some((2, 9)));
        assert_eq!(level_up(4, 67, 10, 999, clamp, novice_job), Some((5, 27)));
        assert_eq!(
            level_up(1, 9, 10, 999, clamp, novice_job),
            None,
            "below the bar"
        );
    }

    #[test]
    fn multi_level_up_chains_and_stops_at_the_max() {
        let carry = |_| false;
        // 10 + 18 + 28 = 56: three levels, 4 left (the next bar is 40).
        assert_eq!(level_up(1, 60, 10, 999, carry, novice_job), Some((4, 4)));
        // A missing row (0) stops the loop.
        assert_eq!(
            level_up(1, 1_000, 10, 999, carry, novice_job),
            Some((6, 1_000 - 187))
        );
        assert_eq!(
            level_up(1, 1_000, 3, 5, carry, novice_job),
            Some((3, 5)),
            "max"
        );
        assert_eq!(
            level_up(3, 1_000, 3, 5, carry, novice_job),
            None,
            "already max"
        );
    }

    #[test]
    fn exp_stops_at_the_cap_only_at_the_max_level() {
        assert_eq!(add_exp(10, 40, false, 20), 50);
        assert_eq!(add_exp(10, 40, true, 20), 20);
        assert_eq!(add_exp(30, 40, true, 20), 30, "never lowered");
        assert_eq!(add_exp(MAX_EXP - 1, 40, false, 0), MAX_EXP);
    }
}
