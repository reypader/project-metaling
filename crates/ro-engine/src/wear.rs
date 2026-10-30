//! The equip flows (spec `specs/equipment/spec.md` §2.2, §2.4-§2.6): wear an item, take one
//! off, unequip all. The rules they apply are in `equipment.rs`; this file sends the packets in
//! the captured order and keeps the worn-slot table, the looks cache and the status in step.

use ro_core::{ServerEvent, UnitLooks};
use ro_db::{ItemData, ItemType};

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::equipment::{
    ammo_fits_weapon, check_equip, equip_view, is_ranged, prepare_equip, Refusal, Wearer,
    EQI_ORDER, EQP_AMMO, EQP_ARMS, EQP_HAND_L, EQP_HAND_R, EQP_SHOES, EQUIP_FAIL,
};
use crate::inventory::{slot_of, wire_index};
use crate::job::job_mapid;

/// `LOOK_*` types of `0x01d7`.
pub(crate) const LOOK_WEAPON: u8 = 2;
const LOOK_HEAD_BOTTOM: u8 = 3;
const LOOK_HEAD_TOP: u8 = 4;
const LOOK_HEAD_MID: u8 = 5;
const LOOK_SHOES: u8 = 9;
const LOOK_ROBE: u8 = 12;
/// rAthena `dead_sit` value of a dead unit.
const DEAD: u8 = 1;
/// `ZC_NOTIFY_VANISH` type for a death.
const VANISH_DIED: u8 = 1;
/// The `0x099a` index for a take-off whose slot is out of range (server index 0).
const OUT_OF_RANGE_INDEX: u16 = 2;
/// The ammo icon (`EFST_CLIENT_ONLY_EQUIP_ARROW`), started on every ammo equip and map entry and
/// never ended, as in rAthena (spec §2.7).
const EFST_EQUIP_ARROW: u16 = 695;
const AMMO_ICON_STATE: u8 = 1;
const AMMO_ICON_MS: u32 = 9999;
/// `0x013b` type: "ammunition equipped".
const ACTION_FAILURE_AMMO_EQUIPPED: u16 = 3;

/// How one item comes off (rAthena `pc_unequipitem` flags). Every unequip marks the player
/// dirty; the tick's recalc pass sends one net diff (spec `specs/stats/spec.md` §2.3).
#[derive(Debug, Clone, Copy)]
struct Unequip {
    /// A ranged weapon takes the worn ammo off with it.
    cascade: bool,
}

/// A take-off, an unequip-all entry, or a worn item dropped (rAthena flag 1).
const TAKE_OFF: Unequip = Unequip { cascade: true };
/// Displaced by an equip (flag `1|2|4`): the ammo waits for the new weapon (spec §2.2 step 14).
const DISPLACED: Unequip = Unequip { cascade: false };
/// Ammo taken off by its weapon: only its `0x099a`.
const CASCADED_AMMO: Unequip = Unequip { cascade: false };

fn ammo_icon(gid: u32) -> ServerEvent {
    ServerEvent::StatusIconStarted {
        efst: EFST_EQUIP_ARROW,
        gid,
        state: AMMO_ICON_STATE,
        total_ms: AMMO_ICON_MS,
        remain_ms: AMMO_ICON_MS as i32,
        values: [0; 3],
    }
}

fn is_ranged_weapon(data: &ItemData) -> bool {
    data.item_type == ItemType::Weapon && is_ranged(data.weapon_type)
}

impl MapInstance {
    pub(crate) fn is_dead(&self, e: Entity) -> bool {
        self.world.dead_sit.get(e.index()) == Some(&DEAD)
    }

