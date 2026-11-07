//! Skill requests: the silent checks, target and range checks, and the cast-begin requirements.

use std::sync::Arc;

use ro_core::ServerEvent;
use ro_db::{Element, SkillData, WeaponType};
use ro_scripting::SkillLevels;

use crate::cast::cast_consts::{
    FAIL_HP, FAIL_MONEY, FAIL_SP, FAIL_WEAPON, FAIL_WEIGHT, INF_GROUND, INF_SELF,
};
use crate::combat::StepSkill;
use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::movement::battle_check_range;
use crate::protocol_consts::{FAIL_SKILL_INTERVAL, HIDE_SKILL_INTERVAL, SITTING};
use crate::skill::range2;

/// rAthena `major_overweight_rate`: at this weight percentage skills are refused.
const OVERWEIGHT_PERCENT: u64 = 90;

impl MapInstance {
    /// `CZ_USE_SKILL_TO_ID` (`0x0438`, spec §2.2). Also the step action's re-run.
    pub(crate) fn use_skill(
        &mut self,
        e: Entity,
        skill_id: u16,
        requested: u16,
        target_gid: u32,
        now: u64,
    ) {
        // 1. A state that can't act, or a cast in progress.
        if self.is_dead(e)
            || self.world.dead_sit[e.index()] == SITTING
            || self.world.attacks.get(e).is_some_and(|s| s.cast.is_some())
        {
            return;
        }
        // 2. A skill we can cast.
        let Some((definitions, _)) = self.scripts.get() else {
            return;
        };
        let Some(def) = definitions.skill(skill_id) else {
            return;
        };
        let skills = Arc::clone(&self.skills);
        let Some(data) = skills.get(skill_id) else {
            return;
        };
        if data.is_passive() || data.inf & INF_GROUND != 0 {
            return;
        }
        // 3. The learned level (visible slots only); the request is clamped to it.
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let learned = SkillLevels::level(&info.skills, skill_id);
        if learned == 0 {
            return;
        }
        let level = learned.min(u8::try_from(requested.max(1)).unwrap_or(u8::MAX));
        // A move request handled earlier this tick walks first, as for an attack.
        self.plan_move(e, now);
        // 4. Too early: silent at the default `display_skill_fail`.
        let canact_at = self.world.attacks.get(e).map_or(0, |s| s.canact_at);
        if now < canact_at {
            if self.battle.display_skill_fail & HIDE_SKILL_INTERVAL == 0 {
                self.skill_fail(e, skill_id, FAIL_SKILL_INTERVAL);
            }
            return;
        }
        // 5. The target.
        let target = if data.inf & INF_SELF != 0 {
            e
        } else {
            match self.world.entity_by_gid(target_gid) {
                Some(t) if self.castable_target(t) => t,
                _ => return,
            }
        };
        if def.target.reject_undead && self.is_undead_mob(target) {
            return;
        }
        // 6. The cast-begin requirements, in rAthena's order: weight, HP, weapon, SP, zeny
        // (catalog §6). They come before the range check: out of range with too little SP
        // still gets the refusal.
        if let Some(cause) = self.unmet_requirement(e, data, level) {
            return self.skill_fail(e, skill_id, cause);
        }
        if data.item_cost(level).next().is_some() {
            // No slice skill costs items; resolving them is deferred.
            return;
        }
        // 7. Any pending step action ends (rAthena `unit_stop_stepaction`); then the range,
        // or mid-walk and out of `range - 1`, the step action instead.
        self.clear_step_action(e);
        if target != e {
            let Some(info) = self.world.players.get(e) else {
                return;
            };
            let range = range2(data, level, &info.skills, &skills);
            let (me, them) = (self.cell(e), self.cell(target));
            if self.world.walking_paths.contains(e)
                && !battle_check_range(&self.walkability, me, them, range.saturating_sub(1))
            {
                self.set_step_skill(
                    e,
                    target,
                    StepSkill {
                        skill: skill_id,
                        level: requested,
                    },
                );
                return;
            }
            if !battle_check_range(&self.walkability, me, them, range) {
                return;
            }
        }
        self.cast_begin(e, data, level, target, now);
    }

    /// The first unmet cast-begin requirement's `0x0110` cause.
    fn unmet_requirement(&self, e: Entity, data: &SkillData, level: u8) -> Option<u8> {
        let info = self.world.players.get(e)?;
        if self.world.inventories.get(e).is_some_and(|inventory| {
            let max = u64::from(inventory.max_weight());
            max > 0 && u64::from(inventory.weight()) * 100 / max >= OVERWEIGHT_PERCENT
        }) {
            return Some(FAIL_WEIGHT);
        }
        let hp_cost = data.hp_cost(level);
        if hp_cost > 0 && info.status.hp <= hp_cost {
            return Some(FAIL_HP);
        }
        if !self.weapon_allowed(e, data) {
            return Some(FAIL_WEAPON);
        }
        if info.status.sp < u32::from(data.sp(level)) {
            return Some(FAIL_SP);
        }
        if info.status.zeny < data.zeny_cost(level) {
            return Some(FAIL_MONEY);
        }
        None
    }

    pub(crate) fn weapon_allowed(&self, e: Entity, data: &SkillData) -> bool {
        let weapon = self
            .world
            .players
            .get(e)
            .and_then(|info| info.gear.right)
            .map_or(WeaponType::Fist, |w| w.weapon_type);
        data.allows_weapon(weapon)
    }

    pub(crate) fn skill_fail(&mut self, e: Entity, skill_id: u16, cause: u8) {
        self.emit_self(e, ServerEvent::SkillFail { skill_id, cause });
    }

    /// A unit a skill can aim at: a live, visible player or an attackable mob.
    pub(crate) fn castable_target(&self, t: Entity) -> bool {
        if !self.world.entities.is_alive(t) || !self.world.in_grid[t.index()] {
            return false;
        }
        if self.world.players.contains(t) {
            return !self.is_dead(t);
        }
        self.attackable(t)
    }

    fn is_undead_mob(&self, t: Entity) -> bool {
        self.world
            .mobs
            .get(t)
            .and_then(|mob| self.mob_db.get(mob.mob_id))
            .is_some_and(|data| data.element == Element::Undead)
    }
}
