//! The first-login status block (rAthena `clif_parse_LoadEndAck` with `connect_new`, then
//! `clif_initialstatus`). Catalog: `packets/status.md`, `packets/settings.md`.
//!
//! Only the loaded values are real: allocated stats, points and exp. There are no exp tables and
//! no derived-stat formulas yet, so next exp is rAthena's max-level value and every derived stat
//! is 0.

use ro_core::{CharacterStatus, ServerEvent, StatusWindow};

use crate::engine::MapInstance;
use crate::entity::Entity;

/// `SP_*` var ids (rAthena `enum _sp`).
const SP_BASEEXP: u16 = 1;
const SP_JOBEXP: u16 = 2;
const SP_SKILLPOINT: u16 = 12;
const SP_STR: u16 = 13;
const SP_NEXTBASEEXP: u16 = 22;
const SP_NEXTJOBEXP: u16 = 23;
const SP_ASPD: u16 = 53;
const SP_POW: u16 = 219;
/// `SP_PATK`..`SP_CRATE`, `SP_TRAITPOINT`, `SP_AP`, `SP_MAXAP`: sent as `0x00b0` in this order.
const TRAIT_PARAMS: [u16; 9] = [225, 226, 227, 228, 229, 230, 231, 232, 233];
const SP_UPOW: u16 = 247;
/// STR..LUK and POW..CRT.
const STAT_COUNT: u16 = 6;

/// `pc_nextbaseexp` / `pc_nextjobexp` at max level (`MAX_LEVEL_BASE_EXP`, `MAX_LEVEL_JOB_EXP`).
/// Sent until exp tables exist: never 0, so the client's exp bar can't divide by 0.
const PLACEHOLDER_NEXT_BASE_EXP: i64 = 99_999_999;
const PLACEHOLDER_NEXT_JOB_EXP: i64 = 999_999_999;
/// Highest allocated stat for most jobs; 3rd/4th classes allow 130, babies 80.
const MAX_STAT: u16 = 99;
/// Bare-handed attack range in cells.
const BARE_HAND_RANGE: u16 = 1;

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

/// The `0x00bd` window for `status`: loaded stats and their costs, derived values 0.
pub(crate) fn status_window(status: &CharacterStatus) -> StatusWindow {
    let stats = stats(status);
    StatusWindow {
        status_point: status.status_point,
        stats,
        costs: stats.map(stat_raise_cost),
        ..StatusWindow::default()
    }
}

fn exp(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

impl MapInstance {
    /// Sends the exp vars, skill points and the status window with its follow-ups, in rAthena
    /// order. Login only.
    pub(crate) fn send_initial_status(&mut self, e: Entity) {
        let Some(status) = self.world.players.get(e).map(|info| info.status) else {
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

        let window = status_window(&status);
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
                range: BARE_HAND_RANGE,
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

#[cfg(test)]
mod tests {
    use super::*;

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
