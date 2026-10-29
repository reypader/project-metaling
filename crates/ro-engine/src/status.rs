//! The status packets: the push before the client's first `0x007d` (rAthena's first
//! `status_calc_pc`), the block at its actor init (`clif_parse_LoadEndAck` with `connect_new`,
//! then `clif_initialstatus`), the map-change `0x0141`s and the recalculation diff. Catalog:
//! `packets/status.md`, `packets/settings.md`; spec `specs/stats/spec.md` §2.3.
//!
//! The values come from the `ro_mechanics::Status` snapshot (`stats.rs` computes it). There are
//! no exp tables yet, so next exp is rAthena's max-level value. Trait stats and AP follow
//! `specs/traits/spec.md` §2.3.

use ro_core::{CharacterStatus, ServerEvent, StatusWindow};
use ro_mechanics::{next_stat_cost, next_trait_cost, Status};

use crate::engine::{MapInstance, SP_MAXWEIGHT, SP_WEIGHT};
use crate::entity::Entity;
use crate::job::{class_flags, job_mapid};

/// `SP_*` var ids (rAthena `enum _sp`).
pub(crate) const SP_BASEEXP: u16 = 1;
pub(crate) const SP_JOBEXP: u16 = 2;
pub(crate) const SP_HP: u16 = 5;
const SP_MAXHP: u16 = 6;
pub(crate) const SP_SP: u16 = 7;
const SP_MAXSP: u16 = 8;
pub(crate) const SP_STATUSPOINT: u16 = 9;
pub(crate) const SP_SKILLPOINT: u16 = 12;
pub(crate) const SP_STR: u16 = 13;
pub(crate) const SP_LUK: u16 = 18;
/// `SP_USTR`..`SP_ULUK`: the `0x00be` next-point costs.
pub(crate) const SP_USTR: u16 = 32;
pub(crate) const SP_BASELEVEL: u16 = 11;
pub(crate) const SP_JOBLEVEL: u16 = 55;
pub(crate) const SP_TRAITPOINT: u16 = 231;
pub(crate) const SP_NEXTBASEEXP: u16 = 22;
pub(crate) const SP_NEXTJOBEXP: u16 = 23;
const SP_ATK1: u16 = 41;
const SP_ATK2: u16 = 42;
const SP_MATK1: u16 = 43;
const SP_MATK2: u16 = 44;
const SP_DEF1: u16 = 45;
const SP_DEF2: u16 = 46;
const SP_MDEF1: u16 = 47;
const SP_MDEF2: u16 = 48;
const SP_HIT: u16 = 49;
const SP_FLEE1: u16 = 50;
const SP_FLEE2: u16 = 51;
const SP_CRITICAL: u16 = 52;
const SP_ASPD: u16 = 53;
pub(crate) const SP_POW: u16 = 219;
pub(crate) const SP_CRT: u16 = 224;
/// `SP_PATK`..`SP_CRATE`: the trait-derived values, sent as `0x00b0` in this order.
const SP_PATK: u16 = 225;
pub(crate) const SP_AP: u16 = 232;
const SP_MAXAP: u16 = 233;
pub(crate) const SP_UPOW: u16 = 247;
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
/// STR..LUK and POW..CRT.
const STAT_COUNT: u16 = 6;
const STATS: usize = STAT_COUNT as usize;

/// `pc_nextbaseexp` / `pc_nextjobexp` at max level (`MAX_LEVEL_BASE_EXP`, `MAX_LEVEL_JOB_EXP`).
/// Sent until exp tables exist: never 0, so the client's exp bar can't divide by 0.
pub(crate) const PLACEHOLDER_NEXT_BASE_EXP: i64 = 99_999_999;
pub(crate) const PLACEHOLDER_NEXT_JOB_EXP: i64 = 999_999_999;

