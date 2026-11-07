//! The stat pass: dirty marking, the recalc of listed players and their follow-ups.

use std::mem;

use ro_core::{CharId, CharacterProgress, ServerEvent};
use ro_mechanics::{calc_status, BonusSlot, BonusTotals, Primary, Status, StatusInputs, Traits};
use tracing::warn;

use crate::ecs::Entity;
use crate::job::{class_flags, job_mapid, super_novice_bonus};
use crate::map_instance::MapInstance;
use crate::protocol_consts::{SP_AP, SP_HP, SP_MAXWEIGHT, SP_SP};
use crate::stats::recalc::{gear_input, job_row};
use crate::stats::{
    allocated_stats, battle_stats, battle_traits, couple, status_diff, FollowUp, Persisted, DIRTY,
    LISTED, SKILLS, STALE_GEAR,
};

/// The Super Novice rule's bonus to each primary stat.
const SUPER_NOVICE_STATS: i32 = 10;
const PRIMARY_SLOTS: [BonusSlot; 6] = [
    BonusSlot::Str,
    BonusSlot::Agi,
    BonusSlot::Vit,
    BonusSlot::Int,
    BonusSlot::Dex,
    BonusSlot::Luk,
];

impl MapInstance {
    /// The character's job, levels, stats, points, exp and HP/SP (spec §2.7): what a save
    /// writes besides the inventory. Cheap: no inventory clone.
    pub fn progress_snapshot(&self, char_id: CharId) -> Option<CharacterProgress> {
        let e = self.world.entity_by_char(char_id)?;
        self.world.players.get(e).map(|info| CharacterProgress {
            job_class: info.job,
            base_level: info.clevel,
            status: info.status,
            change_level: info.change_level,
        })
    }

    /// A change that can move `e`'s stats: the tick's pass recalculates it once. `gear` also
    /// rebuilds the cached gear status (equip changes).
    pub(crate) fn mark_dirty(&mut self, e: Entity, gear: bool) {
        let flags = if gear { DIRTY | STALE_GEAR } else { DIRTY };
        self.list_for_pass(e, flags);
    }

    /// A change that can move `e`'s visible skills (levels, skill levels): the tick's pass
    /// re-checks the tree once (rAthena runs it inside `status_calc_pc`).
    pub(crate) fn mark_skills_dirty(&mut self, e: Entity) {
        self.list_for_pass(e, SKILLS);
    }

