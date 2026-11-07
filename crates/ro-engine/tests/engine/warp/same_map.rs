//! Warp areas, same-map warps, hidden warps and destination cells (ENG-WARP-01, 02, 04..07).

use super::*;

// ---------- ENG-WARP-01: trigger index ----------

#[test]
fn eng_warp_01_area_marks_walkable_cells_only() {
    let wall = (51, 50);
    let mut engine = instance(
        grid(100, 100, &[wall]),
        vec![warp("w", (50, 50), (2, 1), (90, 90))],
    );
    for y in 46..=54 {
        for x in 45..=55 {
            let inside = (48..=52).contains(&x) && (49..=51).contains(&y);
            let expect = inside && (x, y) != wall;
            assert_eq!(
                arrival_fires(&mut engine, (x, y)),
                expect,
                "cell ({x}, {y})"
            );
        }
    }
}

#[test]
fn eng_warp_01_area_is_clipped_to_the_map() {
    let mut engine = instance(
        grid(20, 20, &[]),
        vec![
            warp("low", (1, 1), (3, 3), (10, 10)),
            warp("high", (19, 19), (2, 2), (10, 10)),
        ],
    );
    assert!(arrival_fires(&mut engine, (0, 0)));
    assert!(arrival_fires(&mut engine, (4, 4)));
    assert!(!arrival_fires(&mut engine, (5, 4)));
    assert!(arrival_fires(&mut engine, (19, 19)));
    assert!(arrival_fires(&mut engine, (17, 17)));
    assert!(!arrival_fires(&mut engine, (16, 19)));
}

#[test]
fn eng_warp_01_warp_without_trigger_marks_nothing() {
    let mut spanless = warp("w", (50, 50), (1, 1), (90, 90));
    spanless.trigger = None;
    let mut engine = instance(grid(100, 100, &[]), vec![spanless]);
    assert!(!arrival_fires(&mut engine, (50, 50)));
}

#[test]
fn eng_warp_01_overlap_earlier_spawn_wins() {
    let first = warp("first", (50, 50), (1, 1), (10, 10));
    let second = warp("second", (51, 50), (1, 1), (90, 90));
    let mut engine = instance(grid(100, 100, &[]), vec![first.clone(), second.clone()]);
    let events = enter(&mut engine, A, (51, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 10, 10)]);

    let mut engine = instance(grid(100, 100, &[]), vec![second, first]);
    let events = enter(&mut engine, A, (51, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 90, 90)]);
}

// ---------- ENG-WARP-02: same-map warp, mid-path ----------

#[test]
fn eng_warp_02_same_map_warp_mid_path() {
    // Area x 59..=61, y 49..=51; the walk east along y=50 (16 cells, under the click limit)
    // enters it at x=59.
    let mut engine = instance(
        grid(200, 200, &[]),
        vec![warp("w", (60, 50), (1, 1), (100, 100))],
    );
    enter(&mut engine, B, (55, 52), 0); // in view of the source only
    enter(&mut engine, C, (100, 105), 0); // in view of the destination only
    enter(&mut engine, A, (50, 50), 0);
    let gid_a = account(A);

    // Nine 150 ms steps reach x=59 at t=1350.
    let mut events = walk(&mut engine, A, (66, 50), 0, 1300);
    assert!(map_moves(&events, A).is_empty());
    assert_eq!(pos(&engine, A), (58, 50));
    let fired = tick(&mut engine, 1350);
    assert_eq!(map_moves(&fired, A), [(MAP.to_string(), 100, 100)]);
    events.extend(fired);
    for now in (1400..=2000).step_by(50) {
        events.extend(tick(&mut engine, now));
    }
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 100, 100)]);
    assert_eq!(vanished(&to(&events, B), gid_a), [0], "observer B");
    assert!(
        vanished(&to(&events, A), gid_a).is_empty(),
        "self gets no vanish"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.event, ServerEvent::UnitStopped { .. })),
        "no stop on a warp"
    );
    assert!(
        !mentions(&to(&events, C), gid_a),
        "destination sees nothing yet"
    );
    assert_eq!(pos(&engine, A), (100, 100));
    assert!(!engine.is_walking(char_id(A)));

    // Out of the grid: moves are ignored until ActorInit.
    let events = walk(&mut engine, A, (105, 100), 2050, 2500);
    assert!(events.is_empty(), "move while loading: {events:?}");
    assert_eq!(pos(&engine, A), (100, 100));

    let events = actor_init(&mut engine, A, 2550);
    let c_sees: Vec<_> = to(&events, C)
        .into_iter()
        .filter(|e| matches!(e, ServerEvent::UnitSpawned { unit } if unit.gid == gid_a))
        .collect();
    assert_eq!(c_sees.len(), 1, "destination observer gets the spawn");
    assert!(
        to(&events, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::UnitStanding { unit } if unit.gid == account(C))),
        "self sees units at the destination"
    );
    assert!(
        !mentions(&to(&events, B), gid_a),
        "source observer is out of view"
    );

    let events = walk(&mut engine, A, (103, 100), 2600, 2600);
    assert!(
        to(&events, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::PlayerMoved { .. })),
        "moves work again"
    );

    assert!(actor_init(&mut engine, A, 2650)
        .iter()
        .all(|e| !matches!(e.event, ServerEvent::UnitSpawned { .. })));
}

