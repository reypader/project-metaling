//! Mob death: rewards, drops and the delayed vanish.

use ro_core::ServerEvent;
use ro_mechanics::{exp_share, shared_exp};
use smallvec::SmallVec;
use tracing::warn;

use crate::combat::CombatTimer;
use crate::ecs::{Entity, AREA_SIZE};
use crate::map_instance::MapInstance;
use crate::protocol_consts::VANISH_DIED;

/// The death vanish follows the killing hit by this much (catalog `mobs.md` §3 step 11).
const DEATH_VANISH_DELAY: u64 = 250;

impl MapInstance {
    /// rAthena `mob_dead` without drops or respawn (spec §2.8): out of the grid at once, the exp
    /// for each valid log entry, and the vanish at `due + 250`.
    pub(crate) fn kill_mob(&mut self, target: Entity, due: u64) {
        let cell = self.cell(target);
        let Some(mob) = self.world.mobs.get_mut(target) else {
            return;
        };
        mob.dying = true;
        let log = std::mem::take(&mut mob.damage_log);
        let (base_exp, job_exp) = (mob.stats.base_exp, mob.stats.job_exp);
        self.world.hide(target);

        // Players on this map count as attackers; the dead among them get nothing; offline or
        // elsewhere neither counts nor gets anything, and nobody inherits their share.
        let total = log
            .entries()
            .iter()
            .fold(0u64, |sum, entry| sum.saturating_add(entry.damage));
        let mut attackers = 0;
        let mut paid: SmallVec<[(Entity, u64); 4]> = SmallVec::new();
        for entry in log.entries() {
            let Some(e) = self.world.entity_by_char(entry.char_id) else {
                continue;
            };
            attackers += 1;
            if !self.is_dead(e) {
                paid.push((e, entry.damage));
            }
        }
        for (e, damage) in paid {
            let share = exp_share(damage, total, attackers, &self.battle);
            self.gain_exp(e, shared_exp(base_exp, share), shared_exp(job_exp, share));
        }
        self.combat_timers.push(
            due + DEATH_VANISH_DELAY,
            CombatTimer::Vanish { mob: target, cell },
        );
    }

    /// The death vanish to the area of the death cell (the mob is out of the grid), then the
    /// despawn, which frees the gid.
    pub(crate) fn vanish_dead_mob(&mut self, mob: Entity, cell: (u16, u16)) {
        let Some(gid) = self.world.mobs.get(mob).map(|info| info.gid) else {
            return;
        };
        let chars = self.recipients_at(cell, AREA_SIZE);
        self.emit_to(
            chars,
            ServerEvent::UnitVanished {
                gid,
                vanish_type: VANISH_DIED,
            },
        );
        if let Err(err) = self.world.despawn(mob) {
            warn!(map = %self.map_name, %err, "dead mob despawn failed");
        }
    }
}
