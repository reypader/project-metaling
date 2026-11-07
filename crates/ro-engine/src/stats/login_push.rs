//! The status pushes sent at login and map change.

use ro_core::{CharacterStatus, ServerEvent};
use ro_mechanics::Status;

use crate::ecs::Entity;
use crate::job::{class_flags, job_mapid};
use crate::map_instance::MapInstance;
use crate::protocol_consts::{
    SP_AP, SP_ASPD, SP_ATK1, SP_ATK2, SP_BASEEXP, SP_CRITICAL, SP_DEF1, SP_DEF2, SP_FLEE1,
    SP_FLEE2, SP_HIT, SP_HP, SP_JOBEXP, SP_MATK1, SP_MATK2, SP_MAXAP, SP_MAXHP, SP_MAXSP,
    SP_MAXWEIGHT, SP_MDEF1, SP_MDEF2, SP_NEXTBASEEXP, SP_NEXTJOBEXP, SP_PATK, SP_SKILLPOINT, SP_SP,
    SP_TRAITPOINT, SP_WEIGHT, STAT_COUNT,
};
use crate::stats::status_push::{derived_value, stats, status_window, trait_couple, trait_values};
use crate::stats::{battle_stats, battle_traits, couple, exp, param, trait_costs};

/// The derived `0x00b0` vars of the pre-entry push, in captured order: HIT, FLEE1, ASPD, ATK1,
/// DEF1, DEF2, ATK2, FLEE2, CRITICAL, MATK2, MATK1, MDEF2, MDEF1. rAthena sends DEF2/DEF1 a
/// second time after ATK2 (two diff branches); once each is enough here.
const PUSH_DERIVED_PARAMS: [u16; 13] = [
    SP_HIT,
    SP_FLEE1,
    SP_ASPD,
    SP_ATK1,
    SP_DEF1,
    SP_DEF2,
    SP_ATK2,
    SP_FLEE2,
    SP_CRITICAL,
    SP_MATK2,
    SP_MATK1,
    SP_MDEF2,
    SP_MDEF1,
];

impl MapInstance {
    /// The status push a login gets before its first `0x007d` (rAthena's first `status_calc_pc`
    /// diffed against zero, captured in `packet_rathena_item_2026-10-24.txt` lines 44-66):
    /// MAXWEIGHT, STR..LUK, the derived values, attack range, MAXHP, MAXSP, HP, SP, WEIGHT.
    pub(crate) fn send_login_status_push(&mut self, e: Entity) {
        let Some((status, derived)) = self
            .world
            .players
            .get(e)
            .map(|info| (info.status, info.derived))
        else {
            return;
        };
        let (weight, max_weight) = self
            .world
            .inventories
            .get(e)
            .map_or((0, 0), |i| (i.weight(), i.max_weight()));
        self.emit_weight(e, SP_MAXWEIGHT, max_weight);
        self.send_couple_stats(e, &status, &derived);
        for var_id in PUSH_DERIVED_PARAMS {
            let value = derived_value(&derived, var_id);
            self.emit_self(e, ServerEvent::ParameterChanged { var_id, value });
        }
        self.emit_self(
            e,
            ServerEvent::AttackRange {
                range: derived.range,
            },
        );
        for (var_id, value) in [
            (SP_MAXHP, status.max_hp),
            (SP_MAXSP, status.max_sp),
            (SP_HP, status.hp),
            (SP_SP, status.sp),
        ] {
            self.emit_self(e, param(var_id, value));
        }
        self.send_nonzero_trait_values(e, &status, &derived);
        self.emit_weight(e, SP_WEIGHT, weight);
    }

