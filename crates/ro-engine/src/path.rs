//! rAthena-compatible A* over a bounded window around the start cell.
//!
//! Rules (`reference/catalog/mechanics/movement_aoi.md` §1): step cost 10 orthogonal / 14
//! diagonal, heuristic Manhattan x 10, no corner cutting, the last row and column are never
//! walkable, the start cell is not checked, and a path longer than `MAX_WALKPATH` steps is
//! rejected. Any path of at most `MAX_WALKPATH` steps stays inside a window of that radius
//! around the start, so the scratch arrays are fixed-size and reused across searches.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use ro_db::MapWalkability;
use smallvec::SmallVec;

/// rAthena `MAX_WALKPATH`.
pub(crate) const MAX_WALKPATH: usize = 32;
/// rAthena `max_walk_path` default: the longest path a click may request.
pub(crate) const MAX_WALK_PATH_CLICK: usize = 17;

const MOVE_COST: u32 = 10;
const MOVE_DIAGONAL_COST: u32 = 14;

const RADIUS: i32 = MAX_WALKPATH as i32;
const SIDE: i32 = 2 * RADIUS + 1;
const WINDOW_CELLS: usize = (SIDE * SIDE) as usize;
const NO_PARENT: u8 = u8::MAX;

/// Neighbour expansion order from rAthena `path.cpp`: SE, E, NE, N, NW, W, SW, S (y+ is north).
const NEIGHBOURS: [(i32, i32); 8] = [
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
];

/// Reusable A* state: per-window-cell cost, parent direction and visit stamps, plus the open
/// heap and the output buffer. No allocation per search once the heap has warmed up.
#[derive(Debug)]
pub(crate) struct AStarScratchBuffer {
    g_cost: Vec<u32>,
    parent: Vec<u8>,
    open_stamp: Vec<u32>,
    closed_stamp: Vec<u32>,
    stamp: u32,
    /// (f cost, insertion order, window index); min-heap via `Reverse`.
    heap: BinaryHeap<Reverse<(u32, u32, u16)>>,
    out: SmallVec<[(u16, u16); MAX_WALKPATH]>,
}

impl Default for AStarScratchBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Map walkability with rAthena's bounds rule: the last row and column never pass.
struct Passable<'a> {
    map: &'a dyn MapWalkability,
    /// `width - 1` / `height - 1`: the first blocked column / row.
    limit_x: i32,
    limit_y: i32,
}

impl Passable<'_> {
    fn at(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && x < self.limit_x
            && y < self.limit_y
            && self.map.is_walkable(x as u16, y as u16)
    }
}

