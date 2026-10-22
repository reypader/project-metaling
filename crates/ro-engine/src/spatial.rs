//! 8x8-cell block grid for Area-of-Interest queries.

use ro_contracts::MapPosition;
use smallvec::SmallVec;

use crate::entity::Entity;

/// View range: Chebyshev distance (rAthena `AREA_SIZE`, square area).
pub(crate) const AREA_SIZE: u16 = 14;
/// Cells per block side (rAthena `BLOCK_SIZE`).
pub(crate) const BLOCK_SIZE: u16 = 8;

/// Units bucketed by 8x8 block. Positions stay in the world's dense column; queries take
/// that column to filter candidates by exact distance.
#[derive(Debug)]
pub(crate) struct SpatialGrid8x8 {
    blocks_w: usize,
    blocks_h: usize,
    blocks: Vec<SmallVec<[Entity; 8]>>,
}

impl SpatialGrid8x8 {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        let blocks_w = usize::from(width).div_ceil(usize::from(BLOCK_SIZE)).max(1);
        let blocks_h = usize::from(height).div_ceil(usize::from(BLOCK_SIZE)).max(1);
        Self {
            blocks_w,
            blocks_h,
            blocks: vec![SmallVec::new(); blocks_w * blocks_h],
        }
    }

    pub(crate) const fn block_of(x: u16, y: u16) -> (u16, u16) {
        (x / BLOCK_SIZE, y / BLOCK_SIZE)
    }

    fn block_index(&self, x: u16, y: u16) -> Option<usize> {
        let (bx, by) = Self::block_of(x, y);
        let (bx, by) = (usize::from(bx), usize::from(by));
        (bx < self.blocks_w && by < self.blocks_h).then_some(by * self.blocks_w + bx)
    }

    /// Adds `entity` at (x, y). Out-of-map cells are ignored (the caller validates positions).
    pub(crate) fn insert(&mut self, entity: Entity, x: u16, y: u16) {
        if let Some(block) = self
            .block_index(x, y)
            .and_then(|idx| self.blocks.get_mut(idx))
        {
            block.push(entity);
        }
    }

    /// Removes `entity` from the block holding (x, y). Returns whether it was there.
    pub(crate) fn remove(&mut self, entity: Entity, x: u16, y: u16) -> bool {
        let Some(block) = self
            .block_index(x, y)
            .and_then(|idx| self.blocks.get_mut(idx))
        else {
            return false;
        };
        match block.iter().position(|&e| e == entity) {
            Some(i) => {
                block.swap_remove(i);
                true
            }
            None => false,
        }
    }

    /// Re-links `entity` only when its block changes. Returns whether it did. An entity that
    /// isn't in the `from` block is left out of the grid (no ghost entries).
    pub(crate) fn move_entity(&mut self, entity: Entity, from: (u16, u16), to: (u16, u16)) -> bool {
        if self.block_index(from.0, from.1) == self.block_index(to.0, to.1) {
            return false;
        }
        if !self.remove(entity, from.0, from.1) {
            return false;
        }
        self.insert(entity, to.0, to.1);
        true
    }

    /// Clears `out` and fills it with every entity within Chebyshev `radius` of (cx, cy).
    pub(crate) fn query_radius(
        &self,
        cx: u16,
        cy: u16,
        radius: u16,
        positions: &[MapPosition],
        out: &mut Vec<Entity>,
    ) {
        out.clear();
        self.visit_radius(cx, cy, radius, positions, |entity| out.push(entity));
    }

    /// Calls `visit` for every entity within Chebyshev `radius` of (cx, cy).
    pub(crate) fn visit_radius(
        &self,
        cx: u16,
        cy: u16,
        radius: u16,
        positions: &[MapPosition],
        mut visit: impl FnMut(Entity),
    ) {
        let bx0 = usize::from(cx.saturating_sub(radius) / BLOCK_SIZE);
        let by0 = usize::from(cy.saturating_sub(radius) / BLOCK_SIZE);
        let bx1 = usize::from(cx.saturating_add(radius) / BLOCK_SIZE).min(self.blocks_w - 1);
        let by1 = usize::from(cy.saturating_add(radius) / BLOCK_SIZE).min(self.blocks_h - 1);
        if bx0 > bx1 || by0 > by1 {
            return;
        }
        for by in by0..=by1 {
            let row = by * self.blocks_w;
            for block in self.blocks.get(row + bx0..=row + bx1).into_iter().flatten() {
                for &entity in block {
                    let Some(pos) = positions.get(entity.index()) else {
                        continue;
                    };
                    if pos.x.abs_diff(cx) <= radius && pos.y.abs_diff(cy) <= radius {
                        visit(entity);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn block_len(&self, bx: usize, by: usize) -> usize {
        self.blocks[by * self.blocks_w + bx].len()
    }
}

/// Visibility changes for one step of `mover` from `old` to `new` (at most one cell).
///
/// `candidates` must hold every unit within `AREA_SIZE + 1` of `old`, which covers both the
/// old and the new view. Fills `left` (visible before, not after) and `entered` (the reverse).
pub(crate) fn step_visibility(
    mover: Entity,
    old: (u16, u16),
    new: (u16, u16),
    candidates: &[Entity],
    positions: &[MapPosition],
    left: &mut Vec<Entity>,
    entered: &mut Vec<Entity>,
) {
    left.clear();
    entered.clear();
    let in_view = |center: (u16, u16), p: &MapPosition| {
        p.x.abs_diff(center.0) <= AREA_SIZE && p.y.abs_diff(center.1) <= AREA_SIZE
    };
    for &other in candidates {
        if other == mover {
            continue;
        }
        let Some(p) = positions.get(other.index()) else {
            continue;
        };
        match (in_view(old, p), in_view(new, p)) {
            (true, false) => left.push(other),
            (false, true) => entered.push(other),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Entity, EntityAllocator};
    use ro_contracts::MapPosition;

    struct Fixture {
        grid: SpatialGrid8x8,
        positions: Vec<MapPosition>,
        alloc: EntityAllocator,
    }

    impl Fixture {
        fn new(w: u16, h: u16) -> Self {
            Self {
                grid: SpatialGrid8x8::new(w, h),
                positions: Vec::new(),
                alloc: EntityAllocator::with_capacity(64),
            }
        }
        fn add(&mut self, x: u16, y: u16) -> Entity {
            let e = self.alloc.allocate().expect("capacity");
            if self.positions.len() <= e.index() {
                self.positions
                    .resize(e.index() + 1, MapPosition { x: 0, y: 0, dir: 0 });
            }
            self.positions[e.index()] = MapPosition { x, y, dir: 0 };
            self.grid.insert(e, x, y);
            e
        }
        fn query(&self, x: u16, y: u16) -> Vec<Entity> {
            let mut out = Vec::new();
            self.grid
                .query_radius(x, y, AREA_SIZE, &self.positions, &mut out);
            out.sort_by_key(|e| e.index());
            out
        }
        /// Steps `e` to (x, y): moves it in the grid and returns whether its block changed.
        fn step(&mut self, e: Entity, x: u16, y: u16) -> bool {
            let old = self.positions[e.index()];
            let changed = self.grid.move_entity(e, (old.x, old.y), (x, y));
            self.positions[e.index()] = MapPosition { x, y, dir: 0 };
            changed
        }
    }

    #[test]
    fn eng_03_query_radius_is_chebyshev_14() {
        let mut f = Fixture::new(200, 200);
        let a = f.add(100, 100);
        let b = f.add(103, 100);
        let c = f.add(114, 114);
        let _d = f.add(115, 100);
        let _e = f.add(150, 150);
        assert_eq!(f.query(100, 100), vec![a, b, c]);
    }

    #[test]
    fn eng_03_query_near_origin_and_far_edge_clamps() {
        let mut f = Fixture::new(200, 200);
        let a = f.add(0, 0);
        let b = f.add(199, 199);
        assert_eq!(f.query(0, 0), vec![a]);
        assert_eq!(f.query(199, 199), vec![b]);
        assert_eq!(f.query(u16::MAX, u16::MAX), Vec::<Entity>::new());
    }

    #[test]
    fn eng_04_block_changes_only_across_8x8_boundaries() {
        let mut f = Fixture::new(200, 200);
        let a = f.add(103, 100);
        assert_eq!(SpatialGrid8x8::block_of(103, 100), (12, 12));
        assert_eq!(f.grid.block_len(12, 12), 1);
        assert!(f.step(a, 104, 100), "103 -> 104 crosses into block 13");
        assert_eq!(SpatialGrid8x8::block_of(104, 100), (13, 12));
        assert_eq!(f.grid.block_len(12, 12), 0);
        assert_eq!(f.grid.block_len(13, 12), 1);
        assert!(!f.step(a, 105, 100), "104 -> 105 stays in block 13");
        assert_eq!(f.grid.block_len(13, 12), 1);
    }

    #[test]
    fn eng_04_visibility_diff_along_a_walk() {
        let mut f = Fixture::new(200, 200);
        let a = f.add(103, 100);
        let d = f.add(120, 100);
        let o = f.add(100, 100);
        let mut candidates = Vec::new();
        let mut left = Vec::new();
        let mut entered = Vec::new();
        let mut log = Vec::new();
        for x in 104..=117u16 {
            let old = f.positions[a.index()];
            f.grid
                .query_radius(old.x, old.y, AREA_SIZE + 1, &f.positions, &mut candidates);
            f.step(a, x, 100);
            step_visibility(
                a,
                (old.x, old.y),
                (x, 100),
                &candidates,
                &f.positions,
                &mut left,
                &mut entered,
            );
            for &e in &entered {
                log.push((x, e, "entered"));
            }
            for &e in &left {
                log.push((x, e, "left"));
            }
        }
        assert_eq!(log, vec![(106, d, "entered"), (115, o, "left")]);
    }

    #[test]
    fn move_of_entity_not_in_grid_does_not_insert_it() {
        let mut f = Fixture::new(50, 50);
        let ghost = f.alloc.allocate().expect("capacity");
        f.positions
            .resize(ghost.index() + 1, MapPosition { x: 7, y: 7, dir: 0 });
        assert!(!f.grid.move_entity(ghost, (7, 7), (8, 7)));
        f.positions[ghost.index()] = MapPosition { x: 8, y: 7, dir: 0 };
        assert!(
            f.query(8, 7).is_empty(),
            "never-inserted entity must not appear"
        );
    }

    #[test]
    fn remove_takes_entity_out_of_queries() {
        let mut f = Fixture::new(50, 50);
        let a = f.add(10, 10);
        assert!(f.grid.remove(a, 10, 10));
        assert!(!f.grid.remove(a, 10, 10));
        assert!(f.query(10, 10).is_empty());
    }
}
