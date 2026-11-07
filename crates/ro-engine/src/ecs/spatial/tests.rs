use ro_core::MapPosition;

use super::*;
use crate::ecs::{Entity, EntityAllocator};

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