impl AStarScratchBuffer {
    pub(crate) fn new() -> Self {
        Self {
            g_cost: vec![0; WINDOW_CELLS],
            parent: vec![NO_PARENT; WINDOW_CELLS],
            open_stamp: vec![0; WINDOW_CELLS],
            closed_stamp: vec![0; WINDOW_CELLS],
            stamp: 0,
            heap: BinaryHeap::with_capacity(WINDOW_CELLS),
            out: SmallVec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn heap_capacity(&self) -> usize {
        self.heap.capacity()
    }

    fn next_stamp(&mut self) {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.open_stamp.fill(0);
            self.closed_stamp.fill(0);
            self.stamp = 1;
        }
    }

    /// Finds a path from `start` to `dest`. Returns the cells after `start` (empty when
    /// `start == dest`), or `None` when the destination is out of bounds or blocked, no path
    /// exists, or the path needs more than `max_steps` steps (capped at `MAX_WALKPATH`).
    ///
    /// Like rAthena's `add_path`, a closed node is reopened when a cheaper route to it turns
    /// up (the Manhattan heuristic is inconsistent with 14-cost diagonals).
    pub(crate) fn find_path(
        &mut self,
        map: &dyn MapWalkability,
        start: (u16, u16),
        dest: (u16, u16),
        max_steps: usize,
    ) -> Option<&[(u16, u16)]> {
        self.out.clear();
        let (width, height) = (map.width(), map.height());
        if start.0 >= width || start.1 >= height {
            return None;
        }
        let pass = Passable {
            map,
            limit_x: i32::from(width) - 1,
            limit_y: i32::from(height) - 1,
        };
        let max_steps = max_steps.min(MAX_WALKPATH);
        let (sx, sy) = (i32::from(start.0), i32::from(start.1));
        let (dx, dy) = (i32::from(dest.0), i32::from(dest.1));
        if !pass.at(dx, dy) {
            return None;
        }
        if start == dest {
            return Some(&self.out);
        }
        // Each step moves at most one cell, so a farther destination can't be reached in time.
        let reach = max_steps as i32;
        if (dx - sx).abs() > reach || (dy - sy).abs() > reach {
            return None;
        }

        let to_window = |x: i32, y: i32| -> Option<usize> {
            let (wx, wy) = (x - sx + RADIUS, y - sy + RADIUS);
            ((0..SIDE).contains(&wx) && (0..SIDE).contains(&wy)).then(|| (wy * SIDE + wx) as usize)
        };
        let from_window = |idx: usize| -> (i32, i32) {
            let idx = idx as i32;
            (idx % SIDE - RADIUS + sx, idx / SIDE - RADIUS + sy)
        };
        let heuristic =
            |x: i32, y: i32| MOVE_COST * ((x - dx).unsigned_abs() + (y - dy).unsigned_abs());

        self.next_stamp();
        self.heap.clear();
        let stamp = self.stamp;
        let start_idx = to_window(sx, sy)?;
        let dest_idx = to_window(dx, dy)?;
        let mut order: u32 = 0;
        self.g_cost[start_idx] = 0;
        self.parent[start_idx] = NO_PARENT;
        self.open_stamp[start_idx] = stamp;
        self.heap
            .push(Reverse((heuristic(sx, sy), order, start_idx as u16)));

        let mut found = false;
        while let Some(Reverse((f, _, idx))) = self.heap.pop() {
            let idx = usize::from(idx);
            let (x, y) = from_window(idx);
            let g = self.g_cost[idx];
            // Skip closed nodes and entries superseded by a cheaper push.
            if self.closed_stamp[idx] == stamp || f != g + heuristic(x, y) {
                continue;
            }
            self.closed_stamp[idx] = stamp;
            if idx == dest_idx {
                found = true;
                break;
            }
            let north = pass.at(x, y + 1);
            let south = pass.at(x, y - 1);
            let east = pass.at(x + 1, y);
            let west = pass.at(x - 1, y);
            for (dir, &(ox, oy)) in NEIGHBOURS.iter().enumerate() {
                let diagonal = ox != 0 && oy != 0;
                let allowed = match (ox, oy) {
                    (0, 1) => north,
                    (0, -1) => south,
                    (1, 0) => east,
                    (-1, 0) => west,
                    (1, 1) => north && east && pass.at(x + 1, y + 1),
                    (1, -1) => south && east && pass.at(x + 1, y - 1),
                    (-1, 1) => north && west && pass.at(x - 1, y + 1),
                    _ => south && west && pass.at(x - 1, y - 1),
                };
                if !allowed {
                    continue;
                }
                let (nx, ny) = (x + ox, y + oy);
                let Some(nidx) = to_window(nx, ny) else {
                    continue;
                };
                let ng = g + if diagonal {
                    MOVE_DIAGONAL_COST
                } else {
                    MOVE_COST
                };
                if self.open_stamp[nidx] == stamp && self.g_cost[nidx] <= ng {
                    continue;
                }
                // New node, or a cheaper route: (re)open it, even if it was closed.
                self.open_stamp[nidx] = stamp;
                self.closed_stamp[nidx] = 0;
                self.g_cost[nidx] = ng;
                self.parent[nidx] = dir as u8;
                order = order.wrapping_add(1);
                self.heap
                    .push(Reverse((ng + heuristic(nx, ny), order, nidx as u16)));
            }
        }
        if !found {
            return None;
        }

        // Walk parents back from the destination; reject paths over `max_steps` steps.
        // g strictly decreases along parents, so this terminates.
        let mut idx = dest_idx;
        while idx != start_idx {
            if self.out.len() >= max_steps {
                self.out.clear();
                return None;
            }
            let (x, y) = from_window(idx);
            self.out.push((x as u16, y as u16));
            let (ox, oy) = NEIGHBOURS.get(usize::from(self.parent[idx]))?;
            idx = to_window(x - ox, y - oy)?;
        }
        self.out.reverse();
        Some(&self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_db::MemoryMapGrid;
    use ro_db::{CellType, MapWalkability};

    fn grid(w: u16, h: u16, walls: &[(u16, u16)]) -> MemoryMapGrid {
        let mut g = MemoryMapGrid::all_walkable(w, h).expect("valid dims");
        for &(x, y) in walls {
            assert!(g.set(x, y, CellType::Wall), "wall in bounds");
        }
        g
    }

    fn find(g: &dyn MapWalkability, s: (u16, u16), d: (u16, u16)) -> Option<Vec<(u16, u16)>> {
        let mut scratch = AStarScratchBuffer::new();
        scratch
            .find_path(g, s, d, MAX_WALKPATH)
            .map(<[(u16, u16)]>::to_vec)
    }

    #[test]
    fn eng_05_wall_destination_rejected() {
        let g = grid(30, 30, &[(15, 10), (14, 10)]);
        assert_eq!(find(&g, (10, 10), (15, 10)), None);
        assert_eq!(find(&g, (14, 9), (15, 10)), None);
    }

    #[test]
    fn eng_05_no_corner_cutting() {
        // (14,9) -> (15,10) diagonal needs (15,9) and (14,10); (14,10) is a wall.
        let g = grid(30, 30, &[(14, 10)]);
        let path = find(&g, (14, 9), (15, 10)).expect("path around the corner");
        assert_eq!(path, vec![(15, 9), (15, 10)]);
    }

    #[test]
    fn eng_05_diagonal_used_when_corners_clear() {
        let g = grid(30, 30, &[]);
        assert_eq!(find(&g, (10, 10), (12, 12)), Some(vec![(11, 11), (12, 12)]));
    }

    #[test]
    fn eng_05_straight_path_waypoints() {
        let g = grid(30, 30, &[(15, 10), (14, 10)]);
        assert_eq!(
            find(&g, (10, 10), (13, 10)),
            Some(vec![(11, 10), (12, 10), (13, 10)])
        );
    }

    #[test]
    fn eng_05_more_than_32_steps_rejected() {
        let g = grid(60, 60, &[]);
        assert_eq!(find(&g, (1, 1), (33, 1)).map(|p| p.len()), Some(32));
        assert_eq!(find(&g, (1, 1), (34, 1)), None);
    }

    #[test]
    fn eng_05_detour_longer_than_32_rejected() {
        // A wall across the map forces a long way round.
        let walls: Vec<(u16, u16)> = (0..40).map(|x| (x, 10)).collect();
        let g = grid(60, 60, &walls);
        assert_eq!(find(&g, (2, 9), (2, 11)), None);
    }

    #[test]
    fn eng_05_reopens_closed_nodes_like_rathena() {
        // rAthena's add_path re-pushes a closed node when a cheaper g is found; with the
        // inconsistent Manhattan heuristic that changes the result here (6 steps, not 7).
        let g = grid(12, 12, &[(1, 6), (0, 8)]);
        assert_eq!(
            find(&g, (1, 5), (0, 9)),
            Some(vec![(2, 5), (2, 6), (2, 7), (1, 8), (1, 9), (0, 9)])
        );
    }

    #[test]
    fn eng_05_reconstructed_path_over_32_steps_rejected() {
        // The detour stays inside the search window but is 44 steps long.
        let walls: Vec<(u16, u16)> = (20..=60).map(|x| (x, 41)).collect();
        let g = grid(100, 100, &walls);
        assert_eq!(find(&g, (40, 40), (40, 42)), None);
    }

    #[test]
    fn eng_05_no_corner_cutting_any_direction() {
        // Around (10,10): each diagonal is blocked by one wall on a shared cardinal cell.
        for (corner, dest) in [
            ((10, 11), (11, 11)),
            ((11, 10), (11, 9)),
            ((9, 10), (9, 11)),
            ((10, 9), (9, 9)),
        ] {
            let g = grid(30, 30, &[corner]);
            let path = find(&g, (10, 10), dest).expect("path around the corner");
            assert_eq!(path.len(), 2, "{corner:?} -> {dest:?}: {path:?}");
        }
    }

    #[test]
    fn eng_05_current_cell_is_empty_path() {
        let g = grid(30, 30, &[]);
        assert_eq!(find(&g, (5, 5), (5, 5)), Some(vec![]));
    }

    #[test]
    fn eng_05_enclosed_start_rejected() {
        let ring = [
            (4, 4),
            (5, 4),
            (6, 4),
            (4, 5),
            (6, 5),
            (4, 6),
            (5, 6),
            (6, 6),
        ];
        let g = grid(30, 30, &ring);
        assert_eq!(find(&g, (5, 5), (10, 10)), None);
    }

    #[test]
    fn eng_05_last_row_and_column_unwalkable() {
        let g = grid(30, 30, &[]);
        assert_eq!(find(&g, (25, 5), (29, 5)), None);
        assert_eq!(find(&g, (5, 25), (5, 29)), None);
        assert!(find(&g, (25, 5), (28, 5)).is_some());
        // A detour may not pass through column 29 either.
        let walls: Vec<(u16, u16)> = (20..=28).map(|x| (x, 6)).collect();
        let g = grid(30, 30, &walls);
        let path = find(&g, (28, 5), (28, 7)).expect("detour on the left");
        assert!(path.iter().all(|&(x, _)| x < 29), "{path:?}");
    }

    #[test]
    fn eng_05_out_of_bounds_destination_rejected() {
        let g = grid(30, 30, &[]);
        assert_eq!(find(&g, (5, 5), (250, 5)), None);
        assert_eq!(find(&g, (5, 5), (u16::MAX, u16::MAX)), None);
    }

    /// Reports a 65535x65535 all-walkable map without allocating it.
    struct HugeMap;
    impl MapWalkability for HugeMap {
        fn width(&self) -> u16 {
            u16::MAX
        }
        fn height(&self) -> u16 {
            u16::MAX
        }
        fn is_walkable(&self, x: u16, y: u16) -> bool {
            x < u16::MAX && y < u16::MAX
        }
        fn cell_type(&self, x: u16, y: u16) -> Option<CellType> {
            self.is_walkable(x, y).then_some(CellType::Walkable)
        }
    }

    #[test]
    fn eng_05_map_edge_arithmetic_does_not_overflow() {
        let mut scratch = AStarScratchBuffer::new();
        let _ = scratch.find_path(&HugeMap, (65534, 65534), (65530, 65530), MAX_WALKPATH);
        let _ = scratch.find_path(&HugeMap, (65533, 65533), (65534, 65534), MAX_WALKPATH);
        assert!(scratch
            .find_path(&HugeMap, (65500, 65500), (65510, 65510), MAX_WALKPATH)
            .is_some());
        let _ = scratch.find_path(&HugeMap, (0, 0), (3, 3), MAX_WALKPATH);
    }

    #[test]
    fn eng_05_scratch_is_reused_without_growing() {
        let g = grid(100, 100, &[(50, 49), (50, 50), (50, 51)]);
        let mut scratch = AStarScratchBuffer::new();
        scratch
            .find_path(&g, (40, 50), (60, 50), MAX_WALKPATH)
            .expect("warm-up path");
        let warm = scratch.heap_capacity();
        for i in 0..200u16 {
            let dest = (45 + (i % 20), 40 + (i % 17));
            let _ = scratch.find_path(&g, (40, 50), dest, MAX_WALKPATH);
        }
        assert_eq!(
            scratch.heap_capacity(),
            warm,
            "no heap growth after warm-up"
        );
    }

    #[test]
    fn eng_05_repeated_searches_are_independent() {
        let g = grid(30, 30, &[(14, 10)]);
        let mut scratch = AStarScratchBuffer::new();
        assert!(scratch
            .find_path(&g, (10, 10), (20, 20), MAX_WALKPATH)
            .is_some());
        assert_eq!(scratch.find_path(&g, (14, 9), (14, 10), MAX_WALKPATH), None);
        assert_eq!(
            scratch
                .find_path(&g, (10, 10), (13, 10), MAX_WALKPATH)
                .map(<[_]>::to_vec),
            Some(vec![(11, 10), (12, 10), (13, 10)])
        );
    }
}
