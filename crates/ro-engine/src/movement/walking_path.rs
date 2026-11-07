//! Walking state and step timing.

use smallvec::SmallVec;

use super::MAX_WALKPATH;

/// rAthena `MOVE_COST` / `MOVE_DIAGONAL_COST`.
pub(crate) const MOVE_COST: u64 = 10;
pub(crate) const MOVE_DIAGONAL_COST: u64 = 14;

/// Milliseconds one step takes: `speed` orthogonal, `speed * 14 / 10` diagonal (integer math).
pub(crate) fn step_duration_ms(speed: u16, diagonal: bool) -> u64 {
    let speed = u64::from(speed);
    if diagonal {
        speed * MOVE_DIAGONAL_COST / MOVE_COST
    } else {
        speed
    }
}

/// An in-progress walk. The unit stays on its current cell until `next_step_at`, when it
/// moves to `next_cell()` (the cell changes at the END of a step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WalkingPath {
    cells: SmallVec<[(u16, u16); MAX_WALKPATH]>,
    /// Index of the next cell to enter.
    pos: usize,
    /// When the in-flight step ends.
    pub(crate) next_step_at: u64,
    /// A validated redirect target, applied when the in-flight step ends.
    pub(crate) pending_target: Option<(u16, u16)>,
}

impl WalkingPath {
    /// Starts a walk from `from` along `cells` (non-empty), with the first step at `start_ms`.
    pub(crate) fn new(cells: &[(u16, u16)], start_ms: u64, speed: u16, from: (u16, u16)) -> Self {
        debug_assert!(!cells.is_empty(), "a walk needs at least one step");
        let mut path = Self {
            cells: SmallVec::from_slice(cells),
            pos: 0,
            next_step_at: start_ms,
            pending_target: None,
        };
        path.schedule(start_ms, speed, from);
        path
    }

    /// Arms the timer for the step from `from` into the next cell, starting at `start_ms`.
    fn schedule(&mut self, start_ms: u64, speed: u16, from: (u16, u16)) {
        if let Some(next) = self.next_cell() {
            let diagonal = next.0 != from.0 && next.1 != from.1;
            self.next_step_at = start_ms + step_duration_ms(speed, diagonal);
        }
    }

    pub(crate) fn next_cell(&self) -> Option<(u16, u16)> {
        self.cells.get(self.pos).copied()
    }

    /// Final cell of the walk.
    pub(crate) fn dest(&self) -> (u16, u16) {
        self.cells.last().copied().unwrap_or((0, 0))
    }

    /// Completes the in-flight step (the unit is now at `reached`) and arms the next one,
    /// which starts when this one ended.
    pub(crate) fn advance(&mut self, speed: u16, reached: (u16, u16)) {
        self.pos += 1;
        let ended_at = self.next_step_at;
        self.schedule(ended_at, speed, reached);
    }
}

#[cfg(test)]
mod tests;
