use ro_db::{MapGrid, MobId, MobSpawn};

use super::*;
use crate::ecs::SplitMix64;

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
