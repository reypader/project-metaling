//! Casting a skill on a unit and applying its effects (spec `specs/effects/spec.md` §2.2-§2.6;
//! catalog `scripting/skills.md` §3-§10, §14-§15).
//!
//! A request (`0x0438`) runs the silent checks, the target and range checks (or stores the step
//! action), then the cast-begin requirements. Cast begin stops the attack loop (and a walk, with
//! `0x0088`), sends `0x0b1a` and sets the gates. Castend re-checks the target, resolves the
//! definition through `ro-scripting` and applies the effects in order: the costs, `0x0983` for a
//! skill with an after-cast delay, then heals and weapon hits. With no cast time (every skill
//! in the slice) castend runs in the same tick.

use std::fmt;
use std::mem;
use std::sync::Arc;

use ro_core::ServerEvent;
use ro_db::{Element, SkillData, WeaponType};
use ro_mechanics::{calc_weapon_attack, cast_time, delayfix, AttackContext, CastTimeData, Caster};
use ro_scripting::{
    CastSource, Definitions, Effect, HealShow, HookBackend, HookCtx, MissingHook, Roll, RustHooks,
    SkillLevels, UnitView, Who,
};
use tracing::warn;

use crate::combat::{
    attacker_view, CombatTimer, PendingCast, StepSkill, FAIL_SKILL_INTERVAL, HIDE_SKILL_INTERVAL,
};
use crate::engine::{MapInstance, SITTING};
use crate::entity::Entity;
use crate::job::{class_flags, job_mapid};
use crate::path::battle_check_range;
use crate::rng::SplitMix64;
use crate::skills::{range2, SkillSet};
use crate::status::battle_stats;

// rAthena `inf` bits (the data's target type).
const INF_ATTACK: u8 = 1;
const INF_GROUND: u8 = 2;
const INF_SELF: u8 = 4;

// `0x0110` causes (`USESKILL_FAIL_*`).
const FAIL_LEVEL: u8 = 0;
const FAIL_SP: u8 = 1;
const FAIL_HP: u8 = 2;
const FAIL_MONEY: u8 = 5;
const FAIL_WEAPON: u8 = 6;
const FAIL_WEIGHT: u8 = 9;
/// rAthena `major_overweight_rate`: at this weight percentage skills are refused.
const OVERWEIGHT_PERCENT: u64 = 90;

/// `EFST_POSTDELAY`: the after-cast delay icon.
const EFST_POSTDELAY: u16 = 46;
/// `0x01de` type of every weapon-skill hit (`DMG_MULTI_HIT`, also on a miss).
const SKILL_DAMAGE_TYPE: u8 = 8;
/// rAthena `min_skill_delay_limit` and `SECURITY_CASTTIME` (ms).
const MIN_CAST_DELAY: u64 = 100;
const SECURITY_CASTTIME: u64 = 100;
/// `0x0b1a` sends a skill element only when it is a plain one (`ELE_NONE < ele < ELE_ALL`).
const MAX_PLAIN_ELEMENT: u8 = 9;

/// The skill definitions and the hook backend their hook ids belong to (spec §2.1). They are
/// built together, so the ids always match the backend.
#[derive(Clone, Default)]
pub struct SkillScripts {
    /// `None`: nothing can be cast (the default, for maps and tests that don't need skills).
    definitions: Option<Arc<Definitions>>,
    hooks: Option<Arc<dyn HookBackend + Send + Sync>>,
}

impl SkillScripts {
    /// The built-in definitions bound to `hooks`.
    pub fn with_backend(hooks: Arc<dyn HookBackend + Send + Sync>) -> Result<Self, MissingHook> {
        let definitions = Definitions::builtin(&*hooks)?;
        Ok(Self {
            definitions: Some(Arc::new(definitions)),
            hooks: Some(hooks),
        })
    }

    /// The built-in definitions with the Rust hooks.
    pub fn builtin() -> Result<Self, MissingHook> {
        Self::with_backend(Arc::new(RustHooks::builtin()))
    }

    pub(crate) fn get(&self) -> Option<(Arc<Definitions>, Arc<dyn HookBackend + Send + Sync>)> {
        Some((
            Arc::clone(self.definitions.as_ref()?),
            Arc::clone(self.hooks.as_ref()?),
        ))
    }
}

impl fmt::Debug for SkillScripts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillScripts")
            .field("definitions", &self.definitions)
            .field("hooks", &self.hooks.as_ref().map(|_| "HookBackend"))
            .finish()
    }
}

/// A hook reads the caster's visible skill levels (hidden slots count toward nothing).
impl SkillLevels for SkillSet {
    fn level(&self, skill: u16) -> u8 {
        self.get(skill)
            .filter(|slot| slot.visible)
            .map_or(0, |slot| slot.level)
    }
}

/// The map's seeded RNG as a hook's roll source.
pub(crate) struct MapRoll<'a>(pub(crate) &'a mut SplitMix64);

