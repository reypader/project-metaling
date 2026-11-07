//! The status packets: the push before the client's first `0x007d` (rAthena's first
//! `status_calc_pc`), the block at its actor init (`clif_parse_LoadEndAck` with `connect_new`,
//! then `clif_initialstatus`), the map-change `0x0141`s and the recalculation diff. Catalog:
//! `packets/status.md`, `packets/settings.md`; spec `specs/stats/spec.md` §2.3.
//!
//! The values come from the `ro_mechanics::Status` snapshot (`stats/systems.rs` computes it). There are
//! no exp tables yet, so next exp is rAthena's max-level value. Trait stats and AP follow
//! `specs/traits/spec.md` §2.3.

use ro_core::{CharacterStatus, ServerEvent, StatusWindow};
use ro_mechanics::{next_stat_cost, next_trait_cost, Status};

use crate::protocol_consts::{
    SP_AP, SP_ASPD, SP_ATK1, SP_ATK2, SP_CRITICAL, SP_DEF1, SP_DEF2, SP_FLEE1, SP_FLEE2, SP_HIT,
    SP_HP, SP_MATK1, SP_MATK2, SP_MAXAP, SP_MAXHP, SP_MAXSP, SP_MDEF1, SP_MDEF2, SP_PATK, SP_POW,
    SP_SP, SP_STR, SP_UPOW, STAT_COUNT,
};

const STATS: usize = STAT_COUNT as usize;

/// The value sent for an `SP_*` derived var.
pub(crate) fn derived_value(status: &Status, var_id: u16) -> i32 {
    match var_id {
        SP_HIT => status.hit,
        SP_FLEE1 => status.flee,
        SP_ASPD => status.amotion,
        SP_ATK1 => status.batk,
        SP_DEF1 => status.soft_def,
        SP_DEF2 => status.hard_def,
        SP_ATK2 => status.watk,
        SP_FLEE2 => status.flee2_shown(),
        SP_CRITICAL => status.critical_shown(),
        SP_MATK1 => status.weapon_matk,
        SP_MATK2 => status.status_matk,
        SP_MDEF1 => status.soft_mdef,
        SP_MDEF2 => status.hard_mdef,
        _ => 0,
    }
}

pub(crate) fn battle_stats(status: &Status) -> [i32; 6] {
    let b = status.battle;
    [b.str, b.agi, b.vit, b.int, b.dex, b.luk]
}

/// POW..CRT after every layer.
pub(crate) fn battle_traits(status: &Status) -> [i32; 6] {
    let t = status.trait_battle;
    [t.pow, t.sta, t.wis, t.spl, t.con, t.crt]
}

/// PATK, SMATK, RES, MRES, HPLUS, CRATE: the values of `SP_PATK + i`.
pub(crate) fn trait_values(status: &Status) -> [i32; 6] {
    [
        status.patk,
        status.smatk,
        status.res,
        status.mres,
        status.hplus,
        status.crate_,
    ]
}

/// What a recalc compares besides the snapshot: the allocated STR..LUK then POW..CRT, and HP,
/// SP and AP after the clamp.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Persisted {
    pub(crate) allocated: [u16; 12],
    pub(crate) hp: u32,
    pub(crate) sp: u32,
    pub(crate) ap: u32,
}

/// The allocated STR..LUK then POW..CRT.
pub(crate) fn allocated_stats(status: &CharacterStatus) -> [u16; 12] {
    let [str, agi, vit, int, dex, luk] = stats(status);
    let [pow, sta, wis, spl, con, crt] = status.traits;
    [str, agi, vit, int, dex, luk, pow, sta, wis, spl, con, crt]
}

/// `0x0141` for allocated stat `index` (0..6 STR..LUK, 6..12 POW..CRT, as in
/// `allocated_stats`): `base` is the allocated stat, `bonus` battle − allocated.
pub(crate) fn couple(index: u16, allocated: u16, battle: i32) -> ServerEvent {
    let var_id = match index.checked_sub(STAT_COUNT) {
        Some(offset) => SP_POW + offset,
        None => SP_STR + index,
    };
    couple_var(var_id, allocated, battle)
}

/// `0x0141` for POW..CRT `offset`, likewise.
pub(crate) fn trait_couple(offset: u16, allocated: u16, battle: i32) -> ServerEvent {
    couple(STAT_COUNT + offset, allocated, battle)
}

fn couple_var(var_id: u16, allocated: u16, battle: i32) -> ServerEvent {
    ServerEvent::CoupleStatus {
        var_id: u32::from(var_id),
        base: i32::from(allocated),
        bonus: battle - i32::from(allocated),
    }
}

pub(crate) fn param(var_id: u16, value: u32) -> ServerEvent {
    ServerEvent::ParameterChanged {
        var_id,
        value: i32::try_from(value).unwrap_or(i32::MAX),
    }
}

