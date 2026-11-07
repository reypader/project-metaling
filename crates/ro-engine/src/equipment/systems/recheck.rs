//! Re-checking worn gear after a level or job change.

use ro_core::ServerEvent;

use crate::ecs::Entity;
use crate::equipment::systems::unequip::TAKE_OFF;
use crate::equipment::{check_equip, Refusal, Wearer};
use crate::job::job_mapid;
use crate::map_instance::MapInstance;

impl MapInstance {
    /// rAthena `pc_baselevelchanged` (spec `specs/stats/spec.md` §2.5): every worn item whose
    /// `EquipLevelMax` is below the base level comes off, with its normal take-off packets.
    pub(crate) fn unequip_over_level(&mut self, e: Entity) {
        let Some(level) = self.world.players.get(e).map(|info| info.clevel) else {
            return;
        };
        for slot in self.worn_slots(e) {
            let over = self
                .world
                .inventories
                .get(e)
                .and_then(|inventory| inventory.get(slot))
                .filter(|item| item.equip != 0)
                .and_then(|item| self.items.get(item.item_id))
                .is_some_and(|d| d.equip_level_max != 0 && level > d.equip_level_max);
            if over {
                self.unequip_slot(e, slot, TAKE_OFF);
            }
        }
    }

    /// `pc_jobchange` step 12 (spec `specs/stats/spec.md` §2.6): every worn item the current
    /// class may not wear comes off, checked one at a time against what is still worn. A refusal
    /// that carries a client message (ammo with no fitting weapon) sends it first, as
    /// `pc_isequip` does.
    pub(crate) fn unequip_unusable(&mut self, e: Entity) {
        let Some(wearer) = self.world.players.get(e).map(|info| Wearer {
            base_level: info.clevel,
            sex: info.sex,
            mapid: job_mapid(info.job),
        }) else {
            return;
        };
        for slot in self.worn_slots(e) {
            let refusal = {
                let Some(inventory) = self.world.inventories.get(e) else {
                    return;
                };
                let entry = inventory.get(slot).filter(|item| item.equip != 0);
                let Some((entry, data)) =
                    entry.and_then(|entry| Some((entry, self.items.get(entry.item_id)?)))
                else {
                    continue;
                };
                let weapon_type = inventory.gear(&self.items).weapon_type();
                check_equip(data, entry, &wearer, weapon_type).err()
            };
            match refusal {
                None => {}
                Some(Refusal::Message { msg_id }) => {
                    self.emit_self(e, ServerEvent::ClientMessage { msg_id });
                    self.unequip_slot(e, slot, TAKE_OFF);
                }
                Some(_) => self.unequip_slot(e, slot, TAKE_OFF),
            }
        }
    }
}
