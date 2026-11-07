//! 8x8-cell block grid for Area-of-Interest queries.

use ro_core::MapPosition;
use smallvec::SmallVec;

use crate::ecs::Entity;

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
mod tests;
