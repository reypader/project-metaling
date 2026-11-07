//! `@streset`: give back every allocated stat point.

use ro_core::{CharacterStatus, ServerEvent};
use ro_mechanics::next_stat_cost;

use super::msg_consts::MSG_STATS_RESET;
use super::points::gets_transcendent_points;
use crate::ecs::Entity;
use crate::job::{class_flags, job_mapid};
use crate::map_instance::{chat_line, MapInstance};
use crate::progression::points;
use crate::protocol_consts::{SP_STATUSPOINT, SP_TRAITPOINT, SP_USTR, STAT_COUNT};
use crate::stats::{param, trait_costs, FollowUp};

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

impl MapInstance {
    /// `@streset` / `@resetstat` (traits spec §2.8): the reset, then the diff, then our message.
    pub(crate) fn command_reset_stats(&mut self, e: Entity) {
        self.reset_stats(e);
        self.queue_followup(e, FollowUp::ToSelf(chat_line(MSG_STATS_RESET)));
    }

    /// rAthena `pc_resetstate` (traits spec §2.6), for the class and base level in effect now:
    /// STR..LUK to 1, POW..CRT to 0, `status_point` to the table (+52 for an upper or primary
    /// 4th class) and `trait_point` to the trait table (+7 for a trait job). The costs (stats,
    /// then traits), `SP_STATUSPOINT` and `SP_TRAITPOINT` go out at once; the `0x0141`s and the
    /// derived values come from the tick's diff (rAthena's stale `0x0141` ×12 is skipped).
    pub(crate) fn reset_stats(&mut self, e: Entity) {
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let (class, level) = (class_flags(job_mapid(info.job)), info.clevel);
        let mut status_point = self.jobs.status_points(level);
        if gets_transcendent_points(&class) {
            status_point += self.battle.transcendent_status_points;
        }
        let mut trait_point = self.jobs.trait_points(level);
        if class.trait_job {
            trait_point += self.battle.trait_points_job_change;
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        for stat in stats_mut(&mut info.status) {
            *stat = 1;
        }
        info.status.traits = [0; 6];
        info.status.status_point = points(status_point);
        info.status.trait_point = points(trait_point);
        let (status_point, trait_point, traits) = (
            info.status.status_point,
            info.status.trait_point,
            info.status.traits,
        );
        let cap = self.battle.max_stat(&class);
        let trait_cap = self.battle.max_trait(&class);
        self.mark_dirty(e, false);
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
        for event in trait_costs(&traits, trait_cap) {
            self.emit_self(e, event);
        }
        self.emit_self(e, param(SP_STATUSPOINT, status_point.into()));
        self.emit_self(e, param(SP_TRAITPOINT, trait_point.into()));
    }
}
