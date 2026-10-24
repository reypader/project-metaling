//! Mob drops: the roll and the placement (spec `specs/items/spec.md` §2.6; catalog
//! `mechanics/mobs.md` §4). Not called at mob death yet: there is no combat. The combat feature
//! calls `drop_mob_items` in the death tick, after the exp packets (PF-15).

use std::sync::Arc;

use ro_core::{CharId, InventoryItem, ItemId};
use ro_db::{ItemDb, ItemType, MobData, MobId, MAX_DROP_RATE};

use crate::engine::MapInstance;
use crate::floor_item::FloorItemSpec;
use crate::world::EntityKind;

/// rAthena `drop_rate_cap`: bonuses can't push a rate past 90% unless the base rate already was.
const DROP_RATE_CAP: u16 = 9000;
/// Loot-priority waits (ms) per owner tier: normal mobs, then boss/MVP mobs.
const NORMAL_OWNER_WAITS: [u64; 3] = [3000, 2000, 2000];
const BOSS_OWNER_WAITS: [u64; 3] = [10_000, 10_000, 2000];
/// Directions the drops after the first one cycle through: SE, W, N (one-cell offsets).
const DROP_OFFSETS: [(i32, i32); 3] = [(1, -1), (-1, 0), (0, 1)];

/// A dead mob's drop: what died, where, and who has loot priority (top damage dealers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobDropRequest {
    pub mob_id: MobId,
    pub cell: (u16, u16),
    pub owners: [Option<CharId>; 3],
}

/// rAthena's drop roll without the deferred parts (level penalty, player bonuses): each `Drops`
/// entry in db order drops one item when `roll() % 10000 < rate`.
pub fn roll_mob_drops(mob: &MobData, mut roll: impl FnMut() -> u32) -> Vec<ItemId> {
    let mut dropped = Vec::new();
    for entry in &mob.drops {
        let base = entry.rate;
        // Bonuses would be applied here; with none the rate is the base rate.
        let mut rate = base;
        if rate > DROP_RATE_CAP && base <= DROP_RATE_CAP {
            rate = DROP_RATE_CAP;
        }
        let rate = rate.clamp(1, MAX_DROP_RATE);
        if roll() % u32::from(MAX_DROP_RATE) < u32::from(rate) {
            dropped.push(entry.item);
        }
    }
    dropped
}

/// rAthena `itemdb_isidentified`: equipment drops unidentified, everything else identified.
fn identified_drop(items: &ItemDb, id: ItemId) -> bool {
    !items.get(id).is_some_and(|d| {
        matches!(
            d.item_type,
            ItemType::Weapon | ItemType::Armor | ItemType::PetArmor | ItemType::ShadowGear
        )
    })
}

impl MapInstance {
    /// Rolls the mob's drops with this instance's RNG and places them (`drop_items`). Returns
    /// the number of items placed; an unknown mob drops nothing.
    pub fn drop_mob_items(&mut self, request: &MobDropRequest, now_ms: u64) -> usize {
        let mob_db = Arc::clone(&self.mob_db);
        let Some(mob) = mob_db.get(request.mob_id) else {
            return 0;
        };
        let rng = &mut self.rng;
        let items = roll_mob_drops(mob, || rng.below(u64::from(u32::MAX)) as u32);
        self.drop_items(request, &items, now_ms)
    }

    /// Places already-rolled mob drops (spec §2.6): the first on the death cell, the rest on the
    /// SE / W / N cycle of one-cell offsets (a free neighbour of a blocked offset, else the
    /// offset itself). Each gets the loot ticks of the mob's boss type and its `DropEffect`, and
    /// a `0x0add` to everyone in view. Returns how many were placed; running out of ids or
    /// entity slots stops placement.
    pub fn drop_items(&mut self, request: &MobDropRequest, items: &[ItemId], now_ms: u64) -> usize {
        let boss = self
            .mob_db
            .get(request.mob_id)
            .is_some_and(|m| m.boss_type() != 0);
        let waits = if boss {
            BOSS_OWNER_WAITS
        } else {
            NORMAL_OWNER_WAITS
        };
        let mut owner_ticks = [0; 3];
        let mut at = now_ms;
        for (tick, wait) in owner_ticks.iter_mut().zip(waits) {
            at += wait;
            *tick = at;
        }
        let mut placed = 0;
        for (k, &id) in items.iter().enumerate() {
            let cell = if k == 0 {
                request.cell
            } else {
                let offset = DROP_OFFSETS[(k - 1) % DROP_OFFSETS.len()];
                self.mob_drop_cell(request.cell, offset)
            };
            let effect = self.items.get(id).map_or(0, |d| d.drop_effect);
            let spec = FloorItemSpec {
                owners: request.owners,
                owner_ticks,
                mob_id: request.mob_id.0,
                show_drop_effect: effect > 0,
                drop_effect_mode: u16::from(effect.saturating_sub(1)),
            };
            let mut item = InventoryItem::new(id, 1);
            item.identified = identified_drop(&self.items, id);
            if self.add_floor_item(item, cell, spec, now_ms).is_none() {
                break;
            }
            placed += 1;
        }
        placed
    }

    /// The cell for a non-first drop: the offset cell if free, else a free neighbour of it,
    /// else the offset cell anyway. An offset off the map falls back to the death cell.
    fn mob_drop_cell(&mut self, center: (u16, u16), (dx, dy): (i32, i32)) -> (u16, u16) {
        let x = i32::from(center.0) + dx;
        let y = i32::from(center.1) + dy;
        let (Ok(ox), Ok(oy)) = (u16::try_from(x), u16::try_from(y)) else {
            return center;
        };
        if ox >= self.walkability.width() || oy >= self.walkability.height() {
            return center;
        }
        if self.mob_drop_cell_free((x, y)) {
            return (ox, oy);
        }
        self.search_free_cell((ox, oy), 1, |this, c| this.mob_drop_cell_free(c))
            .unwrap_or((ox, oy))
    }

    /// rAthena `map_cell_free` for mob drops: walkable, 5 cells from the edge, and no player or
    /// mob standing on it. Floor items and NPCs don't block.
    fn mob_drop_cell_free(&self, cell: (i32, i32)) -> bool {
        if !self.drop_cell_free(cell) {
            return false;
        }
        let (Ok(x), Ok(y)) = (u16::try_from(cell.0), u16::try_from(cell.1)) else {
            return false;
        };
        let mut occupied = false;
        let kinds = &self.world.kinds;
        self.world
            .grid
            .visit_radius(x, y, 0, &self.world.positions, |e| {
                occupied |= matches!(
                    kinds.get(e.index()),
                    Some(EntityKind::Player | EntityKind::Mob)
                );
            });
        !occupied
    }
}
