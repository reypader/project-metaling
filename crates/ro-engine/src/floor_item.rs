//! Items on the floor: creation, expiry, pickup (`0x0362`) and player drop (`0x0363`)
//! (spec `specs/items/spec.md` §2.3-§2.5; catalog `packets/ground_items.md`).

use std::cmp::Reverse;

use ro_core::{CharId, FloorItemAppearance, InventoryItem, MapPosition, ServerEvent};
use ro_db::ItemData;
use tracing::{debug, warn};

use crate::engine::{MapInstance, SP_WEIGHT};
use crate::entity::Entity;
use crate::inventory::{slot_of, wire_index};
use crate::world::{EntityKind, FloorItem};

/// Floor item ids cycle through `[MIN_FLOOR_ID, MAX_FLOOR_ID)` (rAthena `MIN_FLOORITEM`,
/// `MAX_FLOORITEM`): below account ids and NPC/mob ids.
pub(crate) const MIN_FLOOR_ID: u32 = 2;
pub(crate) const MAX_FLOOR_ID: u32 = 2_000_000;
/// rAthena `flooritem_lifetime`.
pub(crate) const FLOOR_ITEM_LIFETIME_MS: u64 = 60_000;
/// Pickup reach: Chebyshev distance.
const PICKUP_RANGE: u16 = 2;
/// `0x0b41` result for every refused pickup before the add.
const PICKUP_REFUSED: u8 = 6;
/// rAthena `dead_sit` value of a dead unit.
const DEAD: u8 = 1;
/// `ZC_NOTIFY_VANISH` type for a death.
const VANISH_DIED: u8 = 1;
/// `ZC_NOTIFY_ACT` type: pick up.
const ACT_PICKUP_ITEM: u8 = 1;
/// `map_cell_free` rejects cells closer than this to the edge.
const EDGE_GAP: i32 = 5;
/// Wire type of an item missing from the registry (Etc).
const UNKNOWN_ITEM_TYPE: u8 = 3;

/// How a new floor item is owned and shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FloorItemSpec {
    pub(crate) owners: [Option<CharId>; 3],
    pub(crate) owner_ticks: [u64; 3],
    pub(crate) mob_id: u32,
    pub(crate) show_drop_effect: bool,
    pub(crate) drop_effect_mode: u16,
}

impl MapInstance {
    /// Whether `e` is a floor item.
    pub(crate) fn is_floor_item(&self, e: Entity) -> bool {
        self.world.kinds.get(e.index()) == Some(&EntityKind::FloorItem)
    }

    /// How floor item `e` looks to a client.
    pub(crate) fn floor_appearance(&self, e: Entity) -> Option<FloorItemAppearance> {
        let floor = self.world.floor_items.get(e)?;
        let pos = self.pos(e);
        let data = self.items.get(floor.item.item_id);
        Some(FloorItemAppearance {
            id: floor.id,
            item_id: data.map_or(floor.item.item_id.0, ItemData::client_id),
            item_type: data.map_or(UNKNOWN_ITEM_TYPE, ItemData::wire_type),
            identified: floor.item.identified,
            x: pos.x,
            y: pos.y,
            sub_x: floor.sub_x,
            sub_y: floor.sub_y,
            amount: floor.item.amount,
            show_drop_effect: false,
            drop_effect_mode: 0,
        })
    }

    /// The next free floor item id, cycling and skipping ids in use. `None` when every id is
    /// taken.
    fn next_floor_id(&mut self) -> Option<u32> {
        for _ in MIN_FLOOR_ID..MAX_FLOOR_ID {
            let id = self.next_floor_id;
            self.next_floor_id = if id + 1 >= MAX_FLOOR_ID {
                MIN_FLOOR_ID
            } else {
                id + 1
            };
            if !self.world.gid_taken(id) {
                return Some(id);
            }
        }
        None
    }