impl Roll for MapRoll<'_> {
    fn roll(&mut self, n: u32) -> u32 {
        u32::try_from(self.0.below(u64::from(n))).unwrap_or(0)
    }
}

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

    fn weapon_allowed(&self, e: Entity, data: &SkillData) -> bool {
        let weapon = self
            .world
            .players
            .get(e)
            .and_then(|info| info.gear.right)
            .map_or(WeaponType::Fist, |w| w.weapon_type);
        data.allows_weapon(weapon)
    }

    fn skill_fail(&mut self, e: Entity, skill_id: u16, cause: u8) {
        self.emit_self(e, ServerEvent::SkillFail { skill_id, cause });
    }

    /// A unit a skill can aim at: a live, visible player or an attackable mob.
    fn castable_target(&self, t: Entity) -> bool {
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

    /// Cast begin (spec §2.3): stop the attack and any walk, `0x0b1a`, the gates, then castend
    /// now or at the end of the cast time.
    fn cast_begin(&mut self, e: Entity, data: &SkillData, level: u8, target: Entity, now: u64) {
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

    /// Applies one resolved effect of `e`'s cast (or item use) on `target`.
    pub(crate) fn apply_effect(
        &mut self,
        e: Entity,
        target: Entity,
        skill_id: u16,
        level: u8,
        effect: Effect,
        now: u64,
    ) {
        match effect {
            Effect::SpendSp(amount) => self.spend_sp(e, amount),
            Effect::SpendHp(amount) => self.spend_hp(e, amount),
            Effect::SpendZeny(amount) => {
                let zeny = self.zeny(e).saturating_sub(amount);
                self.set_zeny(e, zeny);
            }
            Effect::ConsumeItem { item, .. } => {
                warn!(map = %self.map_name, skill = skill_id, item = item.0, "item costs are not applied yet");
            }
            Effect::Heal { to, hp, sp, show } => {
                let unit = match to {
                    Who::Caster => e,
                    Who::Target => target,
                };
                self.apply_heal(e, unit, skill_id, hp, sp, show);
            }
            Effect::WeaponDamage {
                ratio,
                hit_pct,
                div,
            } => self.weapon_skill_hit(e, target, skill_id, level, ratio, hit_pct, div, now),
        }
    }

    /// A heal from `caster` on `unit` (spec §2.5): `0x09cb` with the unclamped amount to the
    /// unit's area, then the player's own HP/SP pushes, or a mob's HP and its HP bar.
    fn apply_heal(
        &mut self,
        caster: Entity,
        unit: Entity,
        skill_id: u16,
        hp: u32,
        sp: u32,
        show: HealShow,
    ) {
        if show == HealShow::Skill {
            let (src, target) = (self.gid(caster), self.gid(unit));
            self.emit_aoi(
                unit,
                false,
                ServerEvent::SkillNoDamage {
                    skill_id,
                    value: i32::try_from(hp).unwrap_or(i32::MAX),
                    target,
                    src,
                    ok: true,
                },
            );
        }
        if self.world.players.contains(unit) {
            self.heal_player(unit, hp, sp);
        } else if let Some(mob) = self.world.mobs.get_mut(unit) {
            mob.hp = mob.hp.saturating_add(hp).min(mob.max_hp);
            if self.battle.monster_hp_bars_info {
                self.send_hp_bar(unit);
            }
        }
    }

    /// A weapon skill on a mob (spec §2.6): the shared damage roll with the skill context,
    /// `0x01de` to the target's area, and the hit at `now + amotion`. A miss queues nothing.
    #[allow(clippy::too_many_arguments)]
    fn weapon_skill_hit(
        &mut self,
        e: Entity,
        target: Entity,
        skill_id: u16,
        level: u8,
        ratio: i32,
        hit_pct: i32,
        div: i16,
        now: u64,
    ) {
        let (Some(info), Some(mob)) = (self.world.players.get(e), self.world.mobs.get(target))
        else {
            return;
        };
        let attacker = attacker_view(info, 0, false);
        let amotion = info.derived.amotion;
        let (src, dst) = (info.account_id.0, mob.gid);
        let dmotion = mob.stats.status.dmotion;
        let result = calc_weapon_attack(
            &attacker,
            &mob.stats.defender,
            &AttackContext::skill(ratio, hit_pct),
            &self.battle,
            |n| u32::try_from(self.rng.below(u64::from(n))).unwrap_or(0),
        );
        let hit = result.kind.is_hit();
        self.emit_aoi(
            target,
            false,
            ServerEvent::SkillDamage {
                skill_id,
                src,
                dst,
                tick: now as u32,
                src_speed: amotion,
                dmg_speed: if hit { dmotion } else { 0 },
                damage: result.damage,
                level: i16::from(level),
                div,
                damage_type: SKILL_DAMAGE_TYPE,
            },
        );
        if hit {
            self.queue_hit(e, target, result.damage, amotion, now);
        }
    }

    /// What a hook sees of a unit (DESIGN §2): battle stats, vitals, and for a player the MATK
    /// parts Heal reads (`status_matk + 5·SPL`, the right-hand weapon's MATK and level).
    pub(crate) fn unit_view(&self, u: Entity) -> UnitView {
        if let Some(info) = self.world.players.get(u) {
            let derived = &info.derived;
            let [str, agi, vit, int, dex, luk] = battle_stats(derived);
            let weapon = info.gear.right;
            return UnitView {
                base_level: info.clevel,
                job: info.job,
                str,
                agi,
                vit,
                int,
                dex,
                luk,
                hp: info.status.hp,
                max_hp: info.status.max_hp,
                sp: info.status.sp,
                max_sp: info.status.max_sp,
                matk_base: derived.status_matk + 5 * derived.trait_battle.spl,
                weapon_matk: weapon.map_or(0, |w| i32::try_from(w.matk).unwrap_or(i32::MAX)),
                weapon_level: weapon.map_or(0, |w| w.level),
            };
        }
        let Some(mob) = self.world.mobs.get(u) else {
            return UnitView::default();
        };
        let status = &mob.stats.status;
        let [str, agi, vit, int, dex, luk] = battle_stats(status);
        UnitView {
            base_level: mob.level,
            str,
            agi,
            vit,
            int,
            dex,
            luk,
            hp: mob.hp,
            max_hp: mob.max_hp,
            ..UnitView::default()
        }
    }
}
