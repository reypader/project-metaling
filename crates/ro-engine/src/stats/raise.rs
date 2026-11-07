//! Stat-up, `0x00bb` (spec `specs/stats/spec.md` §2.4; catalog `packets/status.md` "Stat-up
//! request and acknowledgement").
//!
//! A raise changes the allocated stat and the points at once and marks the player dirty. The
//! tick's recalc pushes the diff, and then the queued `0x00be`, `SP_STATUSPOINT` and ack go out,
//! in `pc_statusup`'s order. A failure sends only the `ok = 0` ack, at once.
//!
//! Trait-up, `0x0b24` (spec `specs/traits/spec.md` §2.4), is the same flow over POW..CRT and the
//! trait-point pool, one point per level.

use ro_core::{CharacterStatus, ServerEvent};
use ro_mechanics::{next_stat_cost, next_trait_cost, raise_stat, raise_trait};

use crate::ecs::Entity;
use crate::job::{class_flags, job_mapid};
use crate::map_instance::MapInstance;
use crate::protocol_consts::{
    SP_CRT, SP_LUK, SP_POW, SP_STATUSPOINT, SP_STR, SP_TRAITPOINT, SP_UPOW, SP_USTR,
};
use crate::stats::{param, FollowUp};

/// The ack carries the stat in one byte; a higher stat gets a trailing `0x0141`.
const ACK_MAX_VALUE: u16 = 255;
/// POW's index among the 12 allocated stats (`FollowUp::CoupleStat`).
const TRAIT_INDEX: u16 = 6;

/// The allocated stat at STR..LUK `offset`.
fn stat_mut(status: &mut CharacterStatus, offset: u16) -> Option<&mut u16> {
    match offset {
        0 => Some(&mut status.str),
        1 => Some(&mut status.agi),
        2 => Some(&mut status.vit),
        3 => Some(&mut status.int),
        4 => Some(&mut status.dex),
        5 => Some(&mut status.luk),
        _ => None,
    }
}

impl MapInstance {
    /// `CZ_STATUS_CHANGE`: raise stat `var` by up to `amount`, as far as the points and the
    /// job's cap allow. Refused while dead (our ruling; rAthena doesn't check), not while
    /// sitting.
    pub(crate) fn raise_stat(&mut self, e: Entity, var: u16, amount: u8) {
        if !(SP_STR..=SP_LUK).contains(&var) || amount == 0 || self.is_dead(e) {
            return self.refuse_raise(e, var);
        }
        let offset = var - SP_STR;
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let cap = self.battle.max_stat(&class_flags(job_mapid(info.job)));
        let points = info.status.status_point;
        let Some(stat) = stat_mut(&mut info.status, offset) else {
            return;
        };
        let raise = raise_stat(*stat, amount.into(), points.into(), cap);
        let Ok(cost) = u16::try_from(raise.cost) else {
            return self.refuse_raise(e, var);
        };
        if raise.amount == 0 {
            return self.refuse_raise(e, var);
        }
        *stat += raise.amount;
        let new = *stat;
        // `raise_stat` never spends more than `points`.
        info.status.status_point = points.saturating_sub(cost);
        let points = info.status.status_point;

        self.mark_dirty(e, false);
        for event in [
            ServerEvent::StatusCost {
                var_id: SP_USTR + offset,
                value: next_stat_cost(new, cap),
            },
            param(SP_STATUSPOINT, points.into()),
            ServerEvent::StatusChangeAck {
                var_id: var,
                ok: true,
                value: u8::try_from(new.min(ACK_MAX_VALUE)).unwrap_or(u8::MAX),
            },
        ] {
            self.queue_followup(e, FollowUp::ToSelf(event));
        }
        if new > ACK_MAX_VALUE {
            self.queue_followup(e, FollowUp::CoupleStat(offset));
        }
    }

    /// `CZ_ADVANCED_STATUS_CHANGE`: raise trait `var` by up to `amount`, capped by the trait
    /// points and the job's trait cap (0 off a trait job). A negative amount is dropped with no
    /// reply, as rAthena does; dead is refused, as for stat-up.
    pub(crate) fn raise_trait(&mut self, e: Entity, var: u16, amount: i16) {
        let Ok(amount) = u16::try_from(amount) else {
            return;
        };
        if !(SP_POW..=SP_CRT).contains(&var) || amount == 0 || self.is_dead(e) {
            return self.refuse_raise(e, var);
        }
        let offset = var - SP_POW;
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let cap = self.battle.max_trait(&class_flags(job_mapid(info.job)));
        let points = info.status.trait_point;
        let Some(value) = info.status.traits.get_mut(usize::from(offset)) else {
            return;
        };
        let raise = raise_trait(*value, amount, points.into(), cap);
        if raise.amount == 0 {
            return self.refuse_raise(e, var);
        }
        *value += raise.amount;
        let new = *value;
        // One point per level, so the cost is the amount, which never exceeds `points`.
        info.status.trait_point = points.saturating_sub(raise.amount);
        let points = info.status.trait_point;

        self.mark_dirty(e, false);
        for event in [
            ServerEvent::StatusCost {
                var_id: SP_UPOW + offset,
                value: next_trait_cost(new, cap),
            },
            param(SP_TRAITPOINT, points.into()),
            ServerEvent::StatusChangeAck {
                var_id: var,
                ok: true,
                value: u8::try_from(new.min(ACK_MAX_VALUE)).unwrap_or(u8::MAX),
            },
        ] {
            self.queue_followup(e, FollowUp::ToSelf(event));
        }
        // Only reachable with `max_trait_parameter` raised past 255 (shipped 110).
        if new > ACK_MAX_VALUE {
            self.queue_followup(e, FollowUp::CoupleStat(TRAIT_INDEX + offset));
        }
    }

    fn refuse_raise(&mut self, e: Entity, var: u16) {
        self.emit_self(
            e,
            ServerEvent::StatusChangeAck {
                var_id: var,
                ok: false,
                value: 0,
            },
        );
    }
}

#[cfg(test)]
mod tests;
