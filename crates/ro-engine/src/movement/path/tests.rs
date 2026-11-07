use super::*;
use crate::movement::test_support::grid;

fn find(g: &MapGrid, s: (u16, u16), d: (u16, u16)) -> Option<Vec<(u16, u16)>> {
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

#[test]
fn eng_05_map_edge_arithmetic_does_not_overflow() {
    // x and y arithmetic are independent, so two thin max-size grids cover what a
    // 65535x65535 grid would (4 GiB, too big to allocate).
    let wide = grid(u16::MAX, 70, &[]);
    let tall = grid(70, u16::MAX, &[]);
    let mut scratch = AStarScratchBuffer::new();
    for (g, flip) in [(&wide, false), (&tall, true)] {
        let p = |a: u16, b: u16| if flip { (b, a) } else { (a, b) };
        let _ = scratch.find_path(g, p(65534, 65), p(65530, 61), MAX_WALKPATH);
        let _ = scratch.find_path(g, p(65533, 65), p(65534, 66), MAX_WALKPATH);
        assert!(scratch
            .find_path(g, p(65500, 30), p(65510, 40), MAX_WALKPATH)
            .is_some());
        let _ = scratch.find_path(g, p(0, 0), p(3, 3), MAX_WALKPATH);
    }
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
