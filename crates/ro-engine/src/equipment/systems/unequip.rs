//! Taking gear off: one item, everything, or a slot, with the ammo cascade.

use ro_core::ServerEvent;
use ro_db::{ItemData, ItemType};

use crate::ecs::Entity;
use crate::equipment::{is_ranged, EQI_ORDER, EQP_AMMO, EQP_ARMS};
use crate::item::{slot_of, wire_index};
use crate::map_instance::MapInstance;

/// The `0x099a` index for a take-off whose slot is out of range (server index 0).
const OUT_OF_RANGE_INDEX: u16 = 2;

/// How one item comes off (rAthena `pc_unequipitem` flags). Every unequip marks the player
/// dirty; the tick's recalc pass sends one net diff (spec `specs/stats/spec.md` §2.3).
#[derive(Debug, Clone, Copy)]
pub(super) struct Unequip {
    /// A ranged weapon takes the worn ammo off with it.
    cascade: bool,
}

/// A take-off, an unequip-all entry, or a worn item dropped (rAthena flag 1).
pub(super) const TAKE_OFF: Unequip = Unequip { cascade: true };

/// Displaced by an equip (flag `1|2|4`): the ammo waits for the new weapon (spec §2.2 step 14).
pub(super) const DISPLACED: Unequip = Unequip { cascade: false };

/// Ammo taken off by its weapon: only its `0x099a`.
pub(super) const CASCADED_AMMO: Unequip = Unequip { cascade: false };

fn is_ranged_weapon(data: &ItemData) -> bool {
    data.item_type == ItemType::Weapon && is_ranged(data.weapon_type)
}

impl MapInstance {
    /// Spec §2.8: a worn item whose whole stack is about to be removed comes off first (forced,
    /// with its push and the ammo cascade).
    pub(crate) fn unequip_before_removal(&mut self, e: Entity, slot: usize, amount: u16) {
        let whole_worn_stack = self
            .world
            .inventories
            .get(e)
            .and_then(|inventory| inventory.get(slot))
            .is_some_and(|item| item.equip != 0 && item.amount == amount);
        if whole_worn_stack {
            self.unequip_slot(e, slot, TAKE_OFF);
        }
    }

    /// `CZ_REQ_TAKEOFF_EQUIP` (spec §2.4).
    pub(crate) fn unequip_item(&mut self, e: Entity, index: u16) {
        // The client sends no take-off while dead (capture 4); this is safety only.
        if self.is_dead(e) {
            return self.refuse_dead(e);
        }
        let failed = |index| ServerEvent::UnequipAck {
            index,
            location: 0,
            failed: true,
        };
        let Some(slot) = slot_of(index) else {
            return self.emit_self(e, failed(OUT_OF_RANGE_INDEX));
        };
        let worn = self
            .world
            .inventories
            .get(e)
            .and_then(|inventory| inventory.get(slot))
            .is_some_and(|item| item.equip != 0);
        if !worn {
            return self.emit_self(e, failed(index));
        }
        self.unequip_slot(e, slot, TAKE_OFF);
    }

    /// `CZ_REQ_TAKEOFF_EQUIP_ALL` (spec §2.5): every worn position the mask names, in `EQI`
    /// order, each item with its own packets and push, then `0x0bae`.
    pub(crate) fn unequip_all(&mut self, e: Entity, location: u32) {
        // rAthena's handler has no dead check, so a dead player's unequip-all runs in full
        // (capture 4 line 353). We treat that as an rAthena bug (user ruling, 2026-10-25) and
        // refuse with the reply rAthena gives its own `pc_cant_act` gate. Drop this check to
        // match rAthena.
        if self.is_dead(e) {
            return self.emit_self(e, ServerEvent::UnequipAllAck { failed: true });
        }
        for position in EQI_ORDER.into_iter().filter(|p| p & location != 0) {
            // A two-hander's second hand, and ammo its bow took along, are already free when the
            // loop reaches them.
            let held = self
                .world
                .inventories
                .get(e)
                .and_then(|inventory| inventory.worn_at(position));
            if let Some(slot) = held {
                self.unequip_slot(e, slot, TAKE_OFF);
            }
        }
        self.emit_self(e, ServerEvent::UnequipAllAck { failed: false });
    }

    /// rAthena `pc_unequipitem` (spec §2.6): hand and shoe looks, `0x099a`, head looks, the
    /// ammo cascade, then the push.
    pub(super) fn unequip_slot(&mut self, e: Entity, slot: usize, how: Unequip) {
        let Some(inventory) = self.world.inventories.get_mut(e) else {
            return;
        };
        let item_id = inventory.get(slot).map(|item| item.item_id);
        let Some(mask) = inventory.take_off(slot) else {
            return;
        };
        let ranged = item_id
            .and_then(|id| self.items.get(id))
            .is_some_and(is_ranged_weapon);
        let (old, new) = self.refresh_looks(e);
        self.send_hand_looks(e, mask, &new, 0);
        self.emit_self(
            e,
            ServerEvent::UnequipAck {
                index: wire_index(slot),
                location: mask,
                failed: false,
            },
        );
        self.send_head_looks(e, &old, &new);
        if how.cascade && ranged && mask & EQP_ARMS != 0 {
            let ammo = self
                .world
                .inventories
                .get(e)
                .and_then(|inventory| inventory.worn_at(EQP_AMMO));
            if let Some(ammo) = ammo {
                self.unequip_slot(e, ammo, CASCADED_AMMO);
            }
        }
        self.mark_dirty(e, true);
    }

    /// The slots `e` wears, in `EQI` order, each once (a two-hander holds two positions).
    pub(crate) fn worn_slots(&self, e: Entity) -> Vec<usize> {
        let mut slots = Vec::new();
        let Some(inventory) = self.world.inventories.get(e) else {
            return slots;
        };
        for position in EQI_ORDER {
            if let Some(slot) = inventory.worn_at(position) {
                if !slots.contains(&slot) {
                    slots.push(slot);
                }
            }
        }
        slots
    }
}
