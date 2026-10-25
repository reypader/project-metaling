//! The first-login status packets: the push before the client's first `0x007d` (rAthena's first
//! `status_calc_pc`) and the block at its actor init (`clif_parse_LoadEndAck` with `connect_new`,
//! then `clif_initialstatus`). Catalog: `packets/status.md`, `packets/settings.md`.
//!
//! Also the derived status and its recalculation push (spec `specs/equipment/spec.md` §2.10).
//!
//! Only the loaded values and the gear values are real: allocated stats, points, exp, HP and SP,
//! weapon ATK, hard DEF and attack range. There are no exp tables and no stat formulas yet, so
//! next exp is rAthena's max-level value and every other derived stat is 0.

use ro_core::{CharacterStatus, ServerEvent, StatusWindow};

use crate::engine::{MapInstance, SP_MAXWEIGHT, SP_WEIGHT};
use crate::entity::Entity;
use crate::equipment::GearStatus;

/// `SP_*` var ids (rAthena `enum _sp`).
const SP_BASEEXP: u16 = 1;
const SP_JOBEXP: u16 = 2;
const SP_HP: u16 = 5;
const SP_MAXHP: u16 = 6;
const SP_SP: u16 = 7;
const SP_MAXSP: u16 = 8;
const SP_SKILLPOINT: u16 = 12;
const SP_STR: u16 = 13;
const SP_NEXTBASEEXP: u16 = 22;
const SP_NEXTJOBEXP: u16 = 23;
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
const SP_POW: u16 = 219;
/// `SP_PATK`..`SP_CRATE`, `SP_TRAITPOINT`, `SP_AP`, `SP_MAXAP`: sent as `0x00b0` in this order.
const TRAIT_PARAMS: [u16; 9] = [225, 226, 227, 228, 229, 230, 231, 232, 233];
const SP_UPOW: u16 = 247;
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

/// `pc_nextbaseexp` / `pc_nextjobexp` at max level (`MAX_LEVEL_BASE_EXP`, `MAX_LEVEL_JOB_EXP`).
/// Sent until exp tables exist: never 0, so the client's exp bar can't divide by 0.
const PLACEHOLDER_NEXT_BASE_EXP: i64 = 99_999_999;
const PLACEHOLDER_NEXT_JOB_EXP: i64 = 999_999_999;
/// Highest allocated stat for most jobs; 3rd/4th classes allow 130, babies 80.
const MAX_STAT: u16 = 99;

/// The derived fields of the recalculation push, by the `SP_*` var each one is sent as. HP, SP
/// and their maxima stay in `CharacterStatus` (nothing recomputes them yet); speed and the trait
/// values join with the stats feature. Only the gear part is computed today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DerivedStatus {
    pub(crate) hit: i32,
    pub(crate) flee1: i32,
    /// `amotion`, as the push sends it.
    pub(crate) aspd: i32,
    /// Status ATK (`SP_ATK1`).
    pub(crate) atk1: i32,
    /// Equipment DEF (rAthena `def`, sent as `SP_DEF2`).
    pub(crate) hard_def: i32,
    /// Weapon ATK (`SP_ATK2`).
    pub(crate) atk2: i32,
    /// VIT DEF (rAthena `def2`, sent as `SP_DEF1`).
    pub(crate) soft_def: i32,
    pub(crate) flee2: i32,
    pub(crate) critical: i32,
    /// Weapon MATK (`SP_MATK1`, the window's right side; capture 4).
    pub(crate) matk1: i32,
    /// Status MATK (`SP_MATK2`, the window's left side).
    pub(crate) matk2: i32,
    /// Equipment MDEF (`SP_MDEF2`).
    pub(crate) hard_mdef: i32,
    /// INT MDEF (`SP_MDEF1`).
    pub(crate) soft_mdef: i32,
    /// Attack range in cells (`0x013a`).
    pub(crate) range: u16,
}

impl DerivedStatus {
    /// The gear-only status. The stats feature replaces this with the full calculation.
    pub(crate) fn from_gear(gear: &GearStatus) -> Self {
        let value = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
        Self {
            atk2: value(gear.atk2),
            matk1: value(gear.matk),
            hard_def: value(gear.hard_def),
            range: gear.range,
            ..Self::default()
        }
    }

    /// The value sent for an `SP_*` derived var.
    fn value(&self, var_id: u16) -> i32 {
        match var_id {
            SP_HIT => self.hit,
            SP_FLEE1 => self.flee1,
            SP_ASPD => self.aspd,
            SP_ATK1 => self.atk1,
            SP_DEF1 => self.soft_def,
            SP_DEF2 => self.hard_def,
            SP_ATK2 => self.atk2,
            SP_FLEE2 => self.flee2,
            SP_CRITICAL => self.critical,
            SP_MATK1 => self.matk1,
            SP_MATK2 => self.matk2,
            SP_MDEF1 => self.soft_mdef,
            SP_MDEF2 => self.hard_mdef,
            _ => 0,
        }
    }
}

