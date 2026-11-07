//! The cast pipeline: cast begin, castend and the after-cast delay.

use std::mem;
use std::sync::Arc;

use ro_core::ServerEvent;
use ro_db::SkillData;
use ro_mechanics::{cast_time, delayfix, CastTimeData, Caster};
use ro_scripting::{CastSource, Effect, HookCtx};
use tracing::warn;

use crate::cast::cast_consts::{FAIL_HP, FAIL_LEVEL, FAIL_WEAPON, INF_ATTACK};
use crate::cast::MapRoll;
use crate::combat::{CombatTimer, PendingCast};
use crate::ecs::Entity;
use crate::job::{class_flags, job_mapid};
use crate::map_instance::MapInstance;
use crate::stats::battle_stats;

/// `EFST_POSTDELAY`: the after-cast delay icon.
const EFST_POSTDELAY: u16 = 46;

/// rAthena `min_skill_delay_limit` and `SECURITY_CASTTIME` (ms).
const MIN_CAST_DELAY: u64 = 100;

const SECURITY_CASTTIME: u64 = 100;

/// `0x0b1a` sends a skill element only when it is a plain one (`ELE_NONE < ele < ELE_ALL`).
const MAX_PLAIN_ELEMENT: u8 = 9;

/// The element `0x0b1a` shows: a plain skill element, else 0 (captured: `Weapon` sends 0 even
/// with an elemental weapon or endow; `scripting_2.txt`).
fn casting_element(data: &SkillData, level: u8) -> u32 {
    let code = data.element(level).code();
    if code <= MAX_PLAIN_ELEMENT {
        u32::from(code)
    } else {
        0
    }
}

impl MapInstance {
    /// Cast begin (spec §2.3): stop the attack and any walk, `0x0b1a`, the gates, then castend
    /// now or at the end of the cast time.
    pub(crate) fn cast_begin(
        &mut self,
        e: Entity,
        data: &SkillData,
        level: u8,
        target: Entity,
        now: u64,
    ) {
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let derived = &info.derived;
        let amotion = u64::try_from(derived.amotion).unwrap_or(0);
        let adelay = u64::try_from(derived.adelay).unwrap_or(0);
        let floor = u64::from(self.battle.amotion_floor(&class_flags(job_mapid(info.job))));
        let [_, _, _, int, dex, _] = battle_stats(derived);
        let cast_ms = cast_time(
            CastTimeData {
                cast_time: data.cast_time(level),
                fixed_cast_time: data.fixed_cast_time(level),
                ignore_dex: data.cast_time_flags.ignore_dex,
            },
            Caster::Player { dex, int },
            &self.battle,
        );
        self.stop_attack(e);
        self.stop_walking(e);
        let (src, dst) = (self.gid(e), self.gid(target));
        self.emit_aoi(
            e,
            false,
            ServerEvent::SkillCasting {
                src,
                dst,
                x: 0,
                y: 0,
                skill_id: data.id,
                element: casting_element(data, level),
                delay_ms: cast_ms,
            },
        );
        let cast_delay = u64::from(cast_ms).max(MIN_CAST_DELAY);
        let state = self.world.attacks.entry(e);
        state.attackable_at = state.attackable_at.max(now + adelay);
        state.canact_at = state
            .canact_at
            .max(now + amotion + floor)
            .max(now + cast_delay)
            + SECURITY_CASTTIME;
        state.cast_gen = state.cast_gen.wrapping_add(1);
        state.cast = Some(PendingCast {
            skill: data.id,
            level,
            target,
        });
        let generation = state.cast_gen;
        if cast_ms == 0 {
            self.cast_end(e, generation, now);
        } else {
            self.combat_timers.push(
                now + u64::from(cast_ms),
                CombatTimer::CastEnd {
                    caster: e,
                    generation,
                },
            );
        }
    }