/// The value sent for an `SP_*` derived var.
fn derived_value(status: &Status, var_id: u16) -> i32 {
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
fn trait_values(status: &Status) -> [i32; 6] {
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
fn trait_couple(offset: u16, allocated: u16, battle: i32) -> ServerEvent {
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

fn stats(status: &CharacterStatus) -> [u16; 6] {
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

fn exp(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

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
        let Some((status, derived, job)) = self
            .world
            .players
            .get(e)
            .map(|info| (info.status, info.derived, info.job))
        else {
            return;
        };
        for (var_id, value) in [
            (SP_BASEEXP, exp(status.base_exp)),
            (SP_NEXTBASEEXP, PLACEHOLDER_NEXT_BASE_EXP),
            (SP_JOBEXP, exp(status.job_exp)),
            (SP_NEXTJOBEXP, PLACEHOLDER_NEXT_JOB_EXP),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ro_mechanics::{Primary, Traits};

    fn p(var_id: u16, value: i32) -> ServerEvent {
        ServerEvent::ParameterChanged { var_id, value }
    }

    /// STR..LUK allocated 1, no traits, HP/SP/AP `vitals`.
    fn persisted(vitals: (u32, u32, u32)) -> Persisted {
        let mut allocated = [0; 12];
        allocated[..6].fill(1);
        Persisted {
            allocated,
            hp: vitals.0,
            sp: vitals.1,
            ap: vitals.2,
        }
    }

    fn diff(old: &Status, new: &Status) -> Vec<ServerEvent> {
        let same = persisted((0, 0, 0));
        status_diff(old, new, (&same, &same))
    }

    /// ENG-STAT-01: only changed fields are pushed, in the catalog order.
    #[test]
    fn eng_stat_01_diff_pushes_changes_in_order() {
        let old = Status::default();
        assert!(diff(&old, &old).is_empty(), "no change, no packets");

        let atk = Status { watk: 17, ..old };
        assert_eq!(diff(&old, &atk), vec![p(42, 17)]);

        let def = Status {
            hard_def: 20,
            ..old
        };
        assert_eq!(diff(&old, &def), vec![p(45, 0), p(46, 20)]);

        let soft = Status { soft_def: 3, ..old };
        assert_eq!(diff(&old, &soft), vec![p(46, 0), p(45, 3)]);

        let range = Status { range: 5, ..old };
        assert_eq!(
            diff(&old, &range),
            vec![ServerEvent::AttackRange { range: 5 }]
        );

        let all = Status {
            hit: 1,
            flee: 2,
            amotion: 3,
            batk: 4,
            hard_def: 5,
            watk: 6,
            soft_def: 7,
            flee2: 80,
            cri: 90,
            weapon_matk: 10,
            hard_mdef: 11,
            soft_mdef: 12,
            range: 13,
            max_hp: 14,
            max_sp: 15,
            ..old
        };
        assert_eq!(
            status_diff(&old, &all, (&persisted((0, 0, 0)), &persisted((14, 15, 0)))),
            vec![
                p(49, 1),
                p(50, 2),
                p(53, 3),
                p(41, 4),
                p(45, 7),
                p(46, 5),
                p(42, 6),
                p(46, 5),
                p(45, 7),
                p(51, 8),
                p(52, 9),
                p(44, 0),
                p(43, 10),
                p(47, 12),
                p(48, 11),
                p(48, 11),
                p(47, 12),
                ServerEvent::AttackRange { range: 13 },
                p(6, 14),
                p(8, 15),
                p(5, 14),
                p(7, 15),
            ]
        );
    }

    /// ENG-STAT-01 (extended): `0x0141` first, for each changed battle stat, with `bonus` =
    /// battle − allocated; CRITICAL and FLEE2 compare in 1/10 units (captured LUK re-send).
    #[test]
    fn eng_stat_01_couple_stats_and_internal_units() {
        let old = Status {
            battle: Primary {
                str: 1,
                agi: 1,
                vit: 1,
                int: 1,
                dex: 1,
                luk: 1,
            },
            cri: 13,
            flee2: 11,
            ..Status::default()
        };
        let new = Status {
            battle: Primary {
                luk: 3,
                ..old.battle
            },
            cri: 19,
            flee2: 13,
            ..old
        };
        let mut more_luk = persisted((40, 11, 0));
        more_luk.allocated[5] = 2;
        assert_eq!(
            status_diff(&old, &new, (&persisted((40, 11, 0)), &more_luk)),
            vec![
                ServerEvent::CoupleStatus {
                    var_id: 18,
                    base: 2,
                    bonus: 1,
                },
                p(51, 1),
                p(52, 1),
            ],
            "shown values unchanged, both re-sent"
        );
    }

    /// ENG-STAT-01 with traits (traits spec §2.3): after `SP` come the changed POW..CRT
    /// `0x0141`s (allocated or battle), then PATK..CRATE, then `MAXAP` and `AP`.
    #[test]
    fn eng_stat_01_trait_block_after_sp() {
        let old = Status {
            trait_battle: Traits {
                pow: 1,
                ..Traits::default()
            },
            ..Status::default()
        };
        let new = Status {
            hit: 5,
            max_sp: 9,
            trait_battle: Traits {
                pow: 2,
                con: 4,
                ..old.trait_battle
            },
            patk: 1,
            smatk: 2,
            res: 3,
            mres: 4,
            hplus: 5,
            crate_: 6,
            max_ap: 200,
            ..old
        };
        let before = persisted((10, 10, 0));
        let mut after = persisted((10, 20, 30));
        after.allocated[6] = 1; // POW
        after.allocated[10] = 4; // CON
        after.allocated[11] = 3; // CRT: allocated changed, battle (0) the same
        let couple = |var_id, base, bonus| ServerEvent::CoupleStatus {
            var_id,
            base,
            bonus,
        };
        assert_eq!(
            status_diff(&old, &new, (&before, &after)),
            vec![
                p(49, 5),
                p(8, 9),
                p(7, 20),
                couple(219, 1, 1),
                couple(223, 4, 0),
                couple(224, 3, -3),
                p(225, 1),
                p(226, 2),
                p(227, 3),
                p(228, 4),
                p(229, 5),
                p(230, 6),
                p(233, 200),
                p(232, 30),
            ]
        );
        assert!(diff(&new, &new).is_empty());
    }

    #[test]
    fn window_costs_use_the_job_cap() {
        let status = CharacterStatus {
            str: 99,
            agi: 98,
            ..CharacterStatus::default()
        };
        let derived = Status {
            cri: 15,
            flee2: 11,
            amotion: 440,
            ..Status::default()
        };
        let window = status_window(&status, &derived, 99);
        assert_eq!(window.costs[..3], [0, 11, 2]);
        assert_eq!(
            (window.critical, window.perfect_dodge, window.aspd),
            (1, 1, 440)
        );
        assert_eq!(status_window(&status, &derived, 130).costs[0], 11);
    }
}
