//! Free-cell search for mob placement.

use ro_db::MapGrid;

use crate::ecs::SplitMix64;
use crate::protocol_consts::EDGE_GAP;

/// rAthena `map_edge_size`: a map-wide search stays this far from the edges.
const MAP_EDGE: i32 = 15;

/// rAthena `map_search_freecell` without player avoidance: random cells within `radius` of
/// `base` (the whole map inside `MAP_EDGE` when there is no base or a radius is negative). A
/// try fails on the base cell itself, near an edge, or on an unwalkable cell.
pub(super) fn search(
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
pub(super) fn walkable_cell(grid: &MapGrid, (x, y): (i32, i32)) -> Option<(u16, u16)> {
    let cell = (u16::try_from(x).ok()?, u16::try_from(y).ok()?);
    grid.is_walkable(cell.0, cell.1).then_some(cell)
}
