use ro_db::CellType;

use super::*;
use crate::movement::test_support::grid;

/// MECH-CMB-05: `path_search_long` with `CELL_CHKWALL` (catalog `movement_aoi.md` §5).
#[test]
fn mech_cmb_05_line_of_sight() {
    // A wall between blocks; a cliff doesn't.
    let mut g = grid(30, 30, &[(12, 10)]);
    assert!(!line_of_sight(&g, (10, 10), (15, 10)));
    assert!(!line_of_sight(&g, (15, 10), (10, 10)));
    assert!(g.set(12, 10, CellType::Cliff));
    assert!(line_of_sight(&g, (10, 10), (15, 10)));
    // An unknown raw type is a wall.
    assert!(g.set(12, 10, CellType::Unknown(9)));
    assert!(!line_of_sight(&g, (10, 10), (15, 10)));

    // End cells are never tested.
    let g = grid(30, 30, &[(10, 10), (15, 10)]);
    assert!(line_of_sight(&g, (10, 10), (15, 10)));

    // Asymmetric rounding, dx = 4: dy = +1 steps y on the last step (tested cells on the
    // source row); dy = -1 steps y on the first (tested cells on the destination row).
    let g = grid(30, 30, &[(12, 11)]);
    assert!(line_of_sight(&g, (10, 10), (14, 11)), "(12,11) not visited");
    let g = grid(30, 30, &[(12, 10)]);
    assert!(!line_of_sight(&g, (10, 10), (14, 11)), "(12,10) visited");
    let g = grid(30, 30, &[(12, 9)]);
    assert!(!line_of_sight(&g, (10, 10), (14, 9)), "(12,9) visited");
    let g = grid(30, 30, &[(12, 10)]);
    assert!(line_of_sight(&g, (10, 10), (14, 9)), "(12,10) not visited");
    // The scan always runs from the left endpoint, so swapping the ends of the dy = -1 line
    // gives the same answer as the original.
    let g = grid(30, 30, &[(12, 9)]);
    assert!(!line_of_sight(&g, (14, 9), (10, 10)));

    // Vertical line, and the map's last row/column and outside cells are not walls.
    let g = grid(30, 30, &[(5, 12)]);
    assert!(!line_of_sight(&g, (5, 10), (5, 14)));
    let g = grid(10, 10, &[(9, 5), (5, 9)]);
    assert!(line_of_sight(&g, (9, 2), (9, 8)));
    assert!(line_of_sight(&g, (2, 9), (8, 9)));
}

/// MECH-CMB-05: `battle_check_range` for a player: client distance, adjacent without a wall
/// test, beyond 14 never, else line of sight.
#[test]
fn mech_cmb_05_battle_check_range() {
    let open = grid(60, 60, &[]);
    assert!(battle_check_range(&open, (20, 20), (21, 20), 1));
    assert!(battle_check_range(&open, (20, 20), (21, 21), 1));
    assert!(
        battle_check_range(&open, (20, 20), (22, 20), 1),
        "straight +1"
    );
    assert!(!battle_check_range(&open, (20, 20), (22, 21), 1));
    assert!(battle_check_range(&open, (20, 20), (20, 20), 0));
    let g = grid(60, 60, &[(21, 20)]);
    // Adjacent cells skip the wall test (here the wall is the target's neighbour).
    assert!(battle_check_range(&g, (20, 20), (21, 21), 1));
    assert!(
        !battle_check_range(&g, (20, 20), (22, 20), 1),
        "wall between"
    );
    assert!(
        !battle_check_range(&g, (20, 20), (25, 20), 5),
        "wall blocks"
    );
    assert!(battle_check_range(&g, (20, 21), (25, 21), 5));
    assert!(!battle_check_range(&g, (20, 30), (35, 30), 20), "beyond 14");
    assert!(battle_check_range(&g, (20, 30), (34, 30), 20));
}
