//! Permanent mob spawns: spawn groups, placement and creation (spec
//! `specs/mob-spawning/spec.md` ENG-MOB-01..03; catalog `mechanics/mobs.md` §1.1, §1.3, §1.4).
//!
//! Placement follows rAthena's defaults: `randomize_center_cell: yes`, `force_random_spawn: no`,
//! `no_spawn_on_player: 0`. Mobs have placeholder stats until mob_db stats are loaded.

use std::sync::Arc;

use ro_core::MapPosition;
use ro_db::{MapGrid, MapMobSpawns, MobId, MobSpawn};
use tracing::warn;

use crate::engine::MapInstance;
use crate::error::EngineError;
use crate::rng::SplitMix64;
use crate::world::MobInfo;

/// HP and max HP of every mob until mob_db stats exist.
pub(crate) const PLACEHOLDER_MOB_HP: u32 = 1;
/// Level of a mob whose spawn line gives none, until mob_db stats exist.
pub(crate) const PLACEHOLDER_MOB_LEVEL: u16 = 1;
/// rAthena's default mob `WalkSpeed`.
pub(crate) const DEFAULT_MOB_SPEED: u16 = 150;

/// rAthena `map_edge_size`: a map-wide search stays this far from the edges.
const MAP_EDGE: i32 = 15;
/// A free-cell search rejects cells closer than this to the edge.
const EDGE_GAP: i32 = 5;
/// Tries around the center, then map-wide tries when the center isn't walkable.
const NEAR_TRIES: u32 = 8;
const MAP_WIDE_TRIES: u32 = 50;

/// A spawn line with the rectangle rules applied for this map. Mobs keep its index so respawn
/// can reuse it.
#[derive(Debug)]
pub(crate) struct SpawnGroup {
    /// The line as loaded (delays, boss flag and event are for respawn and death).
    pub(crate) line: MobSpawn,
    /// Shared by every mob of the group.
    pub(crate) name: Arc<str>,
    /// `None` means map-wide.
    center: Option<(i32, i32)>,
    /// `(xs, ys)` after the rectangle rules; `(0, 0)` when map-wide.
    span: (i32, i32),
}

impl SpawnGroup {
    /// ENG-MOB-02 steps 1-2. `line` must lie inside `grid`.
    fn new(line: &MobSpawn, grid: &MapGrid) -> Self {
        let (x, y) = (i32::from(line.x), i32::from(line.y));
        let mut span = (i32::from(line.span.0), i32::from(line.span.1));
        if span.0 == 0 && x > 0 {
            span.0 = 1;
        }
        if span.1 == 0 && y > 0 {
            span.1 = 1;
        }
        let map_wide = (x == 0 && y == 0) || (span == (1, 1) && !grid.is_walkable(line.x, line.y));
        let (center, span) = if map_wide {
            (None, (0, 0))
        } else {
            (Some((x, y)), span)
        };
        Self {
            line: line.clone(),
            name: Arc::from(line.name.as_str()),
            center,
            span,
        }
    }

    /// ENG-MOB-02 step 3: this mob's own center, randomized inside the rectangle and clamped to
    /// the grid. `None` when map-wide.
    fn mob_center(&self, rng: &mut SplitMix64, grid: &MapGrid) -> Option<(i32, i32)> {
        let (x, y) = self.center?;
        let (xs, ys) = self.span;
        let cx = if xs > 1 {
            rng.range_inclusive(x - xs + 1, x + xs - 1)
        } else {
            x
        };
        let cy = if ys > 1 {
            rng.range_inclusive(y - ys + 1, y + ys - 1)
        } else {
            y
        };
        let (w, h) = (i32::from(grid.width()), i32::from(grid.height()));
        Some((cx.clamp(0, w - 1), cy.clamp(0, h - 1)))
    }