    /// rAthena `map_addflooritem`: puts `item` on `cell` for 60 s and sends `0x0add` to everyone
    /// in view of the cell. `None` (nothing changed) when no id or entity slot is free.
    pub(crate) fn add_floor_item(
        &mut self,
        item: InventoryItem,
        cell: (u16, u16),
        spec: FloorItemSpec,
        now_ms: u64,
    ) -> Option<Entity> {
        let id = self.next_floor_id()?;
        // Sub-cell offsets: rnd[1,4] × 3 each.
        let sub_x = (self.rng.range_inclusive(1, 4) * 3) as u8;
        let sub_y = (self.rng.range_inclusive(1, 4) * 3) as u8;
        let expire_at = now_ms + FLOOR_ITEM_LIFETIME_MS;
        let floor = FloorItem {
            id,
            item,
            sub_x,
            sub_y,
            owners: spec.owners,
            owner_ticks: spec.owner_ticks,
            expire_at,
            mob_id: spec.mob_id,
        };
        let pos = MapPosition {
            x: cell.0,
            y: cell.1,
            dir: 0,
        };
        let e = match self.world.spawn_floor_item(floor, pos) {
            Ok(e) => e,
            Err(err) => {
                debug!(map = %self.map_name, "floor item not created: {err}");
                return None;
            }
        };
        self.world.show(e);
        self.floor_expiry.push(Reverse((expire_at, id)));
        if let Some(mut item) = self.floor_appearance(e) {
            item.show_drop_effect = spec.show_drop_effect;
            item.drop_effect_mode = spec.drop_effect_mode;
            self.emit_aoi(e, false, ServerEvent::FloorItemDropped { item });
        }
        Some(e)
    }

    /// Removes floor item `e`, sending `0x00a1` to everyone in view of it.
    fn remove_floor_item(&mut self, e: Entity) {
        if let Some(id) = self.world.floor_items.get(e).map(|f| f.id) {
            self.emit_aoi(e, false, ServerEvent::FloorItemVanished { id });
        }
        if let Err(err) = self.world.despawn(e) {
            debug!(map = %self.map_name, "floor item despawn: {err}");
        }
    }

    /// Removes every floor item whose lifetime ended by `now_ms` (heap order; stale entries for
    /// picked-up items are skipped).
    pub(crate) fn expire_floor_items(&mut self, now_ms: u64) {
        while let Some(&Reverse((expire_at, id))) = self.floor_expiry.peek() {
            if expire_at > now_ms {
                break;
            }
            self.floor_expiry.pop();
            let Some(e) = self.world.entity_by_gid(id) else {
                continue;
            };
            let current = self
                .world
                .floor_items
                .get(e)
                .is_some_and(|f| f.expire_at == expire_at);
            if current {
                self.remove_floor_item(e);
            }
        }
    }

