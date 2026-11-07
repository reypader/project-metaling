//! Battle values of a mob id, computed once per map instance.

use ro_db::{MobData, MobMode};
use ro_mechanics::{calc_mob_status, BattleConfig, DefenderView, MobStatusInputs, Primary, Status};

/// A mob type's battle values, computed once per mob id (spec `specs/combat/spec.md` §2.1) and
/// shared by every mob of that id on the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MobStats {
    pub(crate) status: Status,
    /// What a swing at this mob reads (with the `IgnoreMelee`/`IgnoreRanged` modes).
    pub(crate) defender: DefenderView,
    /// The db `BaseExp` / `JobExp` a kill shares out.
    pub(crate) base_exp: u64,
    pub(crate) job_exp: u64,
}

impl MobStats {
    pub(crate) fn new(data: &MobData, config: &BattleConfig) -> Self {
        let inputs = MobStatusInputs {
            level: data.level,
            stats: Primary {
                str: i32::from(data.str),
                agi: i32::from(data.agi),
                vit: i32::from(data.vit),
                int: i32::from(data.int),
                dex: i32::from(data.dex),
                luk: i32::from(data.luk),
            },
            defense: i32::from(data.defense),
            resistance: i32::from(data.resistance),
            max_hp: data.hp,
            attack_motion: data.attack_motion,
            attack_delay: data.attack_delay,
            damage_motion: data.damage_motion,
            attack_range: data.attack_range,
        };
        let status = calc_mob_status(&inputs, config);
        let defender = DefenderView {
            ignore_melee: data.has_mode(MobMode::IgnoreMelee),
            ignore_ranged: data.has_mode(MobMode::IgnoreRanged),
            ..DefenderView::from_status(&status)
        };
        Self {
            status,
            defender,
            base_exp: data.base_exp,
            job_exp: data.job_exp,
        }
    }
}
