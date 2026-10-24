//! Warp portals: the trigger-cell index, destination resolution and the player warp (spec
//! `specs/warp-map-change/spec.md` §1, ENG-WARP-01..07).

use std::sync::Arc;

use ro_core::{CharacterSpawn, MapPosition, ServerEvent};
use ro_db::NpcState;
use tracing::warn;

use crate::engine::{MapInstance, VANISH_OUT_OF_SIGHT};
use crate::entity::Entity;
use crate::error::EngineError;
use crate::settings::Arrival;
use crate::world::WarpTrigger;

/// rAthena `dead_sit` value of a dead unit.
const DEAD: u8 = 1;
/// A trigger with a higher `rewarp_count` is skipped (rAthena `npc_touch_areanpc`).
const MAX_REWARP: u8 = 10;

/// A player that left this map through a cross-map warp. The map-server hands it to the
/// destination map, which calls [`MapInstance::enter_by_warp`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTransfer {
    /// Identity and look, rebuilt from the entity; `pos` is `from_pos`.
    pub spawn: CharacterSpawn,
    /// Destination map as a `.gat` name.
    pub dest_map: Arc<str>,
    /// The warp's raw destination; the destination map resolves it.
    pub dest: (u16, u16),
    pub rewarp_count: u8,
    /// The cell that fired the warp, on this map.
    pub from_pos: MapPosition,
}

/// One bit per map cell.
#[derive(Debug)]
pub(crate) struct CellBits {
    width: u16,
    height: u16,
    words: Vec<u64>,
}

impl CellBits {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        let cells = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            words: vec![0; cells.div_ceil(64)],
        }
    }

    fn bit(&self, x: u16, y: u16) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    pub(crate) fn set(&mut self, x: u16, y: u16) {
        if let Some(b) = self.bit(x, y) {
            if let Some(word) = self.words.get_mut(b / 64) {
                *word |= 1 << (b % 64);
            }
        }
    }

    pub(crate) fn get(&self, x: u16, y: u16) -> bool {
        self.bit(x, y)
            .and_then(|b| self.words.get(b / 64).map(|w| w & (1 << (b % 64)) != 0))
            .unwrap_or(false)
    }

    /// Whether any set cell lies within Chebyshev distance 1 of `(x, y)`.
    fn any_near(&self, x: u16, y: u16) -> bool {
        let xs = x.saturating_sub(1)..=x.saturating_add(1);
        xs.into_iter()
            .any(|cx| (y.saturating_sub(1)..=y.saturating_add(1)).any(|cy| self.get(cx, cy)))
    }
}

/// Whether `cell` is inside the warp's square area around `center`.
fn area_contains(center: (u16, u16), half_extents: (u16, u16), cell: (u16, u16)) -> bool {
    center.0.abs_diff(cell.0) <= half_extents.0 && center.1.abs_diff(cell.1) <= half_extents.1
}

