//! The mob-drop API (spec `specs/items/spec.md` ENG-MOBDROP-01..04). Not wired to mob death yet.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, CharId, CharacterSpawn, ClientIntent, FloorItemAppearance, ItemId, MapPosition,
    ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{
    CellType, ItemData, ItemDb, ItemType, MapGrid, MobClass, MobData, MobDb, MobDrop, MobId,
};
use ro_engine::{roll_mob_drops, MapInstance, MobDropRequest};

const PORING: u32 = 1002;
const OSIRIS: u32 = 1038;
const BAPHO_JR: u32 = 1039;
const JELLOPY: u32 = 909;
const KNIFE: u32 = 1201;
const SHINY: u32 = 7001;
const PILLAR: u32 = 7002;

fn item(id: u32, item_type: ItemType, drop_effect: u8) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: format!("Item_{id}"),
        name: format!("Item {id}"),
        item_type,
        weight: 1,
        locations: if item_type.is_stackable() { 0 } else { 0x2 },
        view: 0,
        view_id: None,
        slots: 0,
        stack_amount: 0,
        stack_inventory: false,
        unique_id: false,
        bind_on_equip: false,
        drop_effect,
        no_drop: false,
        no_drop_override: 100,
        ..ItemData::default()
    }
}

fn drop(item: u32, rate: u16) -> MobDrop {
    MobDrop {
        item: ItemId(item),
        rate,
    }
}

fn mob(id: u32, class: MobClass, mvp: bool, drops: Vec<MobDrop>) -> MobData {
    MobData {
        id: MobId(id),
        aegis_name: format!("MOB_{id}"),
        name: format!("Mob {id}"),
        level: 1,
        hp: 1,
        walk_speed: 150,
        class,
        mvp,
        drops,
        mvp_drops: vec![],
    }
}

fn engine(walls: &[(u16, u16)]) -> MapInstance {
    let mut grid = MapGrid::all_walkable(100, 100).expect("dims");
    for &(x, y) in walls {
        assert!(grid.set(x, y, CellType::Wall));
    }
    let items = ItemDb::from_items(
        "test",
        vec![
            item(JELLOPY, ItemType::Etc, 0),
            item(KNIFE, ItemType::Weapon, 0),
            item(SHINY, ItemType::Etc, 1),
            item(PILLAR, ItemType::Etc, 4),
        ],
    )
    .expect("items");
    let mobs = MobDb::from_mobs(
        "test",
        vec![
            mob(PORING, MobClass::Normal, false, vec![drop(JELLOPY, 7000)]),
            mob(OSIRIS, MobClass::Boss, true, vec![]),
            mob(BAPHO_JR, MobClass::Boss, false, vec![]),
        ],
    )
    .expect("mobs");
    let mut engine = MapInstance::new("prontera.gat", Arc::new(grid));
    engine.set_registries(ro_engine::Registries {
        items: Arc::new(items),
        mobs: Arc::new(mobs),
        ..Default::default()
    });
    engine
}

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn send(
    engine: &mut MapInstance,
    n: u32,
    intent: ClientIntent,
    now: u64,
) -> Vec<ScopedServerEvent> {
    engine.push_intent(SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(2_000_000 + n)),
        Some(char_id(n)),
        intent,
    ));
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick");
    out
}

fn enter(engine: &mut MapInstance, n: u32, cell: (u16, u16)) {
    enter_events(engine, n, cell);
}