/// The recalculation push from `old` to `new`: one packet per changed field, in rAthena's
/// order (catalog `packets/status.md` "Recalculation pushes"; spec §2.3 of `specs/stats` and
/// `specs/traits`). `persisted` holds the allocated stats and HP/SP/AP before and after the
/// clamp. A stat's `0x0141` goes out when its battle value or its allocated base changed (a
/// reset can keep the battle value, and the window must still show the new base); POW..CRT
/// likewise, after `SP`. Fields compare in rAthena's internal units, so `cri`/`flee2` (1/10)
/// re-send on any change. A hard DEF change sends DEF1 then DEF2, a soft one DEF2 then DEF1;
/// MATK and MDEF likewise. SPEED doesn't change yet.
pub(crate) fn status_diff(
    old: &Status,
    new: &Status,
    persisted: (&Persisted, &Persisted),
) -> Vec<ServerEvent> {
    let mut events = Vec::new();
    let (old_battle, new_battle) = (battle_stats(old), battle_stats(new));
    let (old_p, new_p) = persisted;
    let (old_allocated, new_allocated) = (&old_p.allocated, &new_p.allocated);
    for offset in 0..STAT_COUNT {
        let i = usize::from(offset);
        if old_battle[i] != new_battle[i] || old_allocated[i] != new_allocated[i] {
            events.push(couple(offset, new_allocated[i], new_battle[i]));
        }
    }
    let mut vars: Vec<u16> = Vec::new();
    let mut push = |changed: bool, ids: &[u16]| {
        if changed {
            vars.extend_from_slice(ids);
        }
    };
    push(old.hit != new.hit, &[SP_HIT]);
    push(old.flee != new.flee, &[SP_FLEE1]);
    push(old.amotion != new.amotion, &[SP_ASPD]);
    push(old.batk != new.batk, &[SP_ATK1]);
    push(old.hard_def != new.hard_def, &[SP_DEF1, SP_DEF2]);
    push(old.watk != new.watk, &[SP_ATK2]);
    push(old.soft_def != new.soft_def, &[SP_DEF2, SP_DEF1]);
    push(old.flee2 != new.flee2, &[SP_FLEE2]);
    push(old.cri != new.cri, &[SP_CRITICAL]);
    push(
        old.weapon_matk != new.weapon_matk || old.status_matk != new.status_matk,
        &[SP_MATK2, SP_MATK1],
    );
    push(old.hard_mdef != new.hard_mdef, &[SP_MDEF1, SP_MDEF2]);
    push(old.soft_mdef != new.soft_mdef, &[SP_MDEF2, SP_MDEF1]);
    events.extend(
        vars.into_iter()
            .map(|var_id| ServerEvent::ParameterChanged {
                var_id,
                value: derived_value(new, var_id),
            }),
    );
    if old.range != new.range {
        events.push(ServerEvent::AttackRange { range: new.range });
    }
    for (changed, var_id, value) in [
        (old.max_hp != new.max_hp, SP_MAXHP, new.max_hp),
        (old.max_sp != new.max_sp, SP_MAXSP, new.max_sp),
        (old_p.hp != new_p.hp, SP_HP, new_p.hp),
        (old_p.sp != new_p.sp, SP_SP, new_p.sp),
    ] {
        if changed {
            events.push(param(var_id, value));
        }
    }
    let (old_traits, new_traits) = (battle_traits(old), battle_traits(new));
    for (offset, i) in (0..STAT_COUNT).zip(0..STATS) {
        let (old_base, new_base) = (old_allocated[STATS + i], new_allocated[STATS + i]);
        if old_traits[i] != new_traits[i] || old_base != new_base {
            events.push(trait_couple(offset, new_base, new_traits[i]));
        }
    }
    for ((var_id, old_value), new_value) in
        (SP_PATK..).zip(trait_values(old)).zip(trait_values(new))
    {
        if old_value != new_value {
            events.push(ServerEvent::ParameterChanged {
                var_id,
                value: new_value,
            });
        }
    }
    for (changed, var_id, value) in [
        (old.max_ap != new.max_ap, SP_MAXAP, new.max_ap),
        (old_p.ap != new_p.ap, SP_AP, new_p.ap),
    ] {
        if changed {
            events.push(param(var_id, value));
        }
    }
    events
}

pub(crate) fn stats(status: &CharacterStatus) -> [u16; 6] {
    [
        status.str, status.agi, status.vit, status.int, status.dex, status.luk,
    ]
}

/// The `0x00bd` window: allocated stats and their costs against the job's cap, and the
/// snapshot's values.
pub(crate) fn status_window(status: &CharacterStatus, derived: &Status, cap: u16) -> StatusWindow {
    let stats = stats(status);
    let short = |v: i32| i16::try_from(v).unwrap_or(i16::MAX);
    StatusWindow {
        status_point: status.status_point,
        stats,
        costs: stats.map(|stat| next_stat_cost(stat, cap)),
        atk: short(derived.batk),
        atk_bonus: short(derived.watk),
        soft_def: short(derived.soft_def),
        hard_def: short(derived.hard_def),
        soft_mdef: short(derived.soft_mdef),
        hard_mdef: short(derived.hard_mdef),
        hit: short(derived.hit),
        flee: short(derived.flee),
        perfect_dodge: short(derived.flee2_shown()),
        critical: short(derived.critical_shown()),
        aspd: short(derived.amotion),
        matk_bonus: short(derived.weapon_matk),
        matk: short(derived.status_matk),
    }
}

/// `0x00be` `SP_UPOW..SP_UCRT`: the next-point cost of each allocated trait against `cap`.
pub(crate) fn trait_costs(traits: &[u16; 6], cap: u16) -> impl Iterator<Item = ServerEvent> + '_ {
    (0..STAT_COUNT)
        .zip(traits)
        .map(move |(offset, &value)| ServerEvent::StatusCost {
            var_id: SP_UPOW + offset,
            value: next_trait_cost(value, cap),
        })
}

/// An exp value as the 64-bit signed wire field (rAthena `client_exp`).
pub(crate) fn exp(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests;
