//! Stat-point costs (renewal `RENEWAL_STAT`; catalog `mechanics/stats.md` "Cost to raise one
//! stat") and trait-point costs (catalog `mechanics/trait_stats.md` §4).

/// Points to raise a stat currently at `low` by one (`PC_STATUS_POINT_COST`).
pub fn stat_cost(low: u16) -> u32 {
    let low = u32::from(low);
    if low < 100 {
        2 + low.saturating_sub(1) / 10
    } else {
        16 + 4 * ((low - 100) / 5)
    }
}

/// The next-point cost sent in `0x00be`/`0x00bd`: 0 once the stat is at `cap`.
pub fn next_stat_cost(current: u16, cap: u16) -> u8 {
    if current >= cap {
        return 0;
    }
    u8::try_from(stat_cost(current)).unwrap_or(u8::MAX)
}

/// How far a stat-up request goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatRaise {
    pub amount: u16,
    pub cost: u32,
}

/// Raises `current` by up to `requested`, stopping at `cap` or when `points` run out
/// (`pc_maxparameterincrease`). Zero when nothing can be raised.
pub fn raise_stat(current: u16, requested: u16, points: u32, cap: u16) -> StatRaise {
    let mut raise = StatRaise::default();
    let mut stat = current;
    while raise.amount < requested && stat < cap {
        let cost = stat_cost(stat);
        if raise.cost + cost > points {
            break;
        }
        raise.cost += cost;
        raise.amount += 1;
        stat += 1;
    }
    raise
}

/// The next-point trait cost sent in `0x00be` `SP_U<trait>`: 1 below `cap`, else 0 (always 0
/// for a non-trait job, whose cap is 0).
pub fn next_trait_cost(current: u16, cap: u16) -> u8 {
    u8::from(current < cap)
}

/// Raises a trait at `current` by up to `requested`, one point per level, stopping at `cap` or
/// when `points` run out. Zero when nothing can be raised.
pub fn raise_trait(current: u16, requested: u16, points: u32, cap: u16) -> StatRaise {
    let room = cap.saturating_sub(current);
    let affordable = u16::try_from(points).unwrap_or(u16::MAX);
    let amount = requested.min(room).min(affordable);
    StatRaise {
        amount,
        cost: u32::from(amount),
    }
}