    /// Queues `followup` for after `e`'s recalc in this tick's pass.
    pub(crate) fn queue_followup(&mut self, e: Entity, followup: FollowUp) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.followups.push(followup);
        self.list_for_pass(e, 0);
    }

    fn list_for_pass(&mut self, e: Entity, flags: u8) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if info.stat_flags & LISTED == 0 {
            self.stat_queue.push(e);
        }
        info.stat_flags |= flags | LISTED;
    }

    /// `e`'s status from its current data and the cached gear status. `None` for a non-player.
    fn compute_status(&self, e: Entity) -> Option<Status> {
        let info = self.world.players.get(e)?;
        let job = self.jobs.get(info.job);
        let status = &info.status;
        let allocated = Primary {
            str: status.str.into(),
            agi: status.agi.into(),
            vit: status.vit.into(),
            int: status.int.into(),
            dex: status.dex.into(),
            luk: status.luk.into(),
        };
        let [pow, sta, wis, spl, con, crt] = status.traits.map(i32::from);
        let traits = Traits {
            pow,
            sta,
            wis,
            spl,
            con,
            crt,
        };
        let gear = gear_input(job, &info.gear);
        let mapid = job_mapid(info.job);
        // The base layer's only source until item bonuses and passives: the class extra.
        let mut base_bonus = BonusTotals::default();
        if super_novice_bonus(mapid, status.job_level) {
            for slot in PRIMARY_SLOTS {
                base_bonus.add(slot, SUPER_NOVICE_STATS);
            }
        }
        let none = BonusTotals::default();
        let inputs = StatusInputs {
            allocated,
            traits,
            base_level: info.clevel,
            job: job_row(job, status.job_level, info.clevel),
            class: class_flags(mapid),
            gear: &gear,
            base_bonus: &base_bonus,
            battle_bonus: &none,
        };
        let mut computed = calc_status(&inputs, &self.battle);
        if job.is_none() {
            // No HP/SP table: keep the saved maxima.
            computed.max_hp = status.max_hp;
            computed.max_sp = status.max_sp;
        }
        Some(computed)
    }

    fn rebuild_gear(&mut self, e: Entity) {
        let gear = self
            .world
            .inventories
            .get(e)
            .map(|inventory| inventory.gear(&self.items).gear_status(&self.refine))
            .unwrap_or_default();
        if let Some(info) = self.world.players.get_mut(e) {
            info.gear = gear;
        }
    }

    /// Spawn and cross-map arrival: compute and store the status with nothing to diff against
    /// (spec §2.1). HP/SP/AP above the new maxima are clamped silently; the login push sends
    /// them.
    pub(crate) fn init_status(&mut self, e: Entity) {
        self.rebuild_gear(e);
        let Some(computed) = self.compute_status(e) else {
            return;
        };
        if let Some(inventory) = self.world.inventories.get_mut(e) {
            inventory.set_max_weight(computed.max_weight);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if self.jobs.get(info.job).is_none() {
            warn!(
                map = %self.map_name,
                char_id = ?info.char_id,
                job = info.job,
                "no job data: saved MaxHP/MaxSP kept, default MaxWeight and ASPD"
            );
        }
        let status = &mut info.status;
        status.max_hp = computed.max_hp;
        status.max_sp = computed.max_sp;
        status.hp = status.hp.min(computed.max_hp);
        status.sp = status.sp.min(computed.max_sp);
        status.ap = status.ap.min(computed.max_ap);
        info.allocated = allocated_stats(status);
        info.derived = computed;
    }

    /// Stages 3 and 4: every listed player in marking order gets its recalc (if dirty) and its
    /// follow-ups. Players a follow-up lists again wait for the next tick.
    pub(crate) fn run_stat_pass(&mut self) {
        let mut queue = mem::take(&mut self.stat_queue);
        for &e in &queue {
            self.settle_stats(e);
        }
        queue.clear();
        queue.append(&mut self.stat_queue);
        self.stat_queue = queue;
    }

    /// Runs what the pass owes `e` now: the recalc, then the follow-ups. Also used right before
    /// a cross-map warp takes the player away. Nothing for a player that already left.
    pub(crate) fn settle_stats(&mut self, e: Entity) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let flags = mem::take(&mut info.stat_flags);
        if flags & DIRTY != 0 {
            self.recalc(e, flags & STALE_GEAR != 0, flags & SKILLS != 0);
        } else if flags & SKILLS != 0 {
            self.send_tree_diff(e);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let mut followups = mem::take(&mut info.followups);
        for followup in followups.drain(..) {
            self.run_followup(e, followup);
        }
        if let Some(info) = self.world.players.get_mut(e) {
            // Keep the capacity; anything queued meanwhile stays for the next pass.
            followups.append(&mut info.followups);
            info.followups = followups;
        }
    }

    /// rAthena `status_calc_pc` at tick end: recompute, push `SP_MAXWEIGHT`, the tree diff (when
    /// `skills`; skills spec §2.6), the clamps and the diff in the captured order (spec §2.3),
    /// store. The AP clamp (traits spec §2.3) is pushed with the HP/SP ones.
    fn recalc(&mut self, e: Entity, stale_gear: bool, skills: bool) {
        if stale_gear {
            self.rebuild_gear(e);
        }
        let Some(new) = self.compute_status(e) else {
            return;
        };
        let max_weight_changed = self.world.inventories.get_mut(e).is_some_and(|inventory| {
            let changed = inventory.max_weight() != new.max_weight;
            inventory.set_max_weight(new.max_weight);
            changed
        });
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let old = mem::replace(&mut info.derived, new);
        let status = &mut info.status;
        let (old_hp, old_sp, old_ap) = (status.hp, status.sp, status.ap);
        status.max_hp = new.max_hp;
        status.max_sp = new.max_sp;
        status.hp = status.hp.min(new.max_hp);
        status.sp = status.sp.min(new.max_sp);
        status.ap = status.ap.min(new.max_ap);
        let now = Persisted {
            allocated: allocated_stats(status),
            hp: status.hp,
            sp: status.sp,
            ap: status.ap,
        };
        let before = Persisted {
            allocated: mem::replace(&mut info.allocated, now.allocated),
            hp: old_hp,
            sp: old_sp,
            ap: old_ap,
        };

        if max_weight_changed {
            self.emit_weight(e, SP_MAXWEIGHT, new.max_weight);
        }
        if skills {
            self.send_tree_diff(e);
        }
        for (var_id, old_value, new_value) in [
            (SP_HP, before.hp, now.hp),
            (SP_SP, before.sp, now.sp),
            (SP_AP, before.ap, now.ap),
        ] {
            if new_value != old_value {
                self.emit_param(e, var_id, new_value);
            }
        }
        for event in status_diff(&old, &new, (&before, &now)) {
            self.emit_self(e, event);
        }
    }

    fn run_followup(&mut self, e: Entity, followup: FollowUp) {
        match followup {
            FollowUp::ToSelf(event) => self.emit_self(e, event),
            FollowUp::ToArea(event) => self.emit_aoi(e, false, event),
            FollowUp::CoupleStat(index) => {
                let Some(info) = self.world.players.get(e) else {
                    return;
                };
                let i = usize::from(index);
                let mut battle = battle_stats(&info.derived)
                    .into_iter()
                    .chain(battle_traits(&info.derived));
                let (Some(&allocated), Some(battle)) = (info.allocated.get(i), battle.nth(i))
                else {
                    return;
                };
                self.emit_self(e, couple(index, allocated, battle));
            }
            FollowUp::EquipLooks => {
                for event in self.equip_look_events(e) {
                    self.emit_aoi(e, false, event);
                }
            }
            FollowUp::HealToMax => {
                // rAthena `status_percent_heal(100, 100)`: nothing for the dead; each part is
                // pushed only if it was below its max.
                let Some(status) = self.world.players.get_mut(e).map(|info| &mut info.status)
                else {
                    return;
                };
                if status.hp == 0 {
                    return;
                }
                let (hp_raised, sp_raised) = (status.hp < status.max_hp, status.sp < status.max_sp);
                status.hp = status.max_hp;
                status.sp = status.max_sp;
                let (hp, sp) = (status.hp, status.sp);
                if hp_raised {
                    self.emit_param(e, SP_HP, hp);
                }
                if sp_raised {
                    self.emit_param(e, SP_SP, sp);
                }
            }
            // The unequips mark the player dirty again: the diff they cause waits for the next
            // tick's pass.
            FollowUp::LevelGearCheck => self.unequip_over_level(e),
            FollowUp::Live(push) => {
                for event in self.live_push_events(e, push) {
                    self.emit_self(e, event);
                }
            }
            FollowUp::Save => {
                if let Some(info) = self.world.players.get(e) {
                    self.save_requests.push(info.char_id);
                }
            }
        }
    }

    fn emit_param(&mut self, e: Entity, var_id: u16, value: u32) {
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id,
                value: i32::try_from(value).unwrap_or(i32::MAX),
            },
        );
    }
}
