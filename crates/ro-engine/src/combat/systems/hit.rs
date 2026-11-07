//! Delayed hits: queueing, ammo spend, damage and the HP bar.

use ro_core::{CharId, ServerEvent};

use crate::combat::rules::chebyshev;
use crate::combat::{CombatTimer, PendingHit};
use crate::ecs::{Entity, AREA_SIZE};
use crate::item::wire_index;
use crate::map_instance::MapInstance;
use crate::protocol_consts::SP_WEIGHT;

/// `0x07fa` delete type of spent ammo.
const DELETE_SPENT: u16 = 1;

impl MapInstance {
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
    pub(crate) fn spend_ammo(&mut self, e: Entity, slot: usize) {
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
    pub(crate) fn apply_hit(&mut self, hit: &PendingHit, due: u64) {
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
}