impl MapInstance {
    /// Marks the walkable cells of a warp's area at `center`, clipped to the map (ENG-WARP-01).
    pub(crate) fn mark_warp_cells(&mut self, center: (u16, u16), half_extents: (u16, u16)) {
        let (w, h) = (self.walkability.width(), self.walkability.height());
        if w == 0 || h == 0 {
            return;
        }
        let x0 = center.0.saturating_sub(half_extents.0);
        let x1 = center.0.saturating_add(half_extents.0).min(w - 1);
        let y0 = center.1.saturating_sub(half_extents.1);
        let y1 = center.1.saturating_add(half_extents.1).min(h - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if self.walkability.is_walkable(x, y) {
                    self.warp_cells.set(x, y);
                }
            }
        }
    }

    /// The first warp in spawn order (lowest gid) whose area holds `cell`, among those that
    /// `include` accepts.
    fn warp_area_at(&self, cell: (u16, u16), include: fn(NpcState) -> bool) -> Option<Entity> {
        let warps = &self.world.warps;
        let mut best: Option<(u32, Entity)> = None;
        for (&w, trigger) in warps.entities().iter().zip(warps.values()) {
            let Some(npc) = self.world.npcs.get(w) else {
                continue;
            };
            let center = self.world.positions[w.index()];
            if !include(npc.state)
                || !area_contains((center.x, center.y), trigger.half_extents, cell)
            {
                continue;
            }
            if best.is_none_or(|(gid, _)| npc.gid < gid) {
                best = Some((npc.gid, w));
            }
        }
        best.map(|(_, w)| w)
    }

    /// Fires the warp under player `e`'s cell, if any. Runs after each completed step and at
    /// the end of `actor_init`.
    pub(crate) fn touch_warps(&mut self, e: Entity) {
        let (x, y) = self.cell(e);
        if !self.warp_cells.get(x, y) {
            return;
        }
        let fires = |state| !matches!(state, NpcState::Disabled | NpcState::Cloaked);
        let Some(w) = self.warp_area_at((x, y), fires) else {
            return;
        };
        if let Some(trigger) = self.world.warps.get(w).cloned() {
            self.warp_player(e, &trigger);
        }
    }

    fn warp_player(&mut self, e: Entity, trigger: &WarpTrigger) {
        let i = e.index();
        if self.world.dead_sit[i] == DEAD {
            return;
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if info.rewarp_count > MAX_REWARP {
            warn!(
                map = %self.map_name,
                char_id = ?info.char_id,
                "warp loop guard: skipping warp after {} consecutive warps",
                info.rewarp_count
            );
            info.rewarp_count = 0;
            return;
        }
        if trigger.same_map {
            self.warp_same_map(e, trigger.dest);
        } else {
            self.warp_cross_map(e, trigger);
        }
    }

    /// Vanish to the AoI, despawn, and queue a `MapTransfer`. No `0x0091` from here: the
    /// destination sends it once the cell is resolved.
    fn warp_cross_map(&mut self, e: Entity, trigger: &WarpTrigger) {
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let from_pos = self.pos(e);
        let inventory = self
            .world
            .inventories
            .remove(e)
            .map(|i| i.into_character_inventory())
            .unwrap_or_default();
        let spawn = CharacterSpawn {
            account_id: info.account_id,
            char_id: info.char_id,
            name: info.name.to_string(),
            sex: info.sex,
            job_class: info.job,
            base_level: info.clevel,
            hair_style: info.hair_style,
            hair_color: info.hair_color,
            pos: from_pos,
            speed: self.world.speed[e.index()],
            status: info.status,
            group_id: info.group_id,
            inventory,
        };
        let rewarp_count = info.rewarp_count;
        if let Err(err) = self.despawn_player(e, spawn.char_id, VANISH_OUT_OF_SIGHT) {
            warn!(map = %self.map_name, char_id = ?spawn.char_id, "warp despawn failed: {err}");
            // The player stays here: give the inventory back.
            if self.world.entities.is_alive(e) {
                let inventory = crate::inventory::PlayerInventory::new(
                    spawn.inventory,
                    spawn.char_id,
                    spawn.status.str,
                    &self.items,
                );
                self.world.inventories.insert(e, inventory);
            }
            return;
        }
        self.transfers.push(MapTransfer {
            spawn,
            dest_map: Arc::clone(&trigger.dest_map),
            dest: trigger.dest,
            rewarp_count,
            from_pos,
        });
    }

    fn warp_same_map(&mut self, e: Entity, dest: (u16, u16)) {
        let Some(cell) = self.resolve_dest(dest) else {
            warn!(map = %self.map_name, ?dest, "warp: no free random cell; not warping");
            return;
        };
        let gid = self.gid(e);
        self.emit_aoi(
            e,
            true,
            ServerEvent::UnitVanished {
                gid,
                vanish_type: VANISH_OUT_OF_SIGHT,
            },
        );
        self.world.hide(e);
        let i = e.index();
        self.world.walking_paths.remove(e);
        self.world.pending_move[i] = None;
        self.world.positions[i].x = cell.0;
        self.world.positions[i].y = cell.1;
        self.update_rewarp(e, cell);
        let dest_map = self.gat_name.to_string();
        self.emit_self(
            e,
            ServerEvent::PlayerMapMove {
                dest_map,
                dest_x: cell.0,
                dest_y: cell.1,
            },
        );
    }

    /// Resolves a warp destination on this map (ENG-WARP-07): in bounds is used as-is (even a
    /// wall), out of bounds becomes `(0, 0)`, and `(0, 0)` rolls a random free cell.
    pub(crate) fn resolve_dest(&mut self, dest: (u16, u16)) -> Option<(u16, u16)> {
        let (w, h) = (self.walkability.width(), self.walkability.height());
        let dest = if dest.0 >= w || dest.1 >= h {
            warn!(map = %self.map_name, ?dest, "warp destination out of bounds; using a random cell");
            (0, 0)
        } else {
            dest
        };
        if dest != (0, 0) {
            return Some(dest);
        }
        if w < 3 || h < 3 {
            return None;
        }
        let tries = u64::from(w) * u64::from(h) * 3;
        let (span_x, span_y) = (u64::from(w - 2), u64::from(h - 2));
        for _ in 0..tries {
            let x = self.rng.below(span_x) as u16 + 1;
            let y = self.rng.below(span_y) as u16 + 1;
            // rAthena checks warp areas within range 1, skipping disabled warps; the bitmap
            // also holds disabled warps' cells and leaves out walls inside areas.
            if self.walkability.is_walkable(x, y) && !self.warp_cells.any_near(x, y) {
                return Some((x, y));
            }
        }
        None
    }

    /// After a warp to `cell`: count it if `cell` is in an enabled warp area, else reset
    /// (ENG-WARP-06).
    pub(crate) fn update_rewarp(&mut self, e: Entity, cell: (u16, u16)) {
        let enabled = |state| state != NpcState::Disabled;
        let in_area = self.warp_area_at(cell, enabled).is_some();
        if let Some(info) = self.world.players.get_mut(e) {
            info.rewarp_count = if in_area {
                info.rewarp_count.saturating_add(1)
            } else {
                0
            };
        }
    }
}

