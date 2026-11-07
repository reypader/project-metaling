//! Status and trait point pools: totals between levels, grants and takes, the trait pool push.

use ro_mechanics::ClassFlags;

use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::progression::points;
use crate::protocol_consts::{SP_STATUSPOINT, SP_TRAITPOINT};
use crate::stats::{param, trait_costs};

/// Upper or primary 4th: the classes `transcendent_status_points` (52) goes with.
pub(super) fn gets_transcendent_points(class: &ClassFlags) -> bool {
    class.upper || class.fourth
}

impl MapInstance {
    /// The `statpoint` gains from `low` to `high`, step by step (a step never takes points
    /// away).
    pub(crate) fn status_points_between(&self, low: u16, high: u16) -> u32 {
        (low..high)
            .map(|l| {
                self.jobs
                    .status_points(l + 1)
                    .saturating_sub(self.jobs.status_points(l))
            })
            .sum()
    }

    /// The `TraitPoints` gain from `low` to `high`. The table is cumulative and never falls.
    pub(crate) fn trait_points_between(&self, low: u16, high: u16) -> u32 {
        self.jobs
            .trait_points(high)
            .saturating_sub(self.jobs.trait_points(low))
    }

    /// `@job` into an upper or primary 4th class: `status_point += n`, pushed.
    pub(crate) fn add_status_points(&mut self, e: Entity, n: u32) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.status_point = points(u32::from(info.status.status_point) + n);
        let status_point = info.status.status_point;
        self.emit_self(e, param(SP_STATUSPOINT, status_point.into()));
    }

    /// `@job` out of an upper or primary 4th class: `status_point -= n`, pushed, or a reset
    /// when fewer than `n` are left.
    pub(crate) fn take_status_points(&mut self, e: Entity, n: u32) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let Some(left) = u32::from(info.status.status_point).checked_sub(n) else {
            return self.reset_stats(e);
        };
        info.status.status_point = points(left);
        let status_point = info.status.status_point;
        self.emit_self(e, param(SP_STATUSPOINT, status_point.into()));
    }

    /// `@job` into a trait job: `trait_point += 7`, then the pool and the trait costs.
    pub(crate) fn grant_job_trait_points(&mut self, e: Entity, class: &ClassFlags) {
        let grant = self.battle.trait_points_job_change;
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.trait_point = points(u32::from(info.status.trait_point) + grant);
        self.send_trait_pool(e, class);
    }

    /// `@job` out of a trait job: with fewer than 7 unspent, a reset (with the new class, so the
    /// pool is normally 0, which is why a level-cap reset before it always leads here: the
    /// captured double reset). Otherwise the pool goes to 0, and the allocated traits stay,
    /// still counting with a cap of 0 (as rAthena).
    pub(crate) fn take_job_trait_points(&mut self, e: Entity, class: &ClassFlags) {
        let needed = self.battle.trait_points_job_change;
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if u32::from(info.status.trait_point) < needed {
            return self.reset_stats(e);
        }
        info.status.trait_point = 0;
        self.send_trait_pool(e, class);
    }

    /// `SP_TRAITPOINT`, then `0x00be` `SP_UPOW..SP_UCRT` against `class`'s trait cap.
    fn send_trait_pool(&mut self, e: Entity, class: &ClassFlags) {
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let (trait_point, traits) = (info.status.trait_point, info.status.traits);
        let cap = self.battle.max_trait(class);
        self.emit_self(e, param(SP_TRAITPOINT, trait_point.into()));
        for event in trait_costs(&traits, cap) {
            self.emit_self(e, event);
        }
    }
}
