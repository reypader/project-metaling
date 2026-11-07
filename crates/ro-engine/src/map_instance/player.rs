//! Player lifecycle: spawn and despawn, inventory hand-off, sit/stand and the dead checks.

use std::mem;

use ro_core::{CharId, CharacterSpawn, InventoryItem, ScopedServerEvent, ServerEvent};

use super::MapInstance;
use crate::ecs::Entity;
use crate::error::EngineError;
use crate::item::PlayerInventory;
use crate::protocol_consts::{
    ACTION_SIT, ACTION_STAND, DEAD, SITTING, SP_MAXWEIGHT, SP_WEIGHT, STANDING, VANISH_DIED,
    VANISH_LOGGED_OUT,
};
use crate::session::Arrival;

impl MapInstance {
    /// `CZ_NOTIFY_ACTORINIT`: the inventory lists and weights (on every one, repeats included);
    /// the first time also the `0x01d7` look before them, then into the grid, spawn to others,
    /// `0x099b`, others to self, the entry settings.
    pub(crate) fn actor_init(&mut self, e: Entity, now_ms: u64) {
        let entering = !self.world.in_grid[e.index()];
        if entering {
            self.send_weapon_look(e);
        }
        self.send_inventory(e);
        if !entering {
            return;
        }
        self.world.show(e);
        if let Some(unit) = self.appearance(e) {
            self.emit_aoi(e, true, ServerEvent::UnitSpawned { unit });
        }
        self.send_map_property(e);
        self.query_into_after(self.cell(e));
        let tick = now_ms as u32;
        for k in 0..self.aoi_after.len() {
            let other = self.aoi_after[k];
            if other == e {
                continue;
            }
            if let Some(event) = self.entry_event(other, tick) {
                self.emit_self(e, event);
            }
        }
        self.send_entry_settings(e);
        // Arrival cell: warps chain from here (ENG-WARP-05).
        self.touch_warps(e);
    }

    /// The entry lists in captured order: start, stackables (if any), non-stackables (if any),
    /// end, the empty equip-switch list, SP_WEIGHT, SP_MAXWEIGHT.
    fn send_inventory(&mut self, e: Entity) {
        let Some(inventory) = self.world.inventories.get(e) else {
            return;
        };
        let (normal, equip) = inventory.list_entries(&self.items);
        let (weight, max_weight) = (inventory.weight(), inventory.max_weight());
        self.emit_self(e, ServerEvent::InventoryStart);
        if !normal.is_empty() {
            self.emit_self(e, ServerEvent::InventoryNormalItems { items: normal });
        }
        self.send_worn_ammo(e);
        if !equip.is_empty() {
            self.emit_self(e, ServerEvent::InventoryEquipItems { items: equip });
        }
        self.emit_self(e, ServerEvent::InventoryEnd);
        self.emit_self(e, ServerEvent::EquipSwitchList);
        self.emit_weight(e, SP_WEIGHT, weight);
        self.emit_weight(e, SP_MAXWEIGHT, max_weight);
    }

    /// Creates the player entity, moves the spawn's inventory into its component and sets
    /// which entry settings its first actor init sends (`arrival`).
    pub(crate) fn spawn_player(
        &mut self,
        spawn: &mut CharacterSpawn,
        arrival: Arrival,
    ) -> Result<Entity, EngineError> {
        let e = self.world.spawn_player(spawn)?;
        if let Some(info) = self.world.players.get_mut(e) {
            info.connect_new = arrival == Arrival::Login;
            info.changemap = true;
        }
        let inventory = mem::take(&mut spawn.inventory);
        // The status sets the max weight right below.
        let inventory = PlayerInventory::new(inventory, spawn.char_id, 0, &self.items);
        let looks = inventory.gear(&self.items).unit_looks();
        self.world.inventories.insert(e, inventory);
        if let Some(info) = self.world.players.get_mut(e) {
            info.looks = looks;
        }
        self.init_status(e);
        // The visible tree, silently: the login list or a later diff sends it.
        self.refresh_skills(e);
        Ok(e)
    }

    /// Adds an item (`item.amount` of it) to `e`'s inventory: `0x0b41` and SP_WEIGHT on
    /// success, the `0x0b41` failure form on failure. Returns the `0x0b41` result code.
    pub(crate) fn give_item_to(&mut self, e: Entity, item: &InventoryItem) -> u8 {
        match self.add_item(e, item) {
            Ok(()) => 0,
            Err(result) => {
                self.emit_self(e, ServerEvent::ItemAddFailed { result });
                result
            }
        }
    }

