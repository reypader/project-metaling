//! Line of sight and attack range checks over the walkability grid.

use ro_db::{CellType, MapGrid};
use ro_mechanics::check_distance_client;

use crate::ecs::AREA_SIZE;

/// rAthena `CELL_CHKWALL` for line of sight: a cell that is neither walkable nor shootable
/// (raw 1 and unknown types; a cliff is shootable). The last row and column and cells outside
/// the map are never walls.
fn is_los_wall(grid: &MapGrid, x: i32, y: i32) -> bool {
    let (w, h) = (i32::from(grid.width()), i32::from(grid.height()));
    if x < 0 || y < 0 || x >= w - 1 || y >= h - 1 {
        return false;
    }
    // In bounds, so the casts are exact.
    match grid.cell_type(x as u16, y as u16) {
        Some(CellType::Wall | CellType::Unknown(_)) => true,
        Some(_) | None => false,
    }
}

/// rAthena `path_search_long` with `CELL_CHKWALL` (catalog `movement_aoi.md` §5): steps from the
/// left endpoint towards the right one and fails on a wall strictly between the two cells.
pub(crate) fn line_of_sight(grid: &MapGrid, from: (u16, u16), to: (u16, u16)) -> bool {
    let (mut x0, mut y0) = (i32::from(from.0), i32::from(from.1));
    let (mut x1, mut y1) = (i32::from(to.0), i32::from(to.1));
    if x1 < x0 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let weight = if dx > dy.abs() { dx } else { dy.abs() };
    let (mut wx, mut wy) = (0, 0);
    while (x0, y0) != (x1, y1) {
        wx += dx;
        wy += dy;
        if wx >= weight {
            wx -= weight;
            x0 += 1;
        }
        if wy >= weight {
            wy -= weight;
            y0 += 1;
        } else if wy < 0 {
            wy += weight;
            y0 -= 1;
        }
        if (x0, y0) != (x1, y1) && is_los_wall(grid, x0, y0) {
            return false;
        }
    }
    true
}

/// rAthena `battle_check_range` for a player attacker on one map: the client distance, then
/// adjacent cells pass without a wall test, more than `AREA_SIZE` cells never pass, and anything
/// between needs line of sight.
pub(crate) fn battle_check_range(
    grid: &MapGrid,
    from: (u16, u16),
    to: (u16, u16),
    range: u16,
) -> bool {
    let dx = i32::from(to.0) - i32::from(from.0);
    let dy = i32::from(to.1) - i32::from(from.1);
    if !check_distance_client(dx, dy, range) {
        return false;
    }
    match dx.abs().max(dy.abs()) {
        d if d < 2 => true,
        d if d > i32::from(AREA_SIZE) => false,
        _ => line_of_sight(grid, from, to),
    }
}

#[cfg(test)]
mod tests;