fn enter_events(engine: &mut MapInstance, n: u32, (x, y): (u16, u16)) -> Vec<ScopedServerEvent> {
    let spawn = CharacterSpawn {
        account_id: AccountId(2_000_000 + n),
        char_id: char_id(n),
        name: format!("C{n}"),
        sex: Sex::Male,
        job_class: 0,
        base_level: 1,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition { x, y, dir: 0 },
        speed: 150,
        status: Default::default(),
        group_id: 0,
        inventory: Default::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    send(engine, n, ClientIntent::NotifyActorInit, 0)
}

fn tick(engine: &mut MapInstance, now: u64) -> Vec<ScopedServerEvent> {
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick");
    out
}

/// Every `0x0add` in the events (one per item, deduplicated by id).
fn drops(events: &[ScopedServerEvent]) -> Vec<FloorItemAppearance> {
    let mut seen: Vec<FloorItemAppearance> = Vec::new();
    for e in events {
        if let ServerEvent::FloorItemDropped { item } = &e.event {
            if !seen.iter().any(|s| s.id == item.id) {
                seen.push(item.clone());
            }
        }
    }
    seen
}

fn request(mob: u32, cell: (u16, u16)) -> MobDropRequest {
    MobDropRequest {
        mob_id: MobId(mob),
        cell,
        owners: [None; 3],
    }
}

/// ENG-MOBDROP-01: `roll % 10000 < rate` per entry in db order; the 9000 cap rule.
#[test]
fn eng_mobdrop_01_roll() {
    let m = mob(
        PORING,
        MobClass::Normal,
        false,
        vec![drop(1, 7000), drop(2, 100), drop(3, 10_000), drop(4, 9500)],
    );
    let rolls = |list: Vec<u32>| {
        let mut it = list.into_iter();
        move || it.next().expect("one roll per entry")
    };
    let ids = |v: Vec<ItemId>| v.into_iter().map(|i| i.0).collect::<Vec<_>>();
    assert_eq!(
        ids(roll_mob_drops(&m, rolls(vec![6999, 99, 9999, 9499]))),
        vec![1, 2, 3, 4]
    );
    assert_eq!(
        ids(roll_mob_drops(&m, rolls(vec![7000, 100, 19_999, 9500]))),
        vec![3]
    );
    // The modulo: 17_000 % 10000 = 7000 (no drop), 16_999 -> 6999 (drop).
    assert_eq!(
        ids(roll_mob_drops(&m, rolls(vec![16_999, 100, 0, 9500]))),
        vec![1, 3]
    );
    // A base rate above 9000 is not capped (no bonuses exist that could push one there):
    // 9400 < 9500 drops. Rate 10000 always drops.
    assert_eq!(
        ids(roll_mob_drops(&m, rolls(vec![7000, 100, 9999, 9400]))),
        vec![3, 4]
    );
}

/// ENG-MOBDROP-02: the first on the death cell, then SE / W / N; a free neighbour of a blocked
/// offset; the blocked offset itself when nothing near it is free.
#[test]
fn eng_mobdrop_02_cells() {
    let mut e = engine(&[]);
    let items = [ItemId(JELLOPY); 5].to_vec();
    assert_eq!(e.drop_items(&request(PORING, (50, 50)), &items, 0), 5);
    let cells: Vec<(u16, u16)> = drops(&tick(&mut e, 0)).iter().map(|d| (d.x, d.y)).collect();
    // No players see them, so no events: read them through a player who enters now.
    assert!(cells.is_empty());
    let entered: Vec<(u16, u16)> = enter_events(&mut e, 1, (40, 40))
        .iter()
        .filter_map(|ev| match &ev.event {
            ServerEvent::FloorItemEntered { item } => Some((item.x, item.y)),
            _ => None,
        })
        .collect();
    let mut expected = vec![(50, 50), (51, 49), (49, 50), (50, 51), (51, 49)];
    let mut got = entered.clone();
    expected.sort_unstable();
    got.sort_unstable();
    assert_eq!(got, expected);

    // A player on the SE cell blocks it: the item goes to a free neighbour of (51, 49).
    let mut e = engine(&[]);
    enter(&mut e, 1, (51, 49));
    let events = {
        e.drop_items(&request(PORING, (50, 50)), &[ItemId(JELLOPY); 2], 40);
        tick(&mut e, 40)
    };
    let placed = drops(&events);
    assert_eq!(
        (placed[0].x, placed[0].y),
        (50, 50),
        "the first ignores units"
    );
    let second = (placed[1].x, placed[1].y);
    assert_ne!(second, (51, 49));
    assert_eq!(second.0.abs_diff(51).max(second.1.abs_diff(49)), 1);

    // The SE cell and all its neighbours (the death cell too) are walls: the item stays on the
    // SE cell. The first item ignores the cell check.
    let mut walls = Vec::new();
    for x in 50..=52 {
        for y in 48..=50 {
            walls.push((x, y));
        }
    }
    let mut e = engine(&walls);
    enter(&mut e, 1, (45, 45));
    e.drop_items(&request(PORING, (50, 50)), &[ItemId(JELLOPY); 2], 40);
    let placed = drops(&tick(&mut e, 40));
    assert_eq!((placed[1].x, placed[1].y), (51, 49));
}

/// ENG-MOBDROP-03: owner ticks, normal (3 / 5 / 7 s) vs boss or MVP (10 / 20 / 22 s).
#[test]
fn eng_mobdrop_03_owner_ticks() {
    for (mob_id, waits) in [
        (PORING, [3_000, 5_000, 7_000]),
        (BAPHO_JR, [10_000, 20_000, 22_000]),
        (OSIRIS, [10_000, 20_000, 22_000]),
    ] {
        let mut e = engine(&[]);
        for n in 1..=4 {
            enter(&mut e, n, (51, 51));
        }
        let req = MobDropRequest {
            mob_id: MobId(mob_id),
            cell: (50, 50),
            owners: [Some(char_id(1)), Some(char_id(2)), Some(char_id(3))],
        };
        let start = 1_000;
        for (n, wait) in [(2, waits[0]), (3, waits[1]), (4, waits[2])] {
            e.drop_items(&req, &[ItemId(JELLOPY)], start);
            let id = drops(&tick(&mut e, start))[0].id;
            let early = send(
                &mut e,
                n,
                ClientIntent::PickUpItem { floor_id: id },
                start + wait - 1,
            );
            assert!(
                early
                    .iter()
                    .any(|ev| ev.event == ServerEvent::ItemAddFailed { result: 6 }),
                "mob {mob_id} char {n} early"
            );
            let on_time = send(
                &mut e,
                n,
                ClientIntent::PickUpItem { floor_id: id },
                start + wait,
            );
            assert!(
                on_time
                    .iter()
                    .any(|ev| matches!(ev.event, ServerEvent::ItemAdded { .. })),
                "mob {mob_id} char {n} on time"
            );
        }
    }
}

/// ENG-MOBDROP-04: `DropEffect` sets `showdropeffect` / `dropeffectmode`; equipment drops
/// unidentified; `drop_mob_items` rolls from the mob db.
#[test]
fn eng_mobdrop_04_effects_and_roll() {
    let mut e = engine(&[]);
    enter(&mut e, 1, (45, 45));
    e.drop_items(
        &request(PORING, (50, 50)),
        &[
            ItemId(JELLOPY),
            ItemId(SHINY),
            ItemId(PILLAR),
            ItemId(KNIFE),
        ],
        40,
    );
    let placed = drops(&tick(&mut e, 40));
    let fx: Vec<(u32, bool, u16, bool)> = placed
        .iter()
        .map(|d| {
            (
                d.item_id,
                d.show_drop_effect,
                d.drop_effect_mode,
                d.identified,
            )
        })
        .collect();
    assert_eq!(
        fx,
        vec![
            (JELLOPY, false, 0, true),
            (SHINY, true, 0, true),
            (PILLAR, true, 3, true),
            (KNIFE, false, 0, false),
        ]
    );

    // 70% Jellopy: over many kills some drop and some don't; unknown mobs drop nothing.
    let mut dropped = 0;
    for k in 0..100 {
        dropped += e.drop_mob_items(&request(PORING, (60, 60)), 100 + k);
    }
    assert!((40..=95).contains(&dropped), "{dropped}");
    assert_eq!(e.drop_mob_items(&request(1001, (60, 60)), 300), 0);
}