    /// The trait part of rAthena's pre-entry diff against zero (captured
    /// `rathena_traits_sequence.txt` 1401-1403): the non-zero POW..CRT, PATK..CRATE, MaxAP and
    /// AP, in diff order.
    fn send_nonzero_trait_values(&mut self, e: Entity, status: &CharacterStatus, derived: &Status) {
        for (offset, (&allocated, battle)) in
            (0..STAT_COUNT).zip(status.traits.iter().zip(battle_traits(derived)))
        {
            if battle != 0 || allocated != 0 {
                self.emit_self(e, trait_couple(offset, allocated, battle));
            }
        }
        for (var_id, value) in (SP_PATK..).zip(trait_values(derived)) {
            if value != 0 {
                self.emit_self(e, ServerEvent::ParameterChanged { var_id, value });
            }
        }
        for (var_id, value) in [(SP_MAXAP, derived.max_ap), (SP_AP, status.ap)] {
            if value != 0 {
                self.emit_self(e, param(var_id, value));
            }
        }
    }

    /// `0x0141` for STR..LUK with the real bonus (battle − allocated).
    fn send_couple_stats(&mut self, e: Entity, status: &CharacterStatus, derived: &Status) {
        for (offset, (stat, battle)) in
            (0..STAT_COUNT).zip(stats(status).into_iter().zip(battle_stats(derived)))
        {
            self.emit_self(e, couple(offset, stat, battle));
        }
    }

    /// `0x0141` for POW..CRT with the real bonus (battle − allocated).
    fn send_trait_couples(&mut self, e: Entity, status: &CharacterStatus, derived: &Status) {
        for (offset, (&allocated, battle)) in
            (0..STAT_COUNT).zip(status.traits.iter().zip(battle_traits(derived)))
        {
            self.emit_self(e, trait_couple(offset, allocated, battle));
        }
    }

    /// A map change's actor init (not a login): `0x0141` for STR..LUK and POW..CRT, after the
    /// unit entries and before the config group (catalog `packets/status.md` "Where the status
    /// packets sit", capture 2 lines 326-337).
    pub(crate) fn send_map_change_stats(&mut self, e: Entity) {
        let Some((status, derived)) = self
            .world
            .players
            .get(e)
            .map(|info| (info.status, info.derived))
        else {
            return;
        };
        self.send_couple_stats(e, &status, &derived);
        self.send_trait_couples(e, &status, &derived);
    }

    /// Sends the exp vars, skill points and the status window with its follow-ups, in rAthena
    /// order. Login only.
    pub(crate) fn send_initial_status(&mut self, e: Entity) {
        let Some((status, derived, job, level)) = self
            .world
            .players
            .get(e)
            .map(|info| (info.status, info.derived, info.job, info.clevel))
        else {
            return;
        };
        for (var_id, value) in [
            (SP_BASEEXP, exp(status.base_exp)),
            (SP_NEXTBASEEXP, exp(self.jobs.next_base_exp(job, level))),
            (SP_JOBEXP, exp(status.job_exp)),
            (
                SP_NEXTJOBEXP,
                exp(self.jobs.next_job_exp(job, status.job_level)),
            ),
        ] {
            self.emit_self(e, ServerEvent::LongLongParameterChanged { var_id, value });
        }
        self.emit_self(e, param(SP_SKILLPOINT, u32::from(status.skill_point)));

        let class = class_flags(job_mapid(job));
        let cap = self.battle.max_stat(&class);
        let window = status_window(&status, &derived, cap);
        self.emit_self(e, ServerEvent::StatusWindow { status: window });
        self.send_couple_stats(e, &status, &derived);
        self.emit_self(
            e,
            ServerEvent::AttackRange {
                range: derived.range,
            },
        );
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id: SP_ASPD,
                value: derived.amotion,
            },
        );
        self.send_trait_couples(e, &status, &derived);
        for (var_id, value) in (SP_PATK..).zip(trait_values(&derived)) {
            self.emit_self(e, ServerEvent::ParameterChanged { var_id, value });
        }
        for (var_id, value) in [
            (SP_TRAITPOINT, u32::from(status.trait_point)),
            (SP_AP, status.ap),
            (SP_MAXAP, derived.max_ap),
        ] {
            self.emit_self(e, param(var_id, value));
        }
        for event in trait_costs(&status.traits, self.battle.max_trait(&class)) {
            self.emit_self(e, event);
        }
    }
}