    /// ENG-MOB-02 step 4: the cell for one mob, or `None` when every try fails.
    fn place(
        &self,
        rng: &mut SplitMix64,
        grid: &MapGrid,
        center: Option<(i32, i32)>,
    ) -> Option<(u16, u16)> {
        let (xs, ys) = self.span;
        if let Some(c) = center {
            if self.line.boss && self.span == (1, 1) {
                return walkable_cell(grid, c);
            }
            // In i64: spans are not bounded by the grid, so `xs * ys` could overflow i32.
            let area = u64::try_from(i64::from(xs) * i64::from(ys))
                .unwrap_or(0)
                .max(1);
            if !self.line.boss && xs + ys >= 1 && rng.below(area) == 0 {
                if let Some(cell) = walkable_cell(grid, c) {
                    return Some(cell);
                }
            }
        }
        if let Some(cell) = search(rng, grid, center, (xs - 1, ys - 1), NEAR_TRIES) {
            return Some(cell);
        }
        if let Some(cell) = center.and_then(|c| walkable_cell(grid, c)) {
            return Some(cell);
        }
        search(rng, grid, center, (-1, -1), MAP_WIDE_TRIES)
    }
}

/// rAthena `map_search_freecell` without player avoidance: random cells within `radius` of
/// `base` (the whole map inside `MAP_EDGE` when there is no base or a radius is negative). A
/// try fails on the base cell itself, near an edge, or on an unwalkable cell.
fn search(
    rng: &mut SplitMix64,
    grid: &MapGrid,
    base: Option<(i32, i32)>,
    (rx, ry): (i32, i32),
    tries: u32,
) -> Option<(u16, u16)> {
    let (w, h) = (i32::from(grid.width()), i32::from(grid.height()));
    for _ in 0..tries {
        let (x, y) = match base {
            Some((bx, by)) if rx >= 0 && ry >= 0 => (
                rng.range_inclusive(bx - rx, bx + rx),
                rng.range_inclusive(by - ry, by + ry),
            ),
            _ => (
                rng.range_inclusive(MAP_EDGE, w - MAP_EDGE - 1),
                rng.range_inclusive(MAP_EDGE, h - MAP_EDGE - 1),
            ),
        };
        if base == Some((x, y)) {
            continue;
        }
        if x < EDGE_GAP || x > w - EDGE_GAP || y < EDGE_GAP || y > h - EDGE_GAP {
            continue;
        }
        if let Some(cell) = walkable_cell(grid, (x, y)) {
            return Some(cell);
        }
    }
    None
}

/// `(x, y)` as a cell when it is on the grid and walkable.
fn walkable_cell(grid: &MapGrid, (x, y): (i32, i32)) -> Option<(u16, u16)> {
    let cell = (u16::try_from(x).ok()?, u16::try_from(y).ok()?);
    grid.is_walkable(cell.0, cell.1).then_some(cell)
}

/// A read-only view of one spawned mob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobView<'a> {
    pub gid: u32,
    pub mob_id: MobId,
    pub name: &'a str,
    pub level: u16,
    pub hp: u32,
    pub max_hp: u32,
    pub pos: MapPosition,
    pub speed: u16,
    /// In the spatial grid (seen by players in range).
    pub visible: bool,
    /// The spawn line the mob came from.
    pub spawn: &'a MobSpawn,
}

impl MapInstance {
    /// Creates `Amount` mobs per line, in order, and returns how many it created (ENG-MOB-01).
    /// Lines outside the map are skipped; mobs with no free cell are not created; spawning
    /// stops when the entity capacity runs out. Emits no events.
    pub fn spawn_mobs(&mut self, spawns: &MapMobSpawns) -> usize {
        let mut created = 0;
        for line in &spawns.spawns {
            if line.x >= self.walkability.width() || line.y >= self.walkability.height() {
                warn!(
                    map = %self.map_name,
                    mob = %line.name,
                    source = %line.source,
                    "mob spawn line outside the map ({}, {}); skipped",
                    line.x,
                    line.y
                );
                continue;
            }
            let Ok(index) = u32::try_from(self.spawn_groups.len()) else {
                warn!(map = %self.map_name, "too many mob spawn groups; rest skipped");
                break;
            };
            self.spawn_groups
                .push(SpawnGroup::new(line, &self.walkability));
            let mut unplaced = 0;
            for _ in 0..line.amount {
                match self.spawn_mob(index) {
                    Ok(true) => created += 1,
                    Ok(false) => unplaced += 1,
                    Err(EngineError::WorldCapacityExceeded) => {
                        warn!(
                            map = %self.map_name,
                            source = %line.source,
                            "entity capacity reached; remaining mobs not spawned"
                        );
                        return created;
                    }
                    Err(err) => warn!(
                        map = %self.map_name,
                        source = %line.source,
                        "mob not spawned: {err}"
                    ),
                }
            }
            if unplaced > 0 {
                warn!(
                    map = %self.map_name,
                    mob = %line.name,
                    source = %line.source,
                    "{unplaced} of {} mobs found no free cell; not spawned",
                    line.amount
                );
            }
        }
        created
    }

