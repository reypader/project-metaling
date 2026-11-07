//! Test fixtures shared by the movement tests.

use ro_db::{CellType, MapGrid};

pub(super) fn grid(w: u16, h: u16, walls: &[(u16, u16)]) -> MapGrid {
    let mut g = MapGrid::all_walkable(w, h).expect("valid dims");
    for &(x, y) in walls {
        assert!(g.set(x, y, CellType::Wall), "wall in bounds");
    }
    g
}
