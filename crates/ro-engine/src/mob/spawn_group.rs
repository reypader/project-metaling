//! One normalized mob spawn line and where its mobs may appear.

use std::sync::Arc;

use ro_db::{MapGrid, MobSpawn};

use crate::ecs::SplitMix64;
use crate::mob::placement::{search, walkable_cell};

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
    pub(crate) center: Option<(i32, i32)>,
    /// `(xs, ys)` after the rectangle rules; `(0, 0)` when map-wide.
    pub(crate) span: (i32, i32),
}

impl SpawnGroup {
    /// ENG-MOB-02 steps 1-2. `line` must lie inside `grid`.
    pub(crate) fn new(line: &MobSpawn, grid: &MapGrid) -> Self {
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
    pub(crate) fn mob_center(&self, rng: &mut SplitMix64, grid: &MapGrid) -> Option<(i32, i32)> {
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
    pub(crate) fn place(
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
