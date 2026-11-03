//! Player auto-attack: the request, the swing loop, ammo and delayed hits (spec
//! `specs/combat/spec.md` §2.2-§2.7; catalog `mechanics/auto_attack.md`).
//!
//! One per-map min-heap holds every combat timer (swings, step actions, delayed hits, death
//! vanishes). It is
//! popped in `step_tick` after the walkers and before the intents. Swing times come from the
//! timer's due time, never the tick's `now`, so a 20 ms tick never stretches the interval.
//! Stale timers are skipped: a swing or step action carries the generation it was armed with,
//! and a hit re-checks both units when it fires.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use ro_core::{CharId, ServerEvent};
use ro_db::{AmmoType, WeaponType};
use ro_mechanics::{
    calc_weapon_attack, check_distance_client, exp_share, shared_exp, AttackContext, AttackerView,
    WeaponAttack,
};
use smallvec::SmallVec;
use tracing::warn;

use crate::engine::{MapInstance, SP_WEIGHT, VANISH_DIED};
use crate::entity::Entity;
use crate::equipment::EQP_AMMO;
use crate::inventory::wire_index;
use crate::job::{class_flags, job_mapid};
use crate::path::{battle_check_range, line_of_sight};
use crate::spatial::AREA_SIZE;
use crate::stats::weapon_class;

/// `0x0110` for a request inside `canact`: `NV_BASIC`, `USESKILL_FAIL_SKILLINTERVAL`.
const SKILL_NV_BASIC: u16 = 1;
pub(crate) const FAIL_SKILL_INTERVAL: u8 = 4;
/// `0x0110` for a gun with no bullets: skill 0, `USESKILL_FAIL_NEED_MORE_BULLET`.
const FAIL_NEED_MORE_BULLET: u8 = 84;
/// `0x013b` `ARROWFAIL_NO_AMMO`.
const ARROW_FAIL_NO_AMMO: u16 = 0;
/// `display_skill_fail` bit that hides the skill-interval refusal.
pub(crate) const HIDE_SKILL_INTERVAL: u32 = 2;
/// `0x07fa` delete type of spent ammo.
const DELETE_SPENT: u16 = 1;
/// A diagonal step costs 14/10 of a straight one.
const DIAGONAL_COST: u64 = 14;
const STRAIGHT_COST: u64 = 10;
/// rAthena `DAMAGELOG_SIZE`: the oldest attacker drops out past this many.
const DAMAGE_LOG_SIZE: usize = 20;
/// The death vanish follows the killing hit by this much (catalog `mobs.md` §3 step 11).
const DEATH_VANISH_DELAY: u64 = 250;

/// One attacker's total damage on a mob, overkill included (rAthena `s_dmglog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DamageEntry {
    pub(crate) char_id: CharId,
    pub(crate) damage: u64,
}

/// A mob's damage log, oldest first, at most `DAMAGE_LOG_SIZE` attackers (spec §2.7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DamageLog(SmallVec<[DamageEntry; 4]>);

impl DamageLog {
    /// Adds to the attacker's entry, or appends one, evicting the oldest when full.
    pub(crate) fn add(&mut self, char_id: CharId, damage: u64) {
        if let Some(entry) = self.0.iter_mut().find(|e| e.char_id == char_id) {
            entry.damage = entry.damage.saturating_add(damage);
            return;
        }
        if self.0.len() >= DAMAGE_LOG_SIZE {
            self.0.remove(0);
        }
        self.0.push(DamageEntry { char_id, damage });
    }

    pub(crate) fn entries(&self) -> &[DamageEntry] {
        &self.0
    }
}

