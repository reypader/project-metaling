//! Renewal cast time and after-cast delay at default config (`specs/effects/spec.md` §2.3-§2.4;
//! catalog `scripting/skills.md` §4 and §4.1). No status, item or skill reductions exist yet.

use crate::{BattleConfig, CastTimeData, Caster};

/// The cast time in ms. For a player: the variable part is
/// `time × (1 − √((2·DEX + INT) / vcast_stat_scale))`, truncated and at least 0, then the fixed
/// part is added. The quotient and the square root are single precision, as in rAthena; the
/// rest is double. Mobs and NPCs ignore the fixed part.
pub fn cast_time(data: CastTimeData, caster: Caster, config: &BattleConfig) -> u32 {
    let Caster::Player { dex, int } = caster else {
        return data.cast_time;
    };
    let mut time = f64::from(data.cast_time);
    let mut fixed = i64::from(data.fixed_cast_time);
    if fixed < 0 {
        let rate = i64::from(config.default_fixed_castrate.min(100));
        fixed = i64::from(data.cast_time) * rate / 100;
        time = time * (100 - rate) as f64 / 100.0;
    }
    if !data.ignore_dex {
        let stats = (2 * i64::from(dex) + i64::from(int)).max(0) as f32;
        let factor = 1.0f32 - (stats / config.vcast_stat_scale.max(1) as f32).sqrt();
        time *= f64::from(factor);
    }
    // C truncates the variable part to an int (at least 0), adds the fixed part, truncates again.
    let variable = (time as i64).max(0);
    u32::try_from(variable + fixed.max(0)).unwrap_or(u32::MAX)
}

/// rAthena `skill_delayfix` at default config: the after-cast act delay, where a negative value
/// means `−delay + amotion`, at least 0.
pub fn delayfix(after_cast_act_delay: i32, amotion: u32) -> u32 {
    let delay = i64::from(after_cast_act_delay);
    let time = if delay < 0 {
        -delay + i64::from(amotion)
    } else {
        delay
    };
    u32::try_from(time.max(0)).unwrap_or(u32::MAX)
}