    /// Places and creates one mob of a group. `Ok(false)` when no cell was found.
    fn spawn_mob(&mut self, index: u32) -> Result<bool, EngineError> {
        let Some(group) = self.spawn_groups.get(index as usize) else {
            return Ok(false);
        };
        let center = group.mob_center(&mut self.rng, &self.walkability);
        let Some((x, y)) = group.place(&mut self.rng, &self.walkability, center) else {
            return Ok(false);
        };
        // Never reuse a gid that is still taken (only possible after the counter wraps).
        while self.world.entity_by_gid(self.next_npc_id).is_some() {
            self.next_npc_id = self.next_npc_id.wrapping_add(1);
        }
        let info = MobInfo {
            gid: self.next_npc_id,
            mob_id: group.line.mob_id,
            name: Arc::clone(&group.name),
            level: group.line.level.unwrap_or(PLACEHOLDER_MOB_LEVEL),
            hp: PLACEHOLDER_MOB_HP,
            max_hp: PLACEHOLDER_MOB_HP,
            spawn_group: index,
        };
        let pos = MapPosition { x, y, dir: 0 };
        let e = self.world.spawn_mob(info, pos, DEFAULT_MOB_SPEED)?;
        self.next_npc_id = self.next_npc_id.wrapping_add(1);
        self.world.show(e);
        Ok(true)
    }