    /// Castend (spec §2.4): re-checks, resolve, `canact`, then the effects in order.
    pub(crate) fn cast_end(&mut self, e: Entity, generation: u32, now: u64) {
        let Some(state) = self.world.attacks.get_mut(e) else {
            return;
        };
        if state.cast_gen != generation {
            return;
        }
        let Some(cast) = state.cast.take() else {
            return;
        };
        let target = cast.target;
        // 1. Re-checks. A target gone or no longer valid drops the cast silently.
        if !self.castable_target(target) {
            return;
        }
        let Some((definitions, hooks)) = self.scripts.get() else {
            return;
        };
        let skills = Arc::clone(&self.skills);
        let (Some(def), Some(data)) = (definitions.skill(cast.skill), skills.get(cast.skill))
        else {
            return;
        };
        let level = cast.level;
        // An attack skill on a player fails here, after its `0x0b1a` (captured,
        // `scripting_2.txt`); the cast-begin gates stay.
        if data.inf & INF_ATTACK != 0 && self.world.players.contains(target) {
            return self.skill_fail(e, cast.skill, FAIL_LEVEL);
        }
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        // `skill_check_condition_castend`: HP then weapon, each with its `0x0110`.
        let hp_cost = data.hp_cost(level);
        let refusal = if hp_cost > 0 && info.status.hp <= hp_cost {
            Some(FAIL_HP)
        } else if !self.weapon_allowed(e, data) {
            Some(FAIL_WEAPON)
        } else {
            None
        };
        if let Some(cause) = refusal {
            return self.skill_fail(e, cast.skill, cause);
        }
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let amotion = info.derived.amotion;

        // 2. Costs first, then the definition's effects. Resolve completely before applying
        // anything: an error clears the list, costs included (ruling 11).
        let mut effects = mem::take(&mut self.effect_buf);
        effects.clear();
        let sp = u32::from(data.sp(level));
        let zeny = data.zeny_cost(level);
        // HP before SP: rAthena's `status_zap` pushes `SP_HP` first.
        if hp_cost > 0 {
            effects.push(Effect::SpendHp(hp_cost));
        }
        if sp > 0 {
            effects.push(Effect::SpendSp(sp));
        }
        if zeny > 0 {
            effects.push(Effect::SpendZeny(zeny));
        }
        let ctx = HookCtx {
            caster: self.unit_view(e),
            target: self.unit_view(target),
            skill: cast.skill,
            level,
            source: CastSource::Learned,
            args: [0; 4],
            skills: &info.skills,
        };
        let resolved = def.resolve(&ctx, &*hooks, &mut MapRoll(&mut self.rng), &mut effects);
        if let Err(err) = resolved {
            warn!(map = %self.map_name, skill = cast.skill, %err, "skill hook failed");
            self.effect_buf = effects;
            return;
        }

        // 3. The after-cast gate.
        let delay = delayfix(
            data.after_cast_act_delay(level),
            u32::try_from(amotion).unwrap_or(0),
        );
        let state = self.world.attacks.entry(e);
        state.canact_at =
            (now + u64::from(delay)).max(state.canact_at.saturating_sub(SECURITY_CASTTIME));

        // 4-5. Apply, with `0x0983` between the costs and the first other effect.
        let mut postdelay_sent = delay == 0;
        for &effect in effects.iter() {
            let spend = matches!(
                effect,
                Effect::SpendSp(_) | Effect::SpendHp(_) | Effect::SpendZeny(_)
            );
            if !spend && !postdelay_sent {
                postdelay_sent = true;
                self.send_postdelay(e, delay);
            }
            self.apply_effect(e, target, cast.skill, level, effect, now);
        }
        if !postdelay_sent {
            self.send_postdelay(e, delay);
        }
        self.effect_buf = effects;
    }

    /// `0x0983` `EFST_POSTDELAY` with the delay, to the caster's area (spec §2.4 step 4).
    fn send_postdelay(&mut self, e: Entity, delay: u32) {
        let gid = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::StatusIconStarted {
                efst: EFST_POSTDELAY,
                gid,
                state: 1,
                total_ms: delay,
                remain_ms: i32::try_from(delay).unwrap_or(i32::MAX),
                values: [0; 3],
            },
        );
    }
}