    /// `CZ_ITEM_PICKUP` (spec §2.4). Every refusal before the add sends `0x0b41` result 6.
    pub(crate) fn pick_up(&mut self, e: Entity, floor_id: u32, now_ms: u64) {
        let i = e.index();
        if self.world.dead_sit[i] == DEAD {
            let gid = self.gid(e);
            self.emit_aoi(
                e,
                false,
                ServerEvent::UnitVanished {
                    gid,
                    vanish_type: VANISH_DIED,
                },
            );
            return self.refuse_pickup(e);
        }
        let Some(target) = self
            .world
            .entity_by_gid(floor_id)
            .filter(|&t| self.is_floor_item(t))
        else {
            return self.refuse_pickup(e);
        };
        let (me, it) = (self.cell(e), self.cell(target));
        if me.0.abs_diff(it.0) > PICKUP_RANGE || me.1.abs_diff(it.1) > PICKUP_RANGE {
            return self.refuse_pickup(e);
        }
        let char_id = self.world.players.get(e).map(|p| p.char_id);
        let Some(floor) = self.world.floor_items.get(target) else {
            return self.refuse_pickup(e);
        };
        if now_ms < loot_tick(floor, char_id) {
            return self.refuse_pickup(e);
        }
        let item = floor.item.clone();
        if self.give_item_to(e, &item) != 0 {
            // The failure ack went out; the item stays.
            return;
        }
        let source_id = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::EntityAction {
                source_id,
                target_id: floor_id,
                action_type: ACT_PICKUP_ITEM,
                action_tick: 0,
            },
        );
        self.remove_floor_item(target);
    }

    fn refuse_pickup(&mut self, e: Entity) {
        self.emit_self(
            e,
            ServerEvent::ItemAddFailed {
                result: PICKUP_REFUSED,
            },
        );
    }

    /// `CZ_ITEM_THROW` (spec §2.5): gates, cell search, floor item, then `0x0add`, SP_WEIGHT
    /// and `0x00af`. Any failure sends only `0x00af` with count 0.
    pub(crate) fn drop_item(&mut self, e: Entity, index: u16, amount: u16, now_ms: u64) {
        let refuse = |this: &mut Self| {
            this.emit_self(e, ServerEvent::ItemThrown { index, amount: 0 });
        };
        if self.world.dead_sit[e.index()] == DEAD {
            return refuse(self);
        }
        let Some(slot) = slot_of(index) else {
            return refuse(self);
        };
        let Some(inventory) = self.world.inventories.get(e) else {
            return refuse(self);
        };
        let Some(stack) = inventory.get(slot) else {
            return refuse(self);
        };
        if amount == 0 || amount > stack.amount || stack.equip_switch != 0 {
            return refuse(self);
        }
        // There are no GM levels yet, so `NoDropOverride` never applies.
        if self.items.get(stack.item_id).is_some_and(|d| d.no_drop) {
            return refuse(self);
        }
        let mut dropped = stack.clone();
        dropped.amount = amount;

        let own = self.cell(e);
        let cell = self
            .search_free_cell(own, 2, |this, c| this.drop_cell_free(c))
            .unwrap_or(own);
        if self
            .add_floor_item(dropped, cell, FloorItemSpec::default(), now_ms)
            .is_none()
        {
            return refuse(self);
        }
        // The gates above make these unreachable; if a gate change ever lets one through, the
        // item would exist twice, so say so loudly.
        let Some(inventory) = self.world.inventories.get_mut(e) else {
            warn!(map = %self.map_name, "drop: inventory vanished after the floor item was made");
            return;
        };
        if inventory.remove(slot, amount, &self.items).is_none() {
            warn!(map = %self.map_name, slot, amount, "drop: removal failed after the gates passed");
            return;
        }
        let weight = inventory.weight();
        self.emit_weight(e, SP_WEIGHT, weight);
        self.emit_self(
            e,
            ServerEvent::ItemThrown {
                index: wire_index(slot),
                amount,
            },
        );
    }

    /// rAthena `map_cell_free` with item stacking on: walkable and at least 5 cells from every
    /// edge. Units and other floor items never block.
    pub(crate) fn drop_cell_free(&self, (x, y): (i32, i32)) -> bool {
        let w = i32::from(self.walkability.width());
        let h = i32::from(self.walkability.height());
        if x < EDGE_GAP || x > w - EDGE_GAP || y < EDGE_GAP || y > h - EDGE_GAP {
            return false;
        }
        match (u16::try_from(x), u16::try_from(y)) {
            (Ok(x), Ok(y)) => self.walkability.is_walkable(x, y),
            _ => false,
        }
    }

    /// rAthena `map_search_freecell_dist`: random mirrors, then the 8 neighbours in fixed order
    /// (outer `i`, inner `j` in -1..=1, skipping 0,0), each at a fresh random distance
    /// `1..=dist` per axis. The first free candidate wins.
    pub(crate) fn search_free_cell(
        &mut self,
        center: (u16, u16),
        dist: i32,
        free: impl Fn(&Self, (i32, i32)) -> bool,
    ) -> Option<(u16, u16)> {
        let mirror_x = if self.rng.below(2) == 0 { -1 } else { 1 };
        let mirror_y = if self.rng.below(2) == 0 { -1 } else { 1 };
        let (cx, cy) = (i32::from(center.0), i32::from(center.1));
        for i in -1..=1 {
            for j in -1..=1 {
                if i == 0 && j == 0 {
                    continue;
                }
                let dx = self.rng.range_inclusive(1, dist);
                let dy = self.rng.range_inclusive(1, dist);
                let cell = (cx + mirror_x * i * dx, cy + mirror_y * j * dy);
                if free(self, cell) {
                    if let (Ok(x), Ok(y)) = (u16::try_from(cell.0), u16::try_from(cell.1)) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }
}

/// When `picker` may take `floor` (spec §2.4 step 4): owner 1 (or no owner) at once, owner 2
/// after tick 1, owner 3 after tick 2, anyone else after tick 3.
fn loot_tick(floor: &FloorItem, picker: Option<CharId>) -> u64 {
    match floor.owners[0] {
        None => 0,
        Some(first) if Some(first) == picker => 0,
        _ if picker.is_some() && floor.owners[1] == picker => floor.owner_ticks[0],
        _ if picker.is_some() && floor.owners[2] == picker => floor.owner_ticks[1],
        _ => floor.owner_ticks[2],
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ro_core::{
        AccountId, BroadcastScope, CharacterInventory, CharacterSpawn, ClientIntent, ItemId,
        ScopedServerEvent, SessionId, SessionIntent, Sex,
    };
    use ro_db::{MapGrid, MobDb};

    use super::*;
    use crate::inventory::tests::test_items;

    fn char_id(n: u32) -> CharId {
        CharId(150_000 + n)
    }

    fn engine_with(capacity: u32) -> MapInstance {
        let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
        let mut engine = MapInstance::with_capacity("prontera.gat", grid, capacity);
        engine.set_registries(Arc::new(test_items()), Arc::new(MobDb::default()));
        engine
    }

    fn enter(engine: &mut MapInstance, n: u32, cell: (u16, u16)) {
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
                x: cell.0,
                y: cell.1,
                dir: 0,
            },
            speed: 150,
            status: Default::default(),
            group_id: 0,
            inventory: CharacterInventory {
                slots: vec![Some(InventoryItem::new(ItemId(501), 5))],
                unique_item_counter: 0,
            },
        };
        engine.spawn_character(spawn, 0).expect("spawn");
        send(engine, n, ClientIntent::NotifyActorInit, 0);
    }

    fn send(
        engine: &mut MapInstance,
        n: u32,
        intent: ClientIntent,
        now: u64,
    ) -> Vec<ScopedServerEvent> {
        engine.push_intent(SessionIntent::new(
            SessionId(u64::from(n)),
            Some(AccountId(2_000_000 + n)),
            Some(char_id(n)),
            intent,
        ));
        let mut out = Vec::new();
        engine.step_tick(now, &mut out).expect("tick");
        out
    }

    fn jellopy() -> InventoryItem {
        InventoryItem::new(ItemId(909), 1)
    }

    fn result_of(events: &[ScopedServerEvent], n: u32) -> Option<u8> {
        events.iter().find_map(|e| match (&e.scope, &e.event) {
            (BroadcastScope::SelfOnly(c), ServerEvent::ItemAddFailed { result })
                if *c == char_id(n) =>
            {
                Some(*result)
            }
            (BroadcastScope::SelfOnly(c), ServerEvent::ItemAdded { .. }) if *c == char_id(n) => {
                Some(0)
            }
            _ => None,
        })
    }

    /// ENG-ITEM-04: a dead picker gets `0x0080` type 1 to the area (self too), then result 6.
    #[test]
    fn eng_item_04_dead_picker() {
        let mut engine = engine_with(64);
        enter(&mut engine, 1, (50, 50));
        enter(&mut engine, 2, (52, 50));
        let floor = engine
            .add_floor_item(jellopy(), (50, 51), FloorItemSpec::default(), 0)
            .expect("created");
        let id = engine.gid(floor);
        engine
            .step_tick(0, &mut Vec::new())
            .expect("flush the 0x0add");
        let e = engine.world.entity_by_char(char_id(1)).expect("A");
        engine.world.dead_sit[e.index()] = DEAD;
        let events = send(
            &mut engine,
            1,
            ClientIntent::PickUpItem { floor_id: id },
            20,
        );
        assert_eq!(events.len(), 2, "{events:#?}");
        assert_eq!(
            events[0],
            ScopedServerEvent::new(
                BroadcastScope::Characters(vec![char_id(1), char_id(2)]),
                ServerEvent::UnitVanished {
                    gid: 2_000_001,
                    vanish_type: 1
                }
            )
        );
        assert_eq!(result_of(&events, 1), Some(6));
        assert!(engine.world.floor_items.contains(floor), "the item stays");
    }

    /// ENG-ITEM-07: a dead dropper gets `0x00af` count 0 only.
    #[test]
    fn eng_item_07_dead_dropper() {
        let mut engine = engine_with(64);
        enter(&mut engine, 1, (50, 50));
        let e = engine.world.entity_by_char(char_id(1)).expect("A");
        engine.world.dead_sit[e.index()] = DEAD;
        let events = send(
            &mut engine,
            1,
            ClientIntent::DropItem {
                index: 2,
                amount: 1,
            },
            20,
        );
        assert_eq!(
            events,
            vec![ScopedServerEvent::new(
                BroadcastScope::SelfOnly(char_id(1)),
                ServerEvent::ItemThrown {
                    index: 2,
                    amount: 0
                }
            )]
        );
    }

    /// ENG-ITEM-04: the loot tick per owner tier.
    #[test]
    fn eng_item_04_owner_tiers() {
        let mut engine = engine_with(64);
        for n in 1..=4 {
            enter(&mut engine, n, (50, 50));
        }
        let spec = FloorItemSpec {
            owners: [Some(char_id(1)), Some(char_id(2)), Some(char_id(3))],
            owner_ticks: [3_000, 5_000, 7_000],
            ..FloorItemSpec::default()
        };
        // Owner 1 takes it at once.
        let floor = engine
            .add_floor_item(jellopy(), (50, 51), spec, 0)
            .expect("item");
        let id = engine.gid(floor);
        assert_eq!(
            result_of(
                &send(&mut engine, 1, ClientIntent::PickUpItem { floor_id: id }, 0),
                1
            ),
            Some(0)
        );

        // Each later tier waits for the previous tier's tick.
        for (n, ready) in [(2, 3_000), (3, 5_000), (4, 7_000)] {
            let floor = engine
                .add_floor_item(jellopy(), (50, 51), spec, 0)
                .expect("item");
            let id = engine.gid(floor);
            let early = send(
                &mut engine,
                n,
                ClientIntent::PickUpItem { floor_id: id },
                ready - 1,
            );
            assert_eq!(result_of(&early, n), Some(6), "char {n} too early");
            let on_time = send(
                &mut engine,
                n,
                ClientIntent::PickUpItem { floor_id: id },
                ready,
            );
            assert_eq!(result_of(&on_time, n), Some(0), "char {n} on time");
        }

        // No owner: anyone at once.
        let floor = engine
            .add_floor_item(jellopy(), (50, 51), FloorItemSpec::default(), 7_000)
            .expect("item");
        let id = engine.gid(floor);
        assert_eq!(
            result_of(
                &send(
                    &mut engine,
                    4,
                    ClientIntent::PickUpItem { floor_id: id },
                    7_000
                ),
                4
            ),
            Some(0)
        );
    }

    /// ENG-ITEM-08: the id counter wraps and skips ids in use; running out fails cleanly.
    #[test]
    fn eng_item_08_ids_wrap_and_capacity() {
        let mut engine = engine_with(64);
        engine.next_floor_id = MAX_FLOOR_ID - 1;
        let last = engine
            .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
            .expect("item");
        assert_eq!(engine.gid(last), MAX_FLOOR_ID - 1);
        let wrapped = engine
            .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
            .expect("item");
        assert_eq!(engine.gid(wrapped), MIN_FLOOR_ID);
        engine.next_floor_id = MIN_FLOOR_ID;
        let skipped = engine
            .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
            .expect("item");
        assert_eq!(engine.gid(skipped), MIN_FLOOR_ID + 1, "2 is taken");

        let mut tiny = engine_with(2);
        assert!(tiny
            .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
            .is_some());
        assert!(tiny
            .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
            .is_some());
        let before = tiny.next_floor_id;
        assert!(
            tiny.add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
                .is_none(),
            "capacity exhausted"
        );
        assert_eq!(tiny.world.floor_items.len(), 2);
        assert!(tiny.floor_expiry.len() == 2 && tiny.next_floor_id != before);
    }
}
