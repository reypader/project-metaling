//! A mob's status from its database entry (catalog `mechanics/combat.md` §1.1). Computed with
//! the db `Level`: with `mobs_level_up` off a spawn-line level changes no stat.

use crate::{BattleConfig, Primary, Status};

/// The renewal clamp for HIT, FLEE and soft DEF (`SHRT_MAX`).
const SHORT_MAX: i32 = i16::MAX as i32;
/// Mob HIT and FLEE bases (renewal; the PC-only `luk` terms are absent).
const MOB_HIT_BASE: i32 = 150;
const MOB_FLEE_BASE: i32 = 100;

/// A mob's db values, mapped from `ro_db::MobData` by the engine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MobStatusInputs {
    /// The db `Level`, not the spawn-line level.
    pub level: u16,
    /// `Str`..`Luk`.
    pub stats: Primary,
    /// Hard DEF.
    pub defense: i32,
    pub resistance: i32,
    pub max_hp: u32,
    /// Already clamped and `attack_delay >= attack_motion` (the importer does it).
    pub attack_motion: u16,
    pub attack_delay: u16,
    pub damage_motion: u16,
    pub attack_range: u16,
}

/// The mob's battle status. Mobs never crit or lucky-dodge (`enable_critical` and
/// `enable_perfect_flee` exclude them by default; other masks are not supported), and have no
/// trait stats. ATK and MATK stay 0 until mobs attack.
pub fn calc_mob_status(inputs: &MobStatusInputs, _config: &BattleConfig) -> Status {
    let level = i32::from(inputs.level);
    let stats = inputs.stats;
    let amotion = i32::from(inputs.attack_motion);
    Status {
        base: stats,
        battle: stats,
        hit: (level + stats.dex + MOB_HIT_BASE).clamp(1, SHORT_MAX),
        flee: (level + stats.agi + MOB_FLEE_BASE).clamp(1, SHORT_MAX),
        flee2: 0,
        cri: 0,
        hard_def: inputs.defense,
        // rAthena divides as float; for non-negative values that is integer `/ 2`.
        soft_def: ((level + stats.vit) / 2).clamp(0, SHORT_MAX),
        soft_mdef: (stats.int + level) / 4,
        res: inputs.resistance,
        amotion,
        adelay: i32::from(inputs.attack_delay).max(amotion),
        dmotion: i32::from(inputs.damage_motion),
        max_hp: inputs.max_hp,
        range: inputs.attack_range,
        ..Status::default()
    }
}