/// rAthena's per-unit attack state (`ud.target`, `attack_continue`, `step_attack`,
/// `attackabletime`, `canact_tick`, `target_to`). Kept after an attack stops.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AttackState {
    target: Option<Entity>,
    /// Action 7: re-arm after each swing.
    continuous: bool,
    /// The attack came from a step action: range + 1.
    step_attack: bool,
    /// The next swing may start here (ms).
    pub(crate) attackable_at: u64,
    /// A fresh request (attack or skill) is accepted from here (ms).
    pub(crate) canact_at: u64,
    /// The pending step action's target (rAthena `target_to`).
    pub(crate) step_target: Option<Entity>,
    /// The pending step action is this skill cast instead of an attack (rAthena
    /// `stepskill_id`/`stepskill_lv`).
    pub(crate) step_skill: Option<StepSkill>,
    /// Bumped when the loop stops or restarts; an armed swing with an older value is stale.
    swing_gen: u32,
    /// Bumped when a step action is armed or replaced.
    step_gen: u32,
    /// The cast between cast begin and castend (rAthena `ud.skill_id`/`skilltarget`).
    pub(crate) cast: Option<PendingCast>,
    /// Bumped by every cast begin; a `CastEnd` timer with an older value is stale.
    pub(crate) cast_gen: u32,
}

/// A skill request stored as the step action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepSkill {
    pub(crate) skill: u16,
    pub(crate) level: u16,
}

/// A begun cast waiting for its castend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingCast {
    pub(crate) skill: u16,
    pub(crate) level: u8,
    pub(crate) target: Entity,
}

/// A hit swung at `target`, applied at swing + amotion (spec §2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingHit {
    attacker: Entity,
    target: Entity,
    damage: i32,
    /// The Chebyshev distance at swing time plus `AREA_SIZE`: farther at apply time drops it.
    max_distance: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatTimer {
    Swing {
        attacker: Entity,
        generation: u32,
    },
    StepAction {
        attacker: Entity,
        generation: u32,
    },
    Hit(PendingHit),
    /// The end of a cast begun with this generation (spec `specs/effects/spec.md` §2.3).
    CastEnd {
        caster: Entity,
        generation: u32,
    },
    /// A dead mob's `0x0080` type 1 around its death cell, then its despawn.
    Vanish {
        mob: Entity,
        cell: (u16, u16),
    },
}

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

    fn pop_due(&mut self, now: u64) -> Option<(u64, CombatTimer)> {
        if self.heap.peek().is_some_and(|Reverse(t)| t.due <= now) {
            self.heap.pop().map(|Reverse(t)| (t.due, t.timer))
        } else {
            None
        }
    }
}

/// The ammo a weapon type fires, if it needs any (catalog §5.3): bows take arrows, guns and
/// grenade launchers take bullets.
fn needed_ammo(weapon: WeaponType) -> Option<AmmoType> {
    match weapon {
        WeaponType::Bow => Some(AmmoType::Arrow),
        WeaponType::Revolver
        | WeaponType::Rifle
        | WeaponType::Gatling
        | WeaponType::Shotgun
        | WeaponType::Grenade => Some(AmmoType::Bullet),
        _ => None,
    }
}

