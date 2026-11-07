//! The swing loop: the pre-swing checks and the swing itself.

use ro_core::ServerEvent;
use ro_db::{AmmoType, WeaponType};
use ro_mechanics::{calc_weapon_attack, check_distance_client, AttackContext};

use crate::combat::rules::needed_ammo;
use crate::combat::{attacker_view, CombatTimer};
use crate::ecs::Entity;
use crate::equipment::EQP_AMMO;
use crate::job::{class_flags, job_mapid};
use crate::map_instance::MapInstance;
use crate::movement::{battle_check_range, line_of_sight};

/// `0x0110` for a gun with no bullets: skill 0, `USESKILL_FAIL_NEED_MORE_BULLET`.
const FAIL_NEED_MORE_BULLET: u8 = 84;

/// `0x013b` `ARROWFAIL_NO_AMMO`.
const ARROW_FAIL_NO_AMMO: u16 = 0;

impl MapInstance {
    /// rAthena `unit_attack_timer_sub` (spec §2.4 "Swing check"), at `due`.
    pub(crate) fn swing_check(&mut self, e: Entity, due: u64, from_request: bool) {
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
}