    /// The dead gate of equip and take-off: `0x0080` type 1 to the area including self, no ack.
    fn refuse_dead(&mut self, e: Entity) {
        let gid = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::UnitVanished {
                gid,
                vanish_type: VANISH_DIED,
            },
        );
    }

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
    fn unequip_slot(&mut self, e: Entity, slot: usize, how: Unequip) {
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
    fn worn_slots(&self, e: Entity) -> Vec<usize> {
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

    /// rAthena `pc_equiplookall` as `0x01d7` events: the hands, the worn shoes' item id, the
    /// head views and the robe, each with its current value.
    pub(crate) fn equip_look_events(&self, e: Entity) -> Vec<ServerEvent> {
        let gid = self.gid(e);
        let looks = self
            .world
            .players
            .get(e)
            .map(|info| info.looks)
            .unwrap_or_default();
        let shoes = self
            .world
            .inventories
            .get(e)
            .and_then(|inventory| inventory.worn_at(EQP_SHOES))
            .and_then(|slot| self.world.inventories.get(e)?.get(slot))
            .map_or(0, |item| {
                self.items
                    .get(item.item_id)
                    .map_or(item.item_id.0, ItemData::client_id)
            });
        [
            (LOOK_WEAPON, looks.weapon, looks.shield),
            (LOOK_SHOES, shoes, 0),
            (LOOK_HEAD_BOTTOM, looks.head_bottom.into(), 0),
            (LOOK_HEAD_TOP, looks.head_top.into(), 0),
            (LOOK_HEAD_MID, looks.head_mid.into(), 0),
            (LOOK_ROBE, looks.robe.into(), 0),
        ]
        .into_iter()
        .map(|(look_type, value, value2)| ServerEvent::SpriteChanged {
            gid,
            look_type,
            value,
            value2,
        })
        .collect()
    }

    /// Recomputes `e`'s looks from the worn-slot table and stores them. Returns (before, after).
    fn refresh_looks(&mut self, e: Entity) -> (UnitLooks, UnitLooks) {
        let new = self
            .world
            .inventories
            .get(e)
            .map(|inventory| inventory.gear(&self.items).unit_looks())
            .unwrap_or_default();
        let old = self
            .world
            .players
            .get_mut(e)
            .map(|info| std::mem::replace(&mut info.looks, new))
            .unwrap_or_default();
        (old, new)
    }

    /// One `LOOK_WEAPON` per hand in `mask` (a two-hander sends two identical ones), and
    /// `LOOK_SHOES` with `shoes` for the shoe position. Always sent, changed or not.
    fn send_hand_looks(&mut self, e: Entity, mask: u32, looks: &UnitLooks, shoes: u32) {
        for hand in [EQP_HAND_R, EQP_HAND_L] {
            if mask & hand != 0 {
                self.send_look(e, LOOK_WEAPON, looks.weapon, looks.shield);
            }
        }
        if mask & EQP_SHOES != 0 {
            self.send_look(e, LOOK_SHOES, shoes, 0);
        }
    }

    /// Head and robe looks, only for the views that changed (after the costume override).
    fn send_head_looks(&mut self, e: Entity, old: &UnitLooks, new: &UnitLooks) {
        for (look_type, before, after) in [
            (LOOK_HEAD_BOTTOM, old.head_bottom, new.head_bottom),
            (LOOK_HEAD_MID, old.head_mid, new.head_mid),
            (LOOK_HEAD_TOP, old.head_top, new.head_top),
            (LOOK_ROBE, old.robe, new.robe),
        ] {
            if before != after {
                self.send_look(e, look_type, after.into(), 0);
            }
        }
    }

    /// `0x01d7` to the area including self.
    pub(crate) fn send_look(&mut self, e: Entity, look_type: u8, value: u32, value2: u32) {
        let gid = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::SpriteChanged {
                gid,
                look_type,
                value,
                value2,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ro_core::{
        AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, ClientIntent,
        InventoryItem, ItemId, MapPosition, ScopedServerEvent, SessionId, SessionIntent, Sex,
    };
    use ro_db::{MapGrid, MobDb};

    use super::*;
    use crate::inventory::tests::test_items;

    const KNIFE: u32 = 1201;

    fn char_id(n: u32) -> CharId {
        CharId(150_000 + n)
    }

    fn send(engine: &mut MapInstance, n: u32, intent: ClientIntent) -> Vec<ScopedServerEvent> {
        engine.push_intent(SessionIntent::new(
            SessionId(u64::from(n)),
            Some(AccountId(2_000_000 + n)),
            Some(char_id(n)),
            intent,
        ));
        let mut out = Vec::new();
        engine.step_tick(0, &mut out).expect("tick");
        out
    }

    fn enter(engine: &mut MapInstance, n: u32, slots: Vec<Option<InventoryItem>>) {
        let spawn = CharacterSpawn {
            account_id: AccountId(2_000_000 + n),
            char_id: char_id(n),
            name: format!("C{n}"),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            hair_style: 1,
            hair_color: 1,
            pos: MapPosition {
                x: 50,
                y: 50,
                dir: 0,
            },
            speed: 150,
            status: Default::default(),
            group_id: 0,
            inventory: CharacterInventory {
                slots,
                unique_item_counter: 0,
            },
            skills: Vec::new(),
            hidden_skills: Vec::new(),
            change_level: [0; 3],
        };
        engine.spawn_character(spawn, 0).expect("spawn");
        send(engine, n, ClientIntent::NotifyActorInit);
    }

    /// A wearing a Knife (index 2) with a second one in its bag (index 3), B watching, A dead.
    fn dead_wearer() -> MapInstance {
        let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        engine.set_registries(crate::Registries {
            items: Arc::new(test_items()),
            mobs: Arc::new(MobDb::default()),
            ..Default::default()
        });
        let mut worn = InventoryItem::new(ItemId(KNIFE), 1);
        worn.equip = EQP_HAND_R;
        let spare = InventoryItem::new(ItemId(KNIFE), 1);
        enter(&mut engine, 1, vec![Some(worn), Some(spare)]);
        enter(&mut engine, 2, vec![]);
        let e = engine.world.entity_by_char(char_id(1)).expect("A");
        engine.world.dead_sit[e.index()] = DEAD;
        engine
    }

    fn equips(engine: &MapInstance) -> Vec<u32> {
        let e = engine.world.entity_by_char(char_id(1)).expect("A");
        let inventory = engine.world.inventories.get(e).expect("inventory");
        (0..2)
            .map(|slot| inventory.get(slot).map_or(0, |item| item.equip))
            .collect()
    }

    fn died_to_the_area() -> Vec<ScopedServerEvent> {
        vec![ScopedServerEvent::new(
            BroadcastScope::Characters(vec![char_id(1), char_id(2)]),
            ServerEvent::UnitVanished {
                gid: 2_000_001,
                vanish_type: VANISH_DIED,
            },
        )]
    }

    /// ENG-EQ-06: a dead player's equip and take-off get `0x0080` type 1 to the area and no ack.
    #[test]
    fn eng_eq_06_dead_equip_and_take_off() {
        let mut engine = dead_wearer();
        let equip = ClientIntent::EquipItem {
            index: 3,
            position: EQP_HAND_R,
        };
        assert_eq!(send(&mut engine, 1, equip), died_to_the_area());
        assert_eq!(
            send(&mut engine, 1, ClientIntent::UnequipItem { index: 2 }),
            died_to_the_area()
        );
        assert_eq!(equips(&engine), vec![EQP_HAND_R, 0], "nothing changed");
    }

    /// ENG-EQ-08: a dead player's unequip-all gets `0x0bae` result 1 and removes nothing (our
    /// ruling; rAthena runs it, spec §2.5).
    #[test]
    fn eng_eq_08_dead_unequip_all_is_refused() {
        let mut engine = dead_wearer();
        let out = send(
            &mut engine,
            1,
            ClientIntent::UnequipAll { location: 0x83ff },
        );
        assert_eq!(
            out,
            vec![ScopedServerEvent::new(
                BroadcastScope::SelfOnly(char_id(1)),
                ServerEvent::UnequipAllAck { failed: true },
            )]
        );
        assert_eq!(equips(&engine), vec![EQP_HAND_R, 0]);
    }
}