/// The recalculation push from `old` to `new`: one packet per changed field, in rAthena's
/// order (catalog `packets/status.md` "Recalculation pushes"). A hard DEF change sends DEF1
/// then DEF2, a soft one DEF2 then DEF1; MATK and MDEF likewise.
pub(crate) fn derived_diff(old: &DerivedStatus, new: &DerivedStatus) -> Vec<ServerEvent> {
    let mut vars: Vec<u16> = Vec::new();
    let mut push = |changed: bool, ids: &[u16]| {
        if changed {
            vars.extend_from_slice(ids);
        }
    };
    push(old.hit != new.hit, &[SP_HIT]);
    push(old.flee1 != new.flee1, &[SP_FLEE1]);
    push(old.aspd != new.aspd, &[SP_ASPD]);
    push(old.atk1 != new.atk1, &[SP_ATK1]);
    push(old.hard_def != new.hard_def, &[SP_DEF1, SP_DEF2]);
    push(old.atk2 != new.atk2, &[SP_ATK2]);
    push(old.soft_def != new.soft_def, &[SP_DEF2, SP_DEF1]);
    push(old.flee2 != new.flee2, &[SP_FLEE2]);
    push(old.critical != new.critical, &[SP_CRITICAL]);
    push(
        old.matk1 != new.matk1 || old.matk2 != new.matk2,
        &[SP_MATK2, SP_MATK1],
    );
    push(old.hard_mdef != new.hard_mdef, &[SP_MDEF1, SP_MDEF2]);
    push(old.soft_mdef != new.soft_mdef, &[SP_MDEF2, SP_MDEF1]);
    let mut events: Vec<ServerEvent> = vars
        .into_iter()
        .map(|var_id| ServerEvent::ParameterChanged {
            var_id,
            value: new.value(var_id),
        })
        .collect();
    if old.range != new.range {
        events.push(ServerEvent::AttackRange { range: new.range });
    }
    events
}

/// Status points to raise a stat from `stat` by one (renewal `PC_STATUS_POINT_COST`); 0 at
/// the cap.
pub(crate) fn stat_raise_cost(stat: u16) -> u8 {
    if stat >= MAX_STAT {
        return 0;
    }
    let cost = if stat < 100 {
        2 + stat.saturating_sub(1) / 10
    } else {
        16 + 4 * ((stat - 100) / 5)
    };
    u8::try_from(cost).unwrap_or(u8::MAX)
}

fn stats(status: &CharacterStatus) -> [u16; 6] {
    [
        status.str, status.agi, status.vit, status.int, status.dex, status.luk,
    ]
}

/// The `0x00bd` window: loaded stats and their costs, and the stored derived values.
pub(crate) fn status_window(status: &CharacterStatus, derived: &DerivedStatus) -> StatusWindow {
    let stats = stats(status);
    let short = |v: i32| i16::try_from(v).unwrap_or(i16::MAX);
    StatusWindow {
        status_point: status.status_point,
        stats,
        costs: stats.map(stat_raise_cost),
        atk: short(derived.atk1),
        atk_bonus: short(derived.atk2),
        soft_def: short(derived.soft_def),
        hard_def: short(derived.hard_def),
        soft_mdef: short(derived.soft_mdef),
        hard_mdef: short(derived.hard_mdef),
        hit: short(derived.hit),
        flee: short(derived.flee1),
        perfect_dodge: short(derived.flee2),
        critical: short(derived.critical),
        aspd: short(derived.aspd),
        matk_bonus: short(derived.matk1),
        matk: short(derived.matk2),
    }
}

fn exp(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn param(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
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
        for (offset, stat) in (0..STAT_COUNT).zip(stats(&status)) {
            self.emit_self(
                e,
                ServerEvent::CoupleStatus {
                    var_id: u32::from(SP_STR + offset),
                    base: i32::from(stat),
                    bonus: 0,
                },
            );
        }
        for var_id in PUSH_DERIVED_PARAMS {
            let value = derived.value(var_id);
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
            self.emit_self(
                e,
                ServerEvent::ParameterChanged {
                    var_id,
                    value: param(value),
                },
            );
        }
        self.emit_weight(e, SP_WEIGHT, weight);
    }

    /// Sends the exp vars, skill points and the status window with its follow-ups, in rAthena
    /// order. Login only.
    pub(crate) fn send_initial_status(&mut self, e: Entity) {
        let Some((status, derived)) = self
            .world
            .players
            .get(e)
            .map(|info| (info.status, info.derived))
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
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id: SP_SKILLPOINT,
                value: i32::from(status.skill_point),
            },
        );

        let window = status_window(&status, &derived);
        self.emit_self(e, ServerEvent::StatusWindow { status: window });
        for (offset, stat) in (0..STAT_COUNT).zip(window.stats) {
            self.emit_self(
                e,
                ServerEvent::CoupleStatus {
                    var_id: u32::from(SP_STR + offset),
                    base: i32::from(stat),
                    bonus: 0,
                },
            );
        }
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
                value: i32::from(window.aspd),
            },
        );
        for offset in 0..STAT_COUNT {
            self.emit_self(
                e,
                ServerEvent::CoupleStatus {
                    var_id: u32::from(SP_POW + offset),
                    base: 0,
                    bonus: 0,
                },
            );
        }
        for var_id in TRAIT_PARAMS {
            self.emit_self(e, ServerEvent::ParameterChanged { var_id, value: 0 });
        }
        for offset in 0..STAT_COUNT {
            self.emit_self(
                e,
                ServerEvent::StatusCost {
                    var_id: SP_UPOW + offset,
                    value: 0,
                },
            );
        }
    }
}

