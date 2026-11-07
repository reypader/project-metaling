//! The per-map combat timer heap, soonest first.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use crate::combat::CombatTimer;

#[derive(Debug)]
struct TimerEntry {
    due: u64,
    /// Insertion order: timers due at the same ms run first-in, first-out.
    seq: u64,
    timer: CombatTimer,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        (self.due, self.seq) == (other.due, other.seq)
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.due, self.seq).cmp(&(other.due, other.seq))
    }
}

/// The per-map combat timer heap, soonest first.
#[derive(Debug, Default)]
pub(crate) struct CombatTimers {
    heap: BinaryHeap<Reverse<TimerEntry>>,
    next_seq: u64,
}

impl CombatTimers {
    pub(crate) fn push(&mut self, due: u64, timer: CombatTimer) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(Reverse(TimerEntry { due, seq, timer }));
    }

    pub(crate) fn pop_due(&mut self, now: u64) -> Option<(u64, CombatTimer)> {
        if self.heap.peek().is_some_and(|Reverse(t)| t.due <= now) {
            self.heap.pop().map(|Reverse(t)| (t.due, t.timer))
        } else {
            None
        }
    }
}
