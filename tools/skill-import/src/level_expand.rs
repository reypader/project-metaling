//! Expanding rAthena's per-level lists to one value per level.

use std::collections::HashMap;
use std::fmt;

use ro_db::MAX_SKILL_LEVEL;

use crate::SLOTS;

/// `(max base level, max job level)` per job id: tree `BaseLevel`/`JobLevel` are capped to
/// them.
pub type MaxLevels = HashMap<u16, (u16, u16)>;

/// Expansion error: a listed level outside `1..=MAX_SKILL_LEVEL` (rAthena drops the skill).
#[derive(Debug)]
pub struct BadLevel(pub u16);

impl fmt::Display for BadLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "per-level entry at Level {} (1..={MAX_SKILL_LEVEL})",
            self.0
        )
    }
}

/// rAthena's per-level list expansion (catalog `db/skills.md` "List form expansion"). Each
/// `(level, value)` sets that slot; `m` is the highest listed level. Slots above `m` continue
/// the list: the first period `step` in `1..=m/2` for which every `v[k] - v[k-step]` over
/// `step+1..=m` is the same difference `d` gives `v[k] = v[k-step] + d` (a value that drops
/// below 1 after a value of 0 or more becomes 1, and so does every later one); with no such
/// period the value at `m` repeats. Unlisted slots up to `m` stay 0.
pub fn expand(listed: &[(u16, i32)]) -> Result<[i32; SLOTS], BadLevel> {
    // 1-based: v[0] is unused.
    let mut v = [0i32; SLOTS + 1];
    let mut m = 0;
    for &(level, value) in listed {
        let slot = usize::from(level);
        if !(1..=SLOTS).contains(&slot) {
            return Err(BadLevel(level));
        }
        v[slot] = value;
        m = m.max(slot);
    }
    if m == 0 {
        return Ok([0; SLOTS]);
    }
    let period = (1..=m / 2).find_map(|step| {
        let d = v[m] - v[m - step];
        ((step + 1)..=m)
            .all(|k| v[k] - v[k - step] == d)
            .then_some((step, d))
    });
    match period {
        Some((mut step, mut d)) => {
            for k in (m + 1)..=SLOTS {
                v[k] = v[k - step] + d;
                if v[k] < 1 && v[k - 1] >= 0 {
                    v[k] = 1;
                    step = 1;
                    d = 0;
                }
            }
        }
        None => {
            for k in (m + 1)..=SLOTS {
                v[k] = v[m];
            }
        }
    }
    let mut out = [0; SLOTS];
    out.copy_from_slice(&v[1..]);
    Ok(out)
}