    /// Every mob on the map, in no particular order.
    pub fn mobs(&self) -> impl Iterator<Item = MobView<'_>> {
        let world = &self.world;
        world
            .mobs
            .entities()
            .iter()
            .zip(world.mobs.values())
            .filter_map(move |(&e, m)| {
                let i = e.index();
                Some(MobView {
                    gid: m.gid,
                    mob_id: m.mob_id,
                    name: &m.name,
                    level: m.level,
                    hp: m.hp,
                    max_hp: m.max_hp,
                    pos: *world.positions.get(i)?,
                    speed: *world.speed.get(i)?,
                    visible: *world.in_grid.get(i)?,
                    spawn: &self.spawn_groups.get(m.spawn_group as usize)?.line,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x: u16, y: u16, span: (u16, u16), boss: bool) -> MobSpawn {
        MobSpawn {
            name: "Poring".into(),
            mob_id: MobId(1002),
            x,
            y,
            span,
            amount: 1,
            delay1: 5000,
            delay2: 0,
            level: None,
            boss,
            event: None,
            source: "t:1".into(),
        }
    }

    fn grid_with_walls(w: u16, h: u16, walls: &[(u16, u16)]) -> MapGrid {
        let mut grid = MapGrid::all_walkable(w, h).expect("valid dims");
        for &(x, y) in walls {
            assert!(grid.set(x, y, ro_db::CellType::Wall), "in bounds");
        }
        grid
    }

    #[test]
    fn eng_mob_02_rectangle_rules() {
        let grid = grid_with_walls(100, 100, &[(40, 40)]);
        let g = SpawnGroup::new(&line(0, 0, (5, 5), false), &grid);
        assert_eq!((g.center, g.span), (None, (0, 0)), "0,0 is map-wide");
        let g = SpawnGroup::new(&line(30, 30, (0, 0), false), &grid);
        assert_eq!(
            (g.center, g.span),
            (Some((30, 30)), (1, 1)),
            "0 span becomes 1"
        );
        let g = SpawnGroup::new(&line(40, 40, (0, 0), false), &grid);
        assert_eq!(g.center, None, "1x1 on a wall is map-wide");
        let g = SpawnGroup::new(&line(40, 40, (3, 3), false), &grid);
        assert_eq!(
            (g.center, g.span),
            (Some((40, 40)), (3, 3)),
            "a wall center with a real rectangle stays"
        );
        let g = SpawnGroup::new(&line(30, 0, (0, 0), false), &grid);
        assert_eq!(
            g.span,
            (1, 0),
            "only an axis with a nonzero coordinate gets 1"
        );
    }

    #[test]
    fn eng_mob_02_centers_stay_in_rectangle_and_grid() {
        let grid = grid_with_walls(50, 50, &[]);
        let mut rng = SplitMix64::from_name("t");
        let g = SpawnGroup::new(&line(2, 48, (10, 4), false), &grid);
        for _ in 0..500 {
            let (cx, cy) = g.mob_center(&mut rng, &grid).expect("not map-wide");
            assert!((0..=11).contains(&cx), "{cx}: x-9..=x+9 clamped at 0");
            assert!((45..=49).contains(&cy), "{cy}: y-3..=y+3 clamped at 49");
        }
    }

    #[test]
    fn eng_mob_02_boss_1x1_takes_its_cell_and_wide_search_stays_off_edges() {
        let grid = grid_with_walls(60, 60, &[]);
        let mut rng = SplitMix64::from_name("t");
        let boss = SpawnGroup::new(&line(3, 3, (0, 0), true), &grid);
        for _ in 0..50 {
            let c = boss.mob_center(&mut rng, &grid);
            assert_eq!(boss.place(&mut rng, &grid, c), Some((3, 3)));
        }
        let wide = SpawnGroup::new(&line(0, 0, (0, 0), false), &grid);
        for _ in 0..500 {
            let (x, y) = wide.place(&mut rng, &grid, None).expect("free cell");
            assert!((15..=44).contains(&x) && (15..=44).contains(&y), "{x},{y}");
        }
    }

    /// Walls everywhere in `(cx-r..=cx+r, cy-r..=cy+r)` except the cells in `open`.
    fn walled_box(c: (u16, u16), r: u16, open: &[(u16, u16)]) -> MapGrid {
        let walls: Vec<_> = (c.0 - r..=c.0 + r)
            .flat_map(|x| (c.1 - r..=c.1 + r).map(move |y| (x, y)))
            .filter(|cell| !open.contains(cell))
            .collect();
        grid_with_walls(100, 100, &walls)
    }

    #[test]
    fn eng_mob_02_near_tries_fail_on_walkable_center_takes_the_center() {
        // Only the center is open around it, and the search never picks the center itself.
        let grid = walled_box((50, 50), 6, &[(50, 50)]);
        let mut rng = SplitMix64::from_name("t");
        let g = SpawnGroup::new(&line(50, 50, (3, 3), false), &grid);
        for _ in 0..200 {
            assert_eq!(g.place(&mut rng, &grid, Some((50, 50))), Some((50, 50)));
        }
    }

    #[test]
    fn eng_mob_02_unwalkable_center_falls_back_to_map_wide() {
        let grid = walled_box((50, 50), 6, &[]);
        let mut rng = SplitMix64::from_name("t");
        let g = SpawnGroup::new(&line(50, 50, (3, 3), false), &grid);
        for _ in 0..200 {
            let (x, y) = g
                .place(&mut rng, &grid, Some((50, 50)))
                .expect("a map-wide try succeeds");
            assert!(grid.is_walkable(x, y));
            assert!((15..=84).contains(&x) && (15..=84).contains(&y), "{x},{y}");
            assert!(
                x.abs_diff(50) > 6 || y.abs_diff(50) > 6,
                "outside the walls"
            );
        }
    }

    #[test]
    fn eng_mob_02_huge_span_does_not_overflow() {
        let grid = grid_with_walls(100, 100, &[]);
        let mut rng = SplitMix64::from_name("t");
        let g = SpawnGroup::new(&line(50, 50, (32767, 32767), false), &grid);
        let c = g.mob_center(&mut rng, &grid);
        assert!(g.place(&mut rng, &grid, c).is_some());
    }

    #[test]
    fn eng_mob_03_no_free_cell_is_none() {
        let walls: Vec<_> = (0..40u16)
            .flat_map(|x| (0..40u16).map(move |y| (x, y)))
            .collect();
        let grid = grid_with_walls(40, 40, &walls);
        let mut rng = SplitMix64::from_name("t");
        let g = SpawnGroup::new(&line(20, 20, (5, 5), false), &grid);
        let c = g.mob_center(&mut rng, &grid);
        assert_eq!(g.place(&mut rng, &grid, c), None);
    }
}
