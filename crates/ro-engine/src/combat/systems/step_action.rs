//! Step actions: an attack or skill armed to fire when a walk step ends.

use crate::combat::rules::chebyshev;
use crate::combat::{CombatTimer, StepSkill};
use crate::ecs::{Entity, AREA_SIZE};
use crate::map_instance::MapInstance;
use crate::movement::{MOVE_COST, MOVE_DIAGONAL_COST};

impl MapInstance {
    /// Stores a skill request as the step action on `target` (rAthena `stepaction` with
    /// `stepskill_id`), replacing any pending one: it fires half a step after the next step
    /// ends.
    pub(crate) fn set_step_skill(&mut self, e: Entity, target: Entity, step: StepSkill) {
        let state = self.world.attacks.entry(e);
        state.step_target = Some(target);
        state.step_skill = Some(step);
        state.step_gen = state.step_gen.wrapping_add(1);
    }

    /// rAthena `unit_stop_stepaction`: no pending step action, and an armed one goes stale.
    pub(crate) fn clear_step_action(&mut self, e: Entity) {
        if let Some(state) = self.world.attacks.get_mut(e) {
            state.step_target = None;
            state.step_skill = None;
            state.step_gen = state.step_gen.wrapping_add(1);
        }
    }

    /// After a completed walk step of a unit with a pending step action: fire it half a step
    /// later (catalog §7.2).
    pub(crate) fn arm_step_action(&mut self, e: Entity, diagonal: bool, step_end: u64) {
        let Some(state) = self.world.attacks.get_mut(e) else {
            return;
        };
        if state.step_target.is_none() {
            return;
        }
        state.step_gen = state.step_gen.wrapping_add(1);
        let generation = state.step_gen;
        let speed = u64::from(self.world.speed[e.index()]);
        let delay = if diagonal {
            speed * MOVE_DIAGONAL_COST / MOVE_COST / 2
        } else {
            speed / 2
        };
        self.combat_timers.push(
            step_end + delay,
            CombatTimer::StepAction {
                attacker: e,
                generation,
            },
        );
    }

    /// rAthena `unit_step_timer`: the target must still be in view (14 cells), then the attack
    /// request runs again from the target step with `step_attack`.
    pub(crate) fn fire_step_action(&mut self, e: Entity, generation: u32, due: u64) {
        let Some(state) = self.world.attacks.get_mut(e) else {
            return;
        };
        if state.step_gen != generation {
            return;
        }
        let Some(target) = state.step_target.take() else {
            return;
        };
        let continuous = state.continuous;
        if let Some(step) = state.step_skill.take() {
            // The cast request runs again from the top (rAthena `unit_skilluse_id`).
            if self.world.entities.is_alive(target) {
                let target_gid = self.gid(target);
                self.use_skill(e, step.skill, step.level, target_gid, due);
            }
            return;
        }
        if !self.attackable(target) || chebyshev(self.cell(e), self.cell(target)) > AREA_SIZE {
            return;
        }
        self.attack(e, target, continuous, true, due);
    }
}