#[test]
fn eng_warp_02_second_actor_init_only_resends_inventory() {
    let mut engine = instance(
        grid(100, 100, &[]),
        vec![warp("w", (50, 50), (1, 1), (80, 80))],
    );
    let events = enter(&mut engine, A, (50, 50), 0);
    assert_eq!(map_moves(&events, A).len(), 1);
    assert!(!actor_init(&mut engine, A, 100).is_empty());
    // A repeat only resends the inventory lists (specs/items ENG-INV-04): no warp, no spawn.
    let again = actor_init(&mut engine, A, 200);
    assert!(map_moves(&again, A).is_empty());
    assert_eq!(again.len(), 5, "{again:#?}");
    assert!(again.iter().all(|e| matches!(
        e.event,
        ServerEvent::InventoryStart
            | ServerEvent::InventoryEnd
            | ServerEvent::EquipSwitchList
            | ServerEvent::ParameterChanged { .. }
    )));
}

#[test]
fn eng_warp_04_disabled_and_cloaked_never_fire() {
    for state in [NpcState::Disabled, NpcState::Cloaked] {
        let mut engine = instance(grid(100, 100, &[]), vec![with_state(state)]);
        enter(&mut engine, B, (52, 52), 0);
        let mut events = enter(&mut engine, A, (50, 50), 0);
        events.extend(walk(&mut engine, A, (60, 50), 50, 2000));
        assert!(map_moves(&events, A).is_empty(), "{state:?}");
        assert!(
            vanished(&to(&events, B), account(A)).is_empty(),
            "{state:?}"
        );
    }
}

#[test]
fn eng_warp_04_hidden_warp_fires() {
    let mut engine = instance(grid(100, 100, &[]), vec![with_state(NpcState::Hidden)]);
    let events = walk_in_from(&mut engine, (45, 50));
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
}

// ---------- ENG-WARP-05: arrival-cell chain ----------

#[test]
fn eng_warp_05_login_on_warp_cell_spawns_then_vanishes() {
    let mut engine = instance(
        grid(100, 100, &[]),
        vec![warp("w", (50, 50), (1, 1), (80, 80))],
    );
    enter(&mut engine, B, (52, 52), 0);
    let events = enter(&mut engine, A, (50, 50), 0);
    let b_sees = to(&events, B);
    let spawned = b_sees
        .iter()
        .position(|e| matches!(e, ServerEvent::UnitSpawned { unit } if unit.gid == account(A)));
    let vanished_at = b_sees.iter().position(
        |e| matches!(e, ServerEvent::UnitVanished { gid, vanish_type: 0 } if *gid == account(A)),
    );
    assert!(spawned.is_some() && vanished_at.is_some(), "{b_sees:?}");
    assert!(spawned < vanished_at, "spawn first, then vanish");
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
    // The entry events for B came before the warp.
    let a_sees = to(&events, A);
    let saw_b = a_sees
        .iter()
        .position(|e| matches!(e, ServerEvent::UnitStanding { unit } if unit.gid == account(B)));
    let moved = a_sees
        .iter()
        .position(|e| matches!(e, ServerEvent::PlayerMapMove { .. }));
    assert!(saw_b < moved);
}

