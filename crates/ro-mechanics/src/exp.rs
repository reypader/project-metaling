//! Kill exp share (catalog `mechanics/experience.md` "Rate modifiers", checked against
//! `combat_sequence_1.txt` step 8). No level-difference penalty: every kill pays the full share
//! (user decision, spec §2.9).

use crate::BattleConfig;

/// One attacker's share of a kill (`exp_calc_type 0`): `damage / total`, at most 1, plus
/// `exp_bonus_attacker` % per extra attacker when several attacked, counting at most
/// `exp_bonus_max_attacker` of them.
pub fn exp_share(damage: u64, total: u64, attackers: usize, config: &BattleConfig) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let mut share = (damage as f64 / total as f64).min(1.0);
    if attackers > 1 {
        let max = usize::try_from(config.exp_bonus_max_attacker).unwrap_or(usize::MAX);
        let extra = u32::try_from(attackers.min(max).saturating_sub(1)).unwrap_or(u32::MAX);
        // rAthena's order: the integer bonus first, then one multiply and one divide. Scaling
        // in another order changes the truncated exp for 3+ attackers.
        let bonus = extra.saturating_mul(config.exp_bonus_attacker);
        share += share * f64::from(bonus) / 100.0;
    }
    share
}

/// The exp one attacker gets from a db exp value: `trunc(exp × share)`, at least 1 unless the
/// db value is 0.
pub fn shared_exp(db_exp: u64, share: f64) -> u64 {
    if db_exp == 0 {
        return 0;
    }
    ((db_exp as f64 * share) as u64).max(1)
}