    /// rAthena `pc_additem`: on success sends `0x0b41` and SP_WEIGHT; on failure sends nothing
    /// and returns the result code (the caller decides on the failure ack).
    pub(crate) fn add_item(&mut self, e: Entity, item: &InventoryItem) -> Result<(), u8> {
        let Some(inventory) = self.world.inventories.get_mut(e) else {
            return Err(crate::item::ADD_INVALID);
        };
        let added = inventory.add(item, u32::from(item.amount), &self.items)?;
        let entry = inventory.pickup_entry(added.slot, added.amount, &self.items);
        let weight = inventory.weight();
        if let Some(item) = entry {
            self.emit_self(e, ServerEvent::ItemAdded { item });
        }
        self.emit_weight(e, SP_WEIGHT, weight);
        Ok(())
    }

    /// Vanishes player `e` to its AoI (excluding itself), drops its queued intents and frees
    /// the entity. Shared by logout (type 2) and cross-map warps (type 0).
    pub(crate) fn despawn_player(
        &mut self,
        e: Entity,
        char_id: CharId,
        vanish_type: u8,
    ) -> Result<(), EngineError> {
        if self.world.in_grid[e.index()] {
            let gid = self.gid(e);
            self.emit_aoi(e, true, ServerEvent::UnitVanished { gid, vanish_type });
        }
        // Intents queued by the old session must not reach a later spawn of this character.
        self.intents.retain(|i| i.char_id != Some(char_id));
        self.world.despawn(e)
    }

    pub(crate) fn sit(&mut self, e: Entity) {
        self.world.dead_sit[e.index()] = SITTING;
        self.emit_action(e, ACTION_SIT);
    }

    pub(crate) fn stand(&mut self, e: Entity) {
        self.world.dead_sit[e.index()] = STANDING;
        self.emit_action(e, ACTION_STAND);
    }

    /// Creates the entity for a login (not yet visible; it is announced on `NotifyActorInit`) and
    /// returns what the player gets before its first `0x007d`, to send after `ZC_ACCEPT_ENTER` and
    /// the friends list: the MOTD, `0x0091` to its own cell and the login status packets.
    /// A duplicate char_id returns `Err(CharacterAlreadyEntered)`.
    pub fn spawn_character(
        &mut self,
        mut spawn: CharacterSpawn,
        _now_ms: u64,
    ) -> Result<Vec<ScopedServerEvent>, EngineError> {
        let (x, y) = (spawn.pos.x, spawn.pos.y);
        if x >= self.walkability.width() || y >= self.walkability.height() {
            return Err(EngineError::InvalidCell { x, y });
        }
        let e = self.spawn_player(&mut spawn, Arrival::Login)?;
        let queued = self.events.len();
        self.send_login_burst(e);
        Ok(self.events.split_off(queued))
    }

    /// Adds `item.amount` of `item` to the character's inventory and sends the `0x0b41` result
    /// (and SP_WEIGHT on success). Returns the `0x0b41` result code: 0 on success.
    pub fn give_item(&mut self, char_id: CharId, item: &InventoryItem) -> Result<u8, EngineError> {
        let e = self
            .world
            .entity_by_char(char_id)
            .ok_or(EngineError::UnknownCharacter(char_id))?;
        Ok(self.give_item_to(e, item))
    }

    /// Removes the entity and emits `UnitVanished{type 2}` to the AoI excluding the leaver.
    pub fn remove_character(
        &mut self,
        char_id: CharId,
    ) -> Result<Vec<ScopedServerEvent>, EngineError> {
        let e = self
            .world
            .entity_by_char(char_id)
            .ok_or(EngineError::UnknownCharacter(char_id))?;
        let start = self.events.len();
        let result = self.despawn_player(e, char_id, VANISH_LOGGED_OUT);
        let events = self.events.split_off(start);
        result.map(|()| events)
    }

    pub(crate) fn is_dead(&self, e: Entity) -> bool {
        self.world.dead_sit.get(e.index()) == Some(&DEAD)
    }

    /// The dead gate of equip and take-off: `0x0080` type 1 to the area including self, no ack.
    pub(crate) fn refuse_dead(&mut self, e: Entity) {
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
}
