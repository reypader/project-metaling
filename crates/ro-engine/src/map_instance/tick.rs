//! The tick schedule, driven by the map-server actor. All times are the actor's monotonic
//! `now_ms`; wire ticks are `now_ms as u32`.

use std::mem;

use ro_core::ScopedServerEvent;

use super::MapInstance;
use crate::error::EngineError;

impl MapInstance {
    /// Runs one tick and appends its events to `out` (the caller reuses the buffer).
    /// Bad per-intent input is logged and dropped; `Err` only for engine-wide faults.
    pub fn step_tick(
        &mut self,
        now_ms: u64,
        out: &mut Vec<ScopedServerEvent>,
    ) -> Result<(), EngineError> {
        self.expire_floor_items(now_ms);
        // Steps that ended before `now` complete first, so this tick's clicks see them.
        self.advance_walkers(now_ms);
        // Swings, step actions and hits due by now, at their own due times.
        self.run_combat_timers(now_ms);
        let mut batch = mem::take(&mut self.intents);
        for intent in batch.drain(..) {
            self.handle_intent(intent, now_ms);
        }
        self.intents = batch;
        self.plan_moves(now_ms);
        // Stages 3 and 4: one recalc per dirty player, after every mutation of the tick.
        self.run_stat_pass();
        out.append(&mut self.events);
        Ok(())
    }
}