impl MapInstance {
    /// The derived status of `e`'s worn gear now.
    pub(crate) fn current_derived(&self, e: Entity) -> DerivedStatus {
        let gear = self
            .world
            .inventories
            .get(e)
            .map(|inventory| inventory.gear(&self.items).gear_status())
            .unwrap_or_default();
        DerivedStatus::from_gear(&gear)
    }

    /// Spawn: store the derived status of the worn gear without pushing it (the login push and
    /// the status block send it).
    pub(crate) fn init_derived(&mut self, e: Entity) {
        let derived = self.current_derived(e);
        if let Some(info) = self.world.players.get_mut(e) {
            info.derived = derived;
        }
    }

    /// rAthena `status_calc_pc` after an equip change: recompute, push what changed, store.
    pub(crate) fn recalc(&mut self, e: Entity) {
        let new = self.current_derived(e);
        let Some(old) = self.world.players.get_mut(e).map(|info| {
            let old = info.derived;
            info.derived = new;
            old
        }) else {
            return;
        };
        for event in derived_diff(&old, &new) {
            self.emit_self(e, event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(var_id: u16, value: i32) -> ServerEvent {
        ServerEvent::ParameterChanged { var_id, value }
    }

    /// ENG-STAT-01: only changed fields are pushed, in the catalog order.
    #[test]
    fn eng_stat_01_diff_pushes_changes_in_order() {
        let old = DerivedStatus::default();
        assert!(derived_diff(&old, &old).is_empty(), "no change, no packets");

        let atk = DerivedStatus { atk2: 17, ..old };
        assert_eq!(derived_diff(&old, &atk), vec![param(42, 17)]);

        let def = DerivedStatus {
            hard_def: 20,
            ..old
        };
        assert_eq!(derived_diff(&old, &def), vec![param(45, 0), param(46, 20)]);

        let soft = DerivedStatus { soft_def: 3, ..old };
        assert_eq!(derived_diff(&old, &soft), vec![param(46, 0), param(45, 3)]);

        let range = DerivedStatus { range: 5, ..old };
        assert_eq!(
            derived_diff(&old, &range),
            vec![ServerEvent::AttackRange { range: 5 }]
        );

        let all = DerivedStatus {
            hit: 1,
            flee1: 2,
            aspd: 3,
            atk1: 4,
            hard_def: 5,
            atk2: 6,
            soft_def: 7,
            flee2: 8,
            critical: 9,
            matk1: 10,
            hard_mdef: 11,
            soft_mdef: 12,
            range: 13,
            ..old
        };
        assert_eq!(
            derived_diff(&old, &all),
            vec![
                param(49, 1),
                param(50, 2),
                param(53, 3),
                param(41, 4),
                param(45, 7),
                param(46, 5),
                param(42, 6),
                param(46, 5),
                param(45, 7),
                param(51, 8),
                param(52, 9),
                param(44, 0),
                param(43, 10),
                param(47, 12),
                param(48, 11),
                param(48, 11),
                param(47, 12),
                ServerEvent::AttackRange { range: 13 },
            ]
        );
    }

    #[test]
    fn gear_values_feed_the_derived_status() {
        let gear = GearStatus {
            atk2: 135,
            matk: 30,
            hard_def: 25,
            range: 5,
        };
        let derived = DerivedStatus::from_gear(&gear);
        assert_eq!(
            (derived.atk2, derived.matk1, derived.hard_def, derived.range),
            (135, 30, 25, 5)
        );
        assert_eq!(derived.aspd, 0, "ASPD waits for the stats feature");
    }

    #[test]
    fn stat_raise_cost_follows_the_renewal_table() {
        assert_eq!(stat_raise_cost(1), 2);
        assert_eq!(stat_raise_cost(10), 2);
        assert_eq!(stat_raise_cost(11), 3);
        assert_eq!(stat_raise_cost(50), 6);
        assert_eq!(stat_raise_cost(98), 11);
        assert_eq!(stat_raise_cost(99), 0, "at the cap");
    }
}