#[test]
fn eng_warp_05_chain_after_same_map_warp() {
    let mut engine = instance(
        grid(150, 150, &[]),
        vec![
            warp("one", (50, 50), (1, 1), (80, 80)),
            warp("two", (80, 80), (1, 1), (120, 120)),
        ],
    );
    let events = enter(&mut engine, A, (50, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
    let events = actor_init(&mut engine, A, 100);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 120, 120)]);
    let events = actor_init(&mut engine, A, 200);
    assert!(
        map_moves(&events, A).is_empty(),
        "arrival outside every area"
    );
    assert_eq!(pos(&engine, A), (120, 120));
}

// ---------- ENG-WARP-06: rewarp guard ----------

#[test]
fn eng_warp_06_eleven_chained_warps_then_skip() {
    let mut engine = instance(
        grid(150, 150, &[]),
        vec![
            warp("one", (50, 50), (1, 1), (80, 80)),
            warp("two", (80, 80), (1, 1), (50, 50)),
        ],
    );
    let mut warps = map_moves(&enter(&mut engine, A, (50, 50), 0), A).len();
    for k in 1..20 {
        warps += map_moves(&actor_init(&mut engine, A, k * 100), A).len();
    }
    assert_eq!(warps, 11);
    assert_eq!(pos(&engine, A), (80, 80), "stays at the 11th destination");

    // The skip reset the count: the next step inside the area warps again.
    let events = walk(&mut engine, A, (80, 83), 3000, 4000);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 50, 50)]);
}

// ---------- ENG-WARP-07: destination cell ----------

#[test]
fn eng_warp_07_in_bounds_wall_is_used_as_is() {
    let mut engine = instance(
        grid(100, 100, &[(80, 80)]),
        vec![warp("w", (50, 50), (1, 1), (80, 80))],
    );
    let events = enter(&mut engine, A, (50, 50), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 80, 80)]);
}

#[test]
fn eng_warp_07_zero_and_out_of_bounds_pick_a_random_free_cell() {
    for dest in [(0, 0), (500, 10), (10, 100)] {
        let (x, y) = random_landing(grid(100, 100, &[]), dest);
        assert!(
            (1..=98).contains(&x) && (1..=98).contains(&y),
            "{dest:?} -> ({x}, {y})"
        );
        assert!(
            x.abs_diff(50) > 2 || y.abs_diff(50) > 2,
            "not within 1 of a warp cell: ({x}, {y})"
        );
    }
}

#[test]
fn eng_warp_07_random_cell_skips_walls_and_cells_near_warps() {
    // 7x7: the random range is x, y in 1..=5. Only (1, 1) and (5, 5) are walkable there, and
    // (5, 5) touches the warp cell (5, 4).
    let mut walls = Vec::new();
    for y in 1..=5 {
        for x in 1..=5 {
            if ![(1, 1), (5, 5), (5, 4)].contains(&(x, y)) {
                walls.push((x, y));
            }
        }
    }
    let mut engine = instance(grid(7, 7, &walls), vec![warp("w", (5, 4), (0, 0), (0, 0))]);
    let events = enter(&mut engine, A, (5, 4), 0);
    assert_eq!(map_moves(&events, A), [(MAP.to_string(), 1, 1)]);
}

#[test]
fn eng_warp_07_no_free_cell_means_no_warp_and_no_events() {
    // 5x5: the random range 1..=3 is all inside the warp area.
    let mut engine = instance(grid(5, 5, &[]), vec![warp("w", (2, 2), (1, 1), (0, 0))]);
    enter(&mut engine, B, (0, 0), 0);
    let events = enter(&mut engine, A, (2, 2), 0);
    assert!(map_moves(&events, A).is_empty());
    assert!(vanished(&to(&events, B), account(A)).is_empty());
    assert_eq!(pos(&engine, A), (2, 2));
    // Still in the grid: moves work.
    let events = walk(&mut engine, A, (3, 3), 50, 50);
    assert!(to(&events, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::PlayerMoved { .. })));
}

#[test]
fn eng_warp_07_rng_is_deterministic_per_map_name() {
    let first = random_landing(grid(100, 100, &[]), (0, 0));
    let again = random_landing(grid(100, 100, &[]), (0, 0));
    assert_eq!(first, again);
}
