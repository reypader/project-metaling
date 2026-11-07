//! Wearing an item: the checks, the ammo slot and the refusals.

use ro_core::ServerEvent;
use ro_db::{ItemData, ItemType};

use crate::ecs::Entity;
use crate::equipment::systems::looks::ammo_icon;
use crate::equipment::systems::unequip::{CASCADED_AMMO, DISPLACED};
use crate::equipment::{
    ammo_fits_weapon, equip_view, prepare_equip, Refusal, Wearer, EQI_ORDER, EQP_AMMO, EQP_ARMS,
    EQUIP_FAIL,
};
use crate::item::{slot_of, wire_index};
use crate::job::job_mapid;
use crate::map_instance::MapInstance;

/// `0x013b` type: "ammunition equipped".
const ACTION_FAILURE_AMMO_EQUIPPED: u16 = 3;

impl MapInstance {
    /// `CZ_REQ_WEAR_EQUIP` (spec §2.2).
    pub(crate) fn equip_item(&mut self, e: Entity, index: u16, requested: u32) {
        // The client sends no equip request while dead (capture 4); this is safety only.
        if self.is_dead(e) {
            return self.refuse_dead(e);
        }
        let Some(slot) = slot_of(index) else {
            return;
        };
        let prepared = {
            let (Some(info), Some(inventory)) =
                (self.world.players.get(e), self.world.inventories.get(e))
            else {
                return;
            };
            let Some(entry) = inventory.get(slot) else {
                return;
            };
            let wearer = Wearer {
                base_level: info.clevel,
                sex: info.sex,
                mapid: job_mapid(info.job),
            };
            prepare_equip(entry, requested, &wearer, &inventory.gear(&self.items))
                .map(|mask| (mask, entry.item_id))
        };
        let (mask, item_id) = match prepared {
            Ok(prepared) => prepared,
            Err(Refusal::NoAck) => return,
            Err(Refusal::Result(result)) => return self.refuse_equip(e, index, result),
            Err(Refusal::Message { msg_id }) => {
                self.emit_self(e, ServerEvent::ClientMessage { msg_id });
                return self.refuse_equip(e, index, EQUIP_FAIL);
            }
        };

        // Step 10: take the positions one at a time, so each displaced item's looks and push
        // already count the new item in the positions it holds so far.
        for position in EQI_ORDER.into_iter().filter(|p| p & mask != 0) {
            let held = self
                .world
                .inventories
                .get(e)
                .and_then(|inventory| inventory.worn_at(position));
            if let Some(other) = held.filter(|&other| other != slot) {
                self.unequip_slot(e, other, DISPLACED);
            }
            if let Some(inventory) = self.world.inventories.get_mut(e) {
                inventory.wear_position(position, slot);
            }
        }
        let data = self.items.get(item_id);
        let view = data.map_or(0, |d| equip_view(d, mask));
        let shoes = data.map_or(item_id.0, ItemData::client_id);
        let is_ammo = data.is_some_and(|d| d.item_type == ItemType::Ammo);
        if let Some(inventory) = self.world.inventories.get_mut(e) {
            inventory.set_equip(slot, mask);
        }

        let (old, new) = self.refresh_looks(e);
        if is_ammo {
            self.send_ammo_equipped(e, index, true);
            self.emit_self(
                e,
                ServerEvent::ActionFailure {
                    failure_type: ACTION_FAILURE_AMMO_EQUIPPED,
                },
            );
        } else {
            self.emit_self(
                e,
                ServerEvent::EquipAck {
                    index,
                    location: mask,
                    view,
                    result: 0,
                },
            );
        }
        self.send_hand_looks(e, mask, &new, shoes);
        self.send_head_looks(e, &old, &new);
        if mask & EQP_ARMS != 0 {
            self.drop_unfit_ammo(e);
        }
        self.mark_dirty(e, true);
    }

    /// `0x0983` (the ammo icon) and `0x013c`. The icon goes to the area on a live equip; at map
    /// entry both go to self only (spec §2.7).
    fn send_ammo_equipped(&mut self, e: Entity, index: u16, to_area: bool) {
        let icon = ammo_icon(self.gid(e));
        if to_area {
            self.emit_aoi(e, false, icon);
        } else {
            self.emit_self(e, icon);
        }
        self.emit_self(e, ServerEvent::AmmoEquipped { index });
    }

    /// Map entry: the worn ammo's icon and `0x013c`, between the two inventory lists.
    pub(crate) fn send_worn_ammo(&mut self, e: Entity) {
        let ammo = self
            .world
            .inventories
            .get(e)
            .and_then(|inventory| inventory.worn_at(EQP_AMMO));
        if let Some(slot) = ammo {
            self.send_ammo_equipped(e, wire_index(slot), false);
        }
    }

    /// Spec §2.2 step 14: a weapon went into a hand; worn ammo it can't fire comes off.
    fn drop_unfit_ammo(&mut self, e: Entity) {
        let Some(inventory) = self.world.inventories.get(e) else {
            return;
        };
        let Some(slot) = inventory.worn_at(EQP_AMMO) else {
            return;
        };
        let weapon_type = inventory.gear(&self.items).weapon_type();
        let fits = inventory
            .get(slot)
            .and_then(|ammo| self.items.get(ammo.item_id))
            .and_then(|d| d.ammo_type)
            .is_none_or(|ammo| ammo_fits_weapon(ammo, weapon_type));
        if !fits {
            self.unequip_slot(e, slot, CASCADED_AMMO);
        }
    }

    fn refuse_equip(&mut self, e: Entity, index: u16, result: u8) {
        self.emit_self(
            e,
            ServerEvent::EquipAck {
                index,
                location: 0,
                view: 0,
                result,
            },
        );
    }
}