/// Cross-map warp API driven by the map-server actor.
impl MapInstance {
    /// Moves this tick's outgoing transfers into `out` (the caller reuses the buffer).
    pub fn drain_transfers(&mut self, out: &mut Vec<MapTransfer>) {
        out.append(&mut self.transfers);
    }

    /// Spawns a player arriving by warp (ENG-WARP-08): resolves `dest` on this map (the map
    /// centre if no random cell is free), spawns the player out of the grid, and updates its
    /// `rewarp_count`. Emits nothing: the caller sends the `0x0091` with the returned cell, and
    /// the client's `NotifyActorInit` shows the player. A character already here is an error
    /// and changes nothing.
    pub fn enter_by_warp(
        &mut self,
        mut spawn: CharacterSpawn,
        dest: (u16, u16),
        rewarp_count: u8,
        _now_ms: u64,
    ) -> Result<MapPosition, EngineError> {
        if self.world.entity_by_char(spawn.char_id).is_some()
            || self.world.entity_by_gid(spawn.account_id.0).is_some()
        {
            return Err(EngineError::CharacterAlreadyEntered(spawn.char_id));
        }
        let cell = self.resolve_dest(dest).unwrap_or_else(|| {
            let centre = (self.walkability.width() / 2, self.walkability.height() / 2);
            warn!(map = %self.map_name, ?dest, ?centre, "warp: no free random cell; using the map centre");
            centre
        });
        spawn.pos.x = cell.0;
        spawn.pos.y = cell.1;
        let e = self.spawn_player(&mut spawn, Arrival::MapChange)?;
        if let Some(info) = self.world.players.get_mut(e) {
            info.rewarp_count = rewarp_count;
        }
        self.update_rewarp(e, cell);
        Ok(spawn.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_bits_set_get_and_clip() {
        let mut bits = CellBits::new(10, 7);
        bits.set(9, 6);
        bits.set(10, 0);
        assert!(bits.get(9, 6));
        assert!(!bits.get(10, 0), "out of bounds is never set");
        assert!(!bits.get(8, 6));
        assert!(bits.any_near(8, 5));
        assert!(!bits.any_near(7, 6));
    }

    use std::sync::Arc;

    use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, Sex};
    use ro_db::{MapGrid, MapNpcs, NpcKind, NpcSpawn, WarpDest};

    fn warp(at: (u16, u16), dest: (u16, u16)) -> NpcSpawn {
        NpcSpawn {
            name: "w".into(),
            unique_name: "w".into(),
            x: at.0,
            y: at.1,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state: NpcState::Normal,
            trigger: Some((1, 1)),
            duplicate_of: None,
            kind: NpcKind::Warp(WarpDest {
                map: "prontera".into(),
                x: dest.0,
                y: dest.1,
                trigger_while_hidden: false,
            }),
            source: "t:1".into(),
        }
    }

    /// A map with the given warps and one player (not yet in the grid) at `at`.
    fn setup(npcs: Vec<NpcSpawn>, at: (u16, u16)) -> (MapInstance, Entity) {
        let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("valid dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        engine.spawn_npcs(&MapNpcs { npcs });
        let spawn = CharacterSpawn {
            account_id: AccountId(2_000_001),
            char_id: CharId(150_001),
            name: "A".into(),
            sex: Sex::Female,
            job_class: 0,
            base_level: 1,
            hair_style: 1,
            hair_color: 1,
            pos: MapPosition {
                x: at.0,
                y: at.1,
                dir: 0,
            },
            speed: 150,
            status: Default::default(),
            group_id: 0,
            inventory: Default::default(),
        };
        let e = engine.world.spawn_player(&spawn).expect("spawn");
        engine.world.show(e);
        (engine, e)
    }

    fn rewarp_count(engine: &MapInstance, e: Entity) -> u8 {
        engine.world.players.get(e).expect("player").rewarp_count
    }

    /// ENG-WARP-04: a dead player never warps.
    #[test]
    fn eng_warp_04_dead_player_does_not_warp() {
        let (mut engine, e) = setup(vec![warp((50, 50), (80, 80))], (50, 50));
        engine.world.dead_sit[e.index()] = DEAD;
        engine.touch_warps(e);
        assert!(engine.events.is_empty());
        assert_eq!(engine.cell(e), (50, 50));
        assert!(engine.world.in_grid[e.index()]);
    }

    /// ENG-WARP-06: the count grows on landings inside an area and resets elsewhere.
    #[test]
    fn eng_warp_06_rewarp_count_increments_and_resets() {
        let (mut engine, e) = setup(
            vec![warp((50, 50), (80, 80)), warp((80, 80), (20, 20))],
            (50, 50),
        );
        engine.touch_warps(e);
        assert_eq!(engine.cell(e), (80, 80));
        assert_eq!(rewarp_count(&engine, e), 1, "landed in the second area");
        engine.world.show(e);
        engine.touch_warps(e);
        assert_eq!(engine.cell(e), (20, 20));
        assert_eq!(rewarp_count(&engine, e), 0, "landed outside every area");
    }
}
