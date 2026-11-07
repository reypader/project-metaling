//! Draining the combat timer heap each tick, including the cast-end arm.

use crate::combat::CombatTimer;
use crate::map_instance::MapInstance;

impl MapInstance {
    /// Runs every combat timer due by `now`, in due order (timers they arm for `<= now` too).
    pub(crate) fn run_combat_timers(&mut self, now: u64) {
        while let Some((due, timer)) = self.combat_timers.pop_due(now) {
            match timer {
                CombatTimer::Swing {
                    attacker,
                    generation,
                } => {
                    let armed = self
                        .world
                        .attacks
                        .get(attacker)
                        .is_some_and(|s| s.swing_gen == generation && s.target.is_some());
                    if armed && self.world.entities.is_alive(attacker) {
                        self.swing_check(attacker, due, false);
                    }
                }
                CombatTimer::StepAction {
                    attacker,
                    generation,
                } => {
                    if self.world.entities.is_alive(attacker) {
                        self.fire_step_action(attacker, generation, due);
                    }
                }
                CombatTimer::Hit(hit) => self.apply_hit(&hit, due),
                CombatTimer::CastEnd { caster, generation } => {
                    if self.world.entities.is_alive(caster) {
                        self.cast_end(caster, generation, due);
                    }
                }
                CombatTimer::Vanish { mob, cell } => {
                    if self.world.entities.is_alive(mob) {
                        self.vanish_dead_mob(mob, cell);
                    }
                }
            }
        }
    }
}
