//! `@blvl`: raise or lower the base level.

use ro_core::ServerEvent;

use super::args::level_argument;
use super::msg_consts::{MSG_BASE_AT_MAX, MSG_BASE_AT_MIN, MSG_BLVL_USAGE, MSG_NO_JOB_DATA};
use crate::ecs::Entity;
use crate::map_instance::{chat_line, MapInstance};
use crate::progression::{points, LivePush};
use crate::protocol_consts::{
    EFFECT_BASE_LEVEL_UP, SP_BASEEXP, SP_BASELEVEL, SP_NEXTBASEEXP, SP_STATUSPOINT, SP_TRAITPOINT,
};
use crate::stats::{exp, param, FollowUp};

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

    /// Level up: the recalc, then the heal, the effect, the message, the tail and the `elvmax`
    /// check, all queued (capture 1 lines 401-423, capture 3 lines 825-881).
    fn raise_base_level(&mut self, e: Entity, from: u16, to: u16) {
        let gain = self.status_points_between(from, to);
        let trait_gain = self.trait_points_between(from, to);
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.clevel = to;
        info.status.base_exp = 0;
        info.status.status_point = points(u32::from(info.status.status_point) + gain);
        info.status.trait_point = points(u32::from(info.status.trait_point) + trait_gain);
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
        let message = format!("Your base level went up to {to}.");
        self.queue_followup(e, FollowUp::ToSelf(chat_line(&message)));
        self.queue_followup(e, FollowUp::Live(LivePush::BaseTail));
        self.queue_followup(e, FollowUp::LevelGearCheck);
    }

    /// Level down: the reset (at the old level) if either unspent pool can't cover its removal,
    /// the message, then the recalc (clamps and diff) and the tail. No heal, no effect (capture
    /// 1 lines 679-703, capture 2 lines 464-527; traits spec §2.5, captured 720-782).
    fn lower_base_level(&mut self, e: Entity, from: u16, to: u16) {
        let remove = self.status_points_between(to, from);
        let remove_trait = self.trait_points_between(to, from);
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        if u32::from(info.status.status_point) < remove
            || u32::from(info.status.trait_point) < remove_trait
        {
            self.reset_stats(e);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.status_point =
            points(u32::from(info.status.status_point).saturating_sub(remove));
        info.status.trait_point =
            points(u32::from(info.status.trait_point).saturating_sub(remove_trait));
        info.clevel = to;
        info.status.base_exp = 0;
        self.tell(e, &format!("Your base level went down to {to}."));
        self.mark_dirty(e, false);
        self.mark_skills_dirty(e);
        self.queue_followup(e, FollowUp::Live(LivePush::BaseTail));
    }

    /// The base tail (spec §2.5; combat spec §2.9) with the player's current values: the point
    /// totals, the level, the exp and the next bar.
    pub(crate) fn base_tail(&self, e: Entity) -> Vec<ServerEvent> {
        let Some(info) = self.world.players.get(e) else {
            return Vec::new();
        };
        let status = &info.status;
        vec![
            param(SP_STATUSPOINT, status.status_point.into()),
            param(SP_TRAITPOINT, status.trait_point.into()),
            param(SP_BASELEVEL, info.clevel.into()),
            ServerEvent::LongLongParameterChanged {
                var_id: SP_BASEEXP,
                value: exp(status.base_exp),
            },
            ServerEvent::LongLongParameterChanged {
                var_id: SP_NEXTBASEEXP,
                value: exp(self.jobs.next_base_exp(info.job, info.clevel)),
            },
        ]
    }
}
