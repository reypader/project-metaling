use super::*;

/// The Novice job bars 1..=4 (`job_exp.yml`), max job level 10.
fn novice_job(level: u16) -> u64 {
    [10, 18, 28, 40, 91]
        .get(usize::from(level) - 1)
        .copied()
        .unwrap_or(0)
}

/// Captured clamp (`experience.md` "Job-exp leftover clamp"): 0 + 40 at job 1 leaves 9, not
/// 30; 27 + 40 at job 4 leaves 27 (below the clamp).
#[test]
fn leftover_clamps_to_the_consumed_bar() {
    let clamp = |_| true;
    assert_eq!(level_up(1, 40, 10, 999, clamp, novice_job), Some((2, 9)));
    assert_eq!(level_up(4, 67, 10, 999, clamp, novice_job), Some((5, 27)));
    assert_eq!(
        level_up(1, 9, 10, 999, clamp, novice_job),
        None,
        "below the bar"
    );
}

#[test]
fn multi_level_up_chains_and_stops_at_the_max() {
    let carry = |_| false;
    // 10 + 18 + 28 = 56: three levels, 4 left (the next bar is 40).
    assert_eq!(level_up(1, 60, 10, 999, carry, novice_job), Some((4, 4)));
    // A missing row (0) stops the loop.
    assert_eq!(
        level_up(1, 1_000, 10, 999, carry, novice_job),
        Some((6, 1_000 - 187))
    );
    assert_eq!(
        level_up(1, 1_000, 3, 5, carry, novice_job),
        Some((3, 5)),
        "max"
    );
    assert_eq!(
        level_up(3, 1_000, 3, 5, carry, novice_job),
        None,
        "already max"
    );
}

#[test]
fn exp_stops_at_the_cap_only_at_the_max_level() {
    assert_eq!(add_exp(10, 40, false, 20), 50);
    assert_eq!(add_exp(10, 40, true, 20), 20);
    assert_eq!(add_exp(30, 40, true, 20), 30, "never lowered");
    assert_eq!(add_exp(MAX_EXP - 1, 40, false, 0), MAX_EXP);
}