fn chebyshev(a: (u16, u16), b: (u16, u16)) -> u16 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
}

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
    fn attack_range(&self, e: Entity) -> u16 {
        self.world
            .players
            .get(e)
            .map_or(1, |info| info.derived.range.max(1))
    }

    /// `0x0088` at the unit's cell to its area, itself included (`clif_fixpos`).
    fn fixpos(&mut self, e: Entity) {
        let (x, y) = self.cell(e);
        let gid = self.gid(e);
        self.emit_aoi(e, false, ServerEvent::UnitStopped { gid, x, y });
    }

    /// rAthena `unit_attack` from setting the target (spec §2.2 steps 5-7).
    fn attack(&mut self, e: Entity, target: Entity, continuous: bool, step_attack: bool, now: u64) {
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

    /// rAthena `unit_attack_timer_sub` (spec §2.4 "Swing check"), at `due`.
    fn swing_check(&mut self, e: Entity, due: u64, from_request: bool) {
        let Some(state) = self.world.attacks.get(e).copied() else {
            return;
        };
        let Some(target) = state.target else {
            return;
        };
        let (me, them) = (self.cell(e), self.cell(target));
        // rAthena's OFFICIAL_WALKPATH line check (`CELL_CHKWALL`, any distance) runs with the
        // liveness checks, before the range test: a wall drops the attack silently even when
        // the target is also out of range (catalog `auto_attack.md` §3 step 2).
        if self.is_dead(e)
            || !self.world.in_grid[e.index()]
            || !self.attackable(target)
            || !line_of_sight(&self.walkability, me, them)
        {
            return self.drop_attack(e);
        }
        let range = self.attack_range(e);
        let reach = range + u16::from(state.step_attack);
        let (dx, dy) = (
            i32::from(them.0) - i32::from(me.0),
            i32::from(them.1) - i32::from(me.1),
        );
        if !check_distance_client(dx, dy, reach) {
            // The client walks closer and asks again; the target stays, nothing re-arms.
            let target_id = self.gid(target);
            return self.emit_self(
                e,
                ServerEvent::AttackOutOfRange {
                    target_id,
                    target_x: them.0,
                    target_y: them.1,
                    x: me.0,
                    y: me.1,
                    range,
                },
            );
        }
        if !battle_check_range(&self.walkability, me, them, reach) {
            // Only the 14-cell limit can fail here (the line was checked above). The attack
            // gives up: no player chase with OFFICIAL_WALKPATH.
            return self.drop_attack(e);
        }
        if from_request {
            self.fixpos(e);
        }
        self.stop_walking(e);
        if !self.swing(e, target, due) {
            return;
        }
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let amotion = u64::try_from(info.derived.amotion).unwrap_or(0);
        let adelay = u64::try_from(info.derived.adelay).unwrap_or(0);
        let floor = u64::from(self.battle.amotion_floor(&class_flags(job_mapid(info.job))));
        let state = self.world.attacks.entry(e);
        state.attackable_at = state.attackable_at.max(due + adelay);
        state.canact_at = state.canact_at.max(due + amotion + floor);
        if state.continuous {
            let (due, generation) = (state.attackable_at, state.swing_gen);
            self.combat_timers.push(
                due,
                CombatTimer::Swing {
                    attacker: e,
                    generation,
                },
            );
        }
    }

    /// rAthena `battle_weapon_attack` for a player hitting a mob (spec §2.5-§2.7): the ammo
    /// check, the damage roll, the ammo spend, `0x08c8` to the target's area and the pending
    /// hit. `false` is `ATK_NONE` (no ammo): the loop ends with no delay set.
    fn swing(&mut self, e: Entity, target: Entity, due: u64) -> bool {
        let Some(info) = self.world.players.get(e) else {
            return false;
        };
        let Some(mob) = self.world.mobs.get(target) else {
            return false;
        };
        let status = &info.derived;
        let weapon = info.gear.right;
        let weapon_type = weapon.map_or(WeaponType::Fist, |w| w.weapon_type);
        let ammo_slot = self
            .world
            .inventories
            .get(e)
            .and_then(|inventory| inventory.worn_at(EQP_AMMO));
        let ammo = ammo_slot
            .and_then(|slot| self.world.inventories.get(e)?.get(slot))
            .and_then(|item| self.items.get(item.item_id));
        let needed = needed_ammo(weapon_type);
        if let Some(needed) = needed {
            if ammo.and_then(|data| data.ammo_type) != Some(needed) {
                let refusal = if needed == AmmoType::Arrow {
                    ServerEvent::ActionFailure {
                        failure_type: ARROW_FAIL_NO_AMMO,
                    }
                } else {
                    ServerEvent::SkillFail {
                        skill_id: 0,
                        cause: FAIL_NEED_MORE_BULLET,
                    }
                };
                self.emit_self(e, refusal);
                return false;
            }
        }
        let long_range = needed.is_some();
        let equip_atk = if long_range {
            ammo.map_or(0, |data| i32::try_from(data.attack).unwrap_or(i32::MAX))
        } else {
            0
        };
        let attacker = attacker_view(info, equip_atk, long_range);
        let amotion = status.amotion;
        let source_id = info.account_id.0;
        let target_id = mob.gid;
        let defender = mob.stats.defender;
        let dmotion = mob.stats.status.dmotion;
        let result = calc_weapon_attack(
            &attacker,
            &defender,
            &AttackContext::NORMAL,
            &self.battle,
            |n| u32::try_from(self.rng.below(u64::from(n))).unwrap_or(0),
        );
        if long_range && self.battle.arrow_decrement > 0 {
            if let Some(slot) = ammo_slot {
                self.spend_ammo(e, slot);
            }
        }
        let hit = result.kind.is_hit();
        self.emit_aoi(
            target,
            false,
            ServerEvent::Damage {
                source_id,
                target_id,
                tick: due as u32,
                src_speed: amotion,
                dmg_speed: if hit { dmotion } else { 0 },
                damage: result.damage,
                div: result.div,
                damage_type: result.kind.wire_type(),
                damage2: result.damage2,
            },
        );
        if hit {
            self.queue_hit(e, target, result.damage, amotion, due);
        }
        true
    }

    /// A hit of `damage` from `e` on mob `target`, landing at `due + amotion` (or at once
    /// without `delay_battle_damage`) through the skip rules of `apply_hit`. Swings and weapon
    /// skills share it.
    pub(crate) fn queue_hit(
        &mut self,
        e: Entity,
        target: Entity,
        damage: i32,
        amotion: i32,
        due: u64,
    ) {
        let hit = PendingHit {
            attacker: e,
            target,
            damage,
            max_distance: chebyshev(self.cell(e), self.cell(target)) + AREA_SIZE,
        };
        let delay = if self.battle.delay_battle_damage {
            u64::try_from(amotion).unwrap_or(0)
        } else {
            1
        };
        if delay <= 1 {
            self.apply_hit(&hit, due);
        } else {
            self.combat_timers.push(due + delay, CombatTimer::Hit(hit));
        }
    }

    /// One ammo off the worn stack (spec §2.6): the last one comes off first (`0x099a`), then
    /// `0x07fa` reason 1 and `SP_WEIGHT`.
    fn spend_ammo(&mut self, e: Entity, slot: usize) {
        self.unequip_before_removal(e, slot, 1);
        let Some(inventory) = self.world.inventories.get_mut(e) else {
            return;
        };
        if inventory.remove(slot, 1, &self.items).is_none() {
            return;
        }
        let weight = inventory.weight();
        self.emit_self(
            e,
            ServerEvent::ItemDeleted {
                index: wire_index(slot),
                amount: 1,
                reason: DELETE_SPENT,
            },
        );
        self.emit_weight(e, SP_WEIGHT, weight);
    }

    /// A pending hit lands at `due` unless a skip rule drops it (catalog §7.1): the target must
    /// still be attackable, the attacker still on this map and within the swing-time snapshot.
    /// Then the HP drop, the damage log (overkill counts), the HP bar and, at 0, death
    /// (spec §2.7).
    fn apply_hit(&mut self, hit: &PendingHit, due: u64) {
        if !self.attackable(hit.target) || !self.world.entities.is_alive(hit.attacker) {
            return;
        }
        let Some(char_id) = self.world.players.get(hit.attacker).map(|p| p.char_id) else {
            return;
        };
        if chebyshev(self.cell(hit.attacker), self.cell(hit.target)) > hit.max_distance {
            return;
        }
        let damage = u32::try_from(hit.damage).unwrap_or(0);
        let Some(mob) = self.world.mobs.get_mut(hit.target) else {
            return;
        };
        mob.hp = mob.hp.saturating_sub(damage);
        mob.damage_log.add(char_id, u64::from(damage));
        let dead = mob.hp == 0;
        if self.battle.monster_hp_bars_info {
            self.send_hp_bar(hit.target);
        }
        if dead {
            self.kill_mob(hit.target, due);
        }
    }

    /// `0x0977` to every damage-log member on this map within `AREA_SIZE` of the mob.
    pub(crate) fn send_hp_bar(&mut self, target: Entity) {
        let Some(mob) = self.world.mobs.get(target) else {
            return;
        };
        let event = ServerEvent::MobHpBar {
            gid: mob.gid,
            hp: mob.hp,
            max_hp: mob.max_hp,
        };
        let at = self.cell(target);
        let mut chars: Vec<CharId> = mob
            .damage_log
            .entries()
            .iter()
            .filter(|entry| {
                self.world
                    .entity_by_char(entry.char_id)
                    .is_some_and(|e| chebyshev(self.cell(e), at) <= AREA_SIZE)
            })
            .map(|entry| entry.char_id)
            .collect();
        chars.sort_unstable();
        self.emit_to(chars, event);
    }

    /// rAthena `mob_dead` without drops or respawn (spec §2.8): out of the grid at once, the exp
    /// for each valid log entry, and the vanish at `due + 250`.
    fn kill_mob(&mut self, target: Entity, due: u64) {
        let cell = self.cell(target);
        let Some(mob) = self.world.mobs.get_mut(target) else {
            return;
        };
        mob.dying = true;
        let log = std::mem::take(&mut mob.damage_log);
        let (base_exp, job_exp) = (mob.stats.base_exp, mob.stats.job_exp);
        self.world.hide(target);

        // Players on this map count as attackers; the dead among them get nothing; offline or
        // elsewhere neither counts nor gets anything, and nobody inherits their share.
        let total = log
            .entries()
            .iter()
            .fold(0u64, |sum, entry| sum.saturating_add(entry.damage));
        let mut attackers = 0;
        let mut paid: SmallVec<[(Entity, u64); 4]> = SmallVec::new();
        for entry in log.entries() {
            let Some(e) = self.world.entity_by_char(entry.char_id) else {
                continue;
            };
            attackers += 1;
            if !self.is_dead(e) {
                paid.push((e, entry.damage));
            }
        }
        for (e, damage) in paid {
            let share = exp_share(damage, total, attackers, &self.battle);
            self.gain_exp(e, shared_exp(base_exp, share), shared_exp(job_exp, share));
        }
        self.combat_timers.push(
            due + DEATH_VANISH_DELAY,
            CombatTimer::Vanish { mob: target, cell },
        );
    }

    /// The death vanish to the area of the death cell (the mob is out of the grid), then the
    /// despawn, which frees the gid.
    fn vanish_dead_mob(&mut self, mob: Entity, cell: (u16, u16)) {
        let Some(gid) = self.world.mobs.get(mob).map(|info| info.gid) else {
            return;
        };
        let chars = self.recipients_at(cell, AREA_SIZE);
        self.emit_to(
            chars,
            ServerEvent::UnitVanished {
                gid,
                vanish_type: VANISH_DIED,
            },
        );
        if let Err(err) = self.world.despawn(mob) {
            warn!(map = %self.map_name, %err, "dead mob despawn failed");
        }
    }

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
            speed * DIAGONAL_COST / STRAIGHT_COST / 2
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
    fn fire_step_action(&mut self, e: Entity, generation: u32, due: u64) {
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

/// What `calc_weapon_attack` reads of a player: its status, the right-hand weapon, and the
/// ammo ATK and range of a ranged swing (`equip_atk` 0 and `long_range` false for melee and for
/// weapon skills).
pub(crate) fn attacker_view(
    info: &crate::world::PlayerInfo,
    equip_atk: i32,
    long_range: bool,
) -> AttackerView {
    let status = &info.derived;
    AttackerView {
        weapon: info.gear.right.map(|w| WeaponAttack {
            atk: w.atk,
            refine_atk: w.refine.bonus / 100,
            level: w.level,
            overrefine: status.overrefine[0],
            dex_based: weapon_class(w.weapon_type).is_ranged(),
        }),
        equip_atk,
        long_range,
        ..AttackerView::from_status(status)
    }
}
