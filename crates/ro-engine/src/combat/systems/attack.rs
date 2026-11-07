//! Attack requests: start, cancel, stop and range.

use ro_core::ServerEvent;

use crate::combat::CombatTimer;
use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::movement::battle_check_range;
use crate::protocol_consts::{FAIL_SKILL_INTERVAL, HIDE_SKILL_INTERVAL};

/// `0x0110` skill id for a request inside `canact` (`NV_BASIC`).
const SKILL_NV_BASIC: u16 = 1;

impl MapInstance {
    /// `CZ_REQUEST_ACT` action 0 (one swing) or 7 (`continuous`), spec §2.2.
    pub(crate) fn request_attack(
        &mut self,
        e: Entity,
        target_gid: u32,
        continuous: bool,
        now: u64,
    ) {
        if self.is_dead(e) {
            return self.refuse_dead(e);
        }
        self.stop_attack(e);
        // A move request handled earlier this tick walks first (it would otherwise be planned
        // after this attack and drop it).
        self.plan_move(e, now);
        let canact_at = self.world.attacks.get(e).map_or(0, |state| state.canact_at);
        if now < canact_at {
            if self.battle.display_skill_fail & HIDE_SKILL_INTERVAL == 0 {
                self.emit_self(
                    e,
                    ServerEvent::SkillFail {
                        skill_id: SKILL_NV_BASIC,
                        cause: FAIL_SKILL_INTERVAL,
                    },
                );
            }
            return;
        }
        let target = self
            .world
            .entity_by_gid(target_gid)
            .filter(|&t| self.attackable(t));
        match target {
            Some(target) => self.attack(e, target, continuous, false, now),
            None => self.drop_attack(e),
        }
    }

    /// `CZ_CANCEL_LOCKON` (`0x0118`): stop the loop and clear `continuous`; a pending step
    /// action stays (catalog §7.2). No reply.
    pub(crate) fn cancel_attack(&mut self, e: Entity) {
        self.stop_attack(e);
        if let Some(state) = self.world.attacks.get_mut(e) {
            state.continuous = false;
        }
    }

    /// rAthena `unit_stop_attack`: no target, no armed swing. Sends nothing.
    pub(crate) fn stop_attack(&mut self, e: Entity) {
        if let Some(state) = self.world.attacks.get_mut(e) {
            state.target = None;
            state.swing_gen = state.swing_gen.wrapping_add(1);
        }
    }

    /// rAthena `unit_unattackable`: the stop plus `continuous`, `step_attack` and the step
    /// action cleared. A move request, another action and a warp end the attack this way.
    pub(crate) fn drop_attack(&mut self, e: Entity) {
        self.stop_attack(e);
        if let Some(state) = self.world.attacks.get_mut(e) {
            state.continuous = false;
            state.step_attack = false;
            state.step_target = None;
            state.step_skill = None;
        }
    }

    /// A live mob in the grid (a dying mob has left it).
    pub(crate) fn attackable(&self, target: Entity) -> bool {
        self.world.entities.is_alive(target)
            && self.world.mobs.get(target).is_some_and(|mob| !mob.dying)
            && self.world.in_grid[target.index()]
    }

    /// The weapon range, at least 1.
    pub(crate) fn attack_range(&self, e: Entity) -> u16 {
        self.world
            .players
            .get(e)
            .map_or(1, |info| info.derived.range.max(1))
    }

    /// `0x0088` at the unit's cell to its area, itself included (`clif_fixpos`).
    pub(crate) fn fixpos(&mut self, e: Entity) {
        let (x, y) = self.cell(e);
        let gid = self.gid(e);
        self.emit_aoi(e, false, ServerEvent::UnitStopped { gid, x, y });
    }

    /// rAthena `unit_attack` from setting the target (spec §2.2 steps 5-7).
    pub(crate) fn attack(
        &mut self,
        e: Entity,
        target: Entity,
        continuous: bool,
        step_attack: bool,
        now: u64,
    ) {
        let range = self.attack_range(e);
        let walking = self.world.walking_paths.contains(e);
        let in_reach = !walking
            || battle_check_range(
                &self.walkability,
                self.cell(e),
                self.cell(target),
                range.saturating_sub(1),
            );
        let state = self.world.attacks.entry(e);
        state.target = Some(target);
        state.continuous = continuous;
        state.step_attack = step_attack;
        state.swing_gen = state.swing_gen.wrapping_add(1);
        state.step_target = None;
        state.step_skill = None;
        state.step_gen = state.step_gen.wrapping_add(1);
        if !in_reach {
            state.step_target = Some(target);
            return;
        }
        if state.attackable_at > now {
            // Deferred: a bare swing at `attackable_at`, nothing now (captured).
            let (due, generation) = (state.attackable_at, state.swing_gen);
            self.combat_timers.push(
                due,
                CombatTimer::Swing {
                    attacker: e,
                    generation,
                },
            );
            return;
        }
        self.fixpos(e);
        self.swing_check(e, now, true);
    }
}
