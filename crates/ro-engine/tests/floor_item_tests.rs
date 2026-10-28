//! Floor items, pickup and player drop (spec `specs/items/spec.md` ENG-ITEM-01..07).
//! Dead-unit gates, loot owner tiers and id exhaustion are unit tests in `src/floor_item.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, FloorItemAppearance, InventoryItem, ItemId, MapPosition, ScopedServerEvent,
    ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{CellType, ItemData, ItemDb, ItemType, MapGrid, MobDb};
use ro_engine::MapInstance;

const A: u32 = 1;
const B: u32 = 2;
const POTION: u32 = 501;
const KNIFE: u32 = 1201;
const NO_DROP: u32 = 7000;
const ANVIL: u32 = 8000;
/// A weightless non-stackable, to fill slots.
const FEATHER: u32 = 2299;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
}

fn data(id: u32, item_type: ItemType, weight: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: format!("Item_{id}"),
        name: format!("Item {id}"),
        item_type,
        weight,
        locations: if item_type.is_stackable() { 0 } else { 0x2 },
        view: 0,
        view_id: None,
        slots: 0,
        stack_amount: 0,
        stack_inventory: false,
        unique_id: false,
        bind_on_equip: false,
        drop_effect: 0,
        no_drop: false,
        no_drop_override: 100,
        ..ItemData::default()
    }
}

fn items() -> Arc<ItemDb> {
    let mut no_drop = data(NO_DROP, ItemType::Etc, 1);
    no_drop.no_drop = true;
    Arc::new(
        ItemDb::from_items(
            "test",
            vec![
                data(POTION, ItemType::Healing, 70),
                data(KNIFE, ItemType::Weapon, 400),
                no_drop,
                data(ANVIL, ItemType::Etc, 30_000),
                data(FEATHER, ItemType::Armor, 0),
            ],
        )
        .expect("items"),
    )
}

fn stack(id: u32, amount: u16) -> Option<InventoryItem> {
    Some(InventoryItem::new(ItemId(id), amount))
}

fn spawn_data(
    n: u32,
    (x, y): (u16, u16),
    str: u16,
    slots: Vec<Option<InventoryItem>>,
) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(gid(n)),
        char_id: char_id(n),
        name: format!("Char{n}"),
        sex: Sex::Female,
        job_class: 0,
        base_level: 10,
        hair_style: 2,
        hair_color: 3,
        pos: MapPosition { x, y, dir: 0 },
        speed: 150,
        status: CharacterStatus {
            str,
            ..CharacterStatus::default()
        },
        group_id: 0,
        inventory: CharacterInventory {
            slots,
            unique_item_counter: 0,
        },
    }
}

struct World {
    engine: MapInstance,
    now: u64,
}

impl World {
    fn new(walls: &[(u16, u16)]) -> Self {
        let mut grid = MapGrid::all_walkable(100, 100).expect("dims");
        for &(x, y) in walls {
            assert!(grid.set(x, y, CellType::Wall));
        }
        let mut engine = MapInstance::new("prontera.gat", Arc::new(grid));
        engine.set_registries(ro_engine::Registries {
            items: items(),
            mobs: Arc::new(MobDb::default()),
            ..Default::default()
        });
        Self { engine, now: 0 }
    }

    fn enter(
        &mut self,
        n: u32,
        cell: (u16, u16),
        str: u16,
        slots: Vec<Option<InventoryItem>>,
    ) -> Vec<ScopedServerEvent> {
        self.engine
            .spawn_character(spawn_data(n, cell, str, slots), self.now)
            .expect("spawn");
        self.send(n, ClientIntent::NotifyActorInit)
    }

    fn send(&mut self, n: u32, intent: ClientIntent) -> Vec<ScopedServerEvent> {
        self.engine.push_intent(SessionIntent::new(
            SessionId(u64::from(n)),
            Some(AccountId(gid(n))),
            Some(char_id(n)),
            intent,
        ));
        self.tick(20)
    }

    fn tick(&mut self, ms: u64) -> Vec<ScopedServerEvent> {
        self.now += ms;
        let mut out = Vec::new();
        self.engine.step_tick(self.now, &mut out).expect("tick");
        out
    }

    fn drop(&mut self, n: u32, index: u16, amount: u16) -> Vec<ScopedServerEvent> {
        self.send(n, ClientIntent::DropItem { index, amount })
    }

    fn pick(&mut self, n: u32, floor_id: u32) -> Vec<ScopedServerEvent> {
        self.send(n, ClientIntent::PickUpItem { floor_id })
    }

    /// Walks `n` to `dest`, ticking until the walk ends.
    fn walk(&mut self, n: u32, dest: (u16, u16)) -> Vec<ScopedServerEvent> {
        let mut events = self.send(
            n,
            ClientIntent::RequestMove {
                dest_x: dest.0,
                dest_y: dest.1,
            },
        );
        for _ in 0..400 {
            events.extend(self.tick(50));
            if !self.engine.is_walking(char_id(n)) {
                break;
            }
        }
        events
    }
}

fn to(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(n),
            BroadcastScope::Characters(cs) => cs.contains(&char_id(n)),
            _ => false,
        })
        .map(|e| e.event.clone())
        .collect()
}

fn dropped(events: &[ServerEvent]) -> FloorItemAppearance {
    events
        .iter()
        .find_map(|e| match e {
            ServerEvent::FloorItemDropped { item } => Some(item.clone()),
            _ => None,
        })
        .expect("0x0add sent")
}

fn chebyshev(a: (u16, u16), b: (u16, u16)) -> u16 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
}

/// ENG-ITEM-06: drop success: packet order, the cell rule, and an observer sees `0x0add`.
#[test]
fn eng_item_06_drop_success() {
    let mut w = World::new(&[]);
    w.enter(A, (50, 50), 1, vec![stack(POTION, 40)]);
    w.enter(B, (60, 50), 1, vec![]);
    let events = w.drop(A, 2, 3);
    let mine = to(&events, A);
    assert_eq!(mine.len(), 3, "{mine:#?}");
    let item = dropped(&mine);
    assert_eq!((item.item_id, item.item_type, item.amount), (POTION, 0, 3));
    assert!(item.identified);
    assert!(!item.show_drop_effect && item.drop_effect_mode == 0);
    assert!([3, 6, 9, 12].contains(&item.sub_x) && [3, 6, 9, 12].contains(&item.sub_y));
    assert_eq!(
        mine[1],
        ServerEvent::ParameterChanged {
            var_id: 24,
            value: 37 * 70
        }
    );
    assert_eq!(
        mine[2],
        ServerEvent::ItemThrown {
            index: 2,
            amount: 3
        }
    );
    assert_eq!(
        to(&events, B),
        vec![ServerEvent::FloorItemDropped { item: item.clone() }]
    );

    // Never the own cell while a candidate is free; always within 2 on one of the 8 rays.
    for _ in 0..30 {
        let item = dropped(&to(&w.drop(A, 2, 1), A));
        let d = chebyshev((item.x, item.y), (50, 50));
        assert!((1..=2).contains(&d), "{:?}", (item.x, item.y));
    }
    assert_eq!(w.engine.weight(char_id(A)), Some((7 * 70, 20_300)));
}

/// ENG-ITEM-06: the 5-cell edge margin, and the own cell when no candidate is free.
#[test]
fn eng_item_06_drop_cells_edge_and_fallback() {
    let mut w = World::new(&[]);
    w.enter(A, (6, 50), 1, vec![stack(POTION, 40)]);
    for _ in 0..30 {
        let item = dropped(&to(&w.drop(A, 2, 1), A));
        assert!(item.x >= 5, "edge margin: {:?}", (item.x, item.y));
    }

    let mut walls = Vec::new();
    for x in 48..=52 {
        for y in 48..=52 {
            if (x, y) != (50, 50) {
                walls.push((x, y));
            }
        }
    }
    let mut w = World::new(&walls);
    w.enter(A, (50, 50), 1, vec![stack(POTION, 5)]);
    let item = dropped(&to(&w.drop(A, 2, 1), A));
    assert_eq!(
        (item.x, item.y),
        (50, 50),
        "no free cell: the dropper's own cell"
    );
}

/// ENG-ITEM-07: every drop gate answers `0x00af` count 0 and nothing else.
#[test]
fn eng_item_07_drop_failures() {
    let mut switched = InventoryItem::new(ItemId(KNIFE), 1);
    switched.equip_switch = 2;
    let mut w = World::new(&[]);
    w.enter(
        A,
        (50, 50),
        1,
        vec![stack(POTION, 5), None, Some(switched), stack(NO_DROP, 1)],
    );
    let cases: [(&str, u16, u16); 8] = [
        ("index 0", 0, 1),
        ("index 1", 1, 1),
        ("past the inventory", 102, 1),
        ("amount 0", 2, 0),
        ("more than the stack", 2, 6),
        ("empty slot", 3, 1),
        ("equip switch", 4, 1),
        ("NoDrop", 5, 1),
    ];
    for (what, index, amount) in cases {
        let events = w.drop(A, index, amount);
        assert_eq!(
            to(&events, A),
            vec![ServerEvent::ItemThrown { index, amount: 0 }],
            "{what}"
        );
        assert_eq!(events.len(), 1, "{what}: nothing else");
    }
    assert_eq!(
        w.engine.weight(char_id(A)),
        Some((5 * 70 + 400 + 1, 20_300))
    );
}

/// ENG-ITEM-01: `0x009d` on entry and walking in; `0x00a1` to self walking out.
#[test]
fn eng_item_01_visibility() {
    let mut w = World::new(&[]);
    w.enter(A, (50, 50), 1, vec![stack(POTION, 5)]);
    let item = dropped(&to(&w.drop(A, 2, 1), A));

    let entry = to(&w.enter(B, (55, 50), 1, vec![]), B);
    let entered: Vec<_> = entry
        .iter()
        .filter(|e| matches!(e, ServerEvent::FloorItemEntered { .. }))
        .collect();
    assert_eq!(entered.len(), 1);
    let ServerEvent::FloorItemEntered { item: seen } = entered[0] else {
        unreachable!()
    };
    assert_eq!(
        (seen.id, seen.x, seen.y, seen.amount),
        (item.id, item.x, item.y, 1)
    );
    assert_eq!((seen.sub_x, seen.sub_y), (item.sub_x, item.sub_y));

    let out = to(&w.walk(B, (item.x + 16, 50)), B);
    assert!(
        out.contains(&ServerEvent::FloorItemVanished { id: item.id }),
        "{out:#?}"
    );
    assert!(!out
        .iter()
        .any(|e| matches!(e, ServerEvent::UnitVanished { gid, .. } if *gid == item.id)));
    let back = to(&w.walk(B, (55, 50)), B);
    assert!(back
        .iter()
        .any(|e| matches!(e, ServerEvent::FloorItemEntered { item: i } if i.id == item.id)));
}

/// ENG-ITEM-02: expiry at 60 s sends `0x00a1` to the area and frees the id.
#[test]
fn eng_item_02_expiry() {
    let mut w = World::new(&[]);
    w.enter(A, (50, 50), 1, vec![stack(POTION, 5)]);
    w.enter(B, (55, 50), 1, vec![]);
    let start = w.now + 20;
    let item = dropped(&to(&w.drop(A, 2, 1), A));
    let before = w.tick(start + 59_999 - w.now);
    assert!(before.is_empty(), "{before:#?}");
    let gone = w.tick(1);
    let vanish = ServerEvent::FloorItemVanished { id: item.id };
    assert_eq!(to(&gone, A), vec![vanish.clone()]);
    assert_eq!(to(&gone, B), vec![vanish]);
    assert_eq!(
        to(&w.pick(A, item.id), A),
        vec![ServerEvent::ItemAddFailed { result: 6 }],
        "the id is free"
    );
    assert!(w.tick(120_000).is_empty(), "no second expiry");
}

/// ENG-ITEM-03: pickup success order for the picker and for an observer.
#[test]
fn eng_item_03_pickup_order() {
    let mut w = World::new(&[]);
    w.enter(A, (50, 50), 1, vec![stack(KNIFE, 1), stack(POTION, 5)]);
    w.enter(B, (55, 50), 1, vec![]);
    let item = dropped(&to(&w.drop(A, 3, 2), A));
    let events = w.pick(A, item.id);
    let mine = to(&events, A);
    assert_eq!(mine.len(), 4, "{mine:#?}");
    let ServerEvent::ItemAdded { item: added } = &mine[0] else {
        panic!("0x0b41 first: {mine:#?}");
    };
    assert_eq!(
        (added.index, added.item_id, added.amount),
        (3, POTION, 2),
        "merged back"
    );
    assert_eq!(
        mine[1],
        ServerEvent::ParameterChanged {
            var_id: 24,
            value: 400 + 5 * 70
        }
    );
    let act = ServerEvent::EntityAction {
        source_id: gid(A),
        target_id: item.id,
        action_type: 1,
        action_tick: 0,
    };
    let vanish = ServerEvent::FloorItemVanished { id: item.id };
    assert_eq!(&mine[2..], &[act.clone(), vanish.clone()]);
    assert_eq!(to(&events, B), vec![act, vanish]);
}

/// ENG-ITEM-04: unknown id, distance 3, overweight and full: result codes; the item stays.
#[test]
fn eng_item_04_pickup_failures() {
    let mut w = World::new(&[]);
    w.enter(A, (50, 50), 100, vec![stack(ANVIL, 1), stack(KNIFE, 1)]);
    let refused = vec![ServerEvent::ItemAddFailed { result: 6 }];
    assert_eq!(to(&w.pick(A, 12_345), A), refused, "unknown id");
    assert_eq!(to(&w.pick(A, gid(A)), A), refused, "not a floor item");

    let anvil = dropped(&to(&w.drop(A, 2, 1), A));
    let knife = dropped(&to(&w.drop(A, 3, 1), A));
    // B stands 3 cells from the anvil.
    let far = (anvil.x + 3, anvil.y);
    let full: Vec<_> = (0..100).map(|_| stack(FEATHER, 1)).collect();
    w.enter(B, far, 1, full);
    assert_eq!(to(&w.pick(B, anvil.id), B), refused, "distance 3");

    // STR 1 can't carry 3000.0; weight is checked before the free slot.
    w.walk(B, (anvil.x + 2, anvil.y));
    assert_eq!(
        to(&w.pick(B, anvil.id), B),
        vec![ServerEvent::ItemAddFailed { result: 2 }],
        "overweight"
    );
    w.walk(B, (knife.x, knife.y));
    assert_eq!(
        to(&w.pick(B, knife.id), B),
        vec![ServerEvent::ItemAddFailed { result: 4 }],
        "inventory full"
    );
    // Both items stayed: A can still take them.
    w.walk(A, (anvil.x, anvil.y));
    assert!(matches!(
        to(&w.pick(A, anvil.id), A)[0],
        ServerEvent::ItemAdded { .. }
    ));
    w.walk(A, (knife.x, knife.y));
    assert!(matches!(
        to(&w.pick(A, knife.id), A)[0],
        ServerEvent::ItemAdded { .. }
    ));
}

/// ENG-ITEM-05: a sitting picker succeeds and stays sitting; a walking picker keeps walking.
#[test]
fn eng_item_05_sitting_and_walking() {
    let mut w = World::new(&[]);
    w.enter(A, (50, 50), 1, vec![stack(POTION, 5)]);
    let first = dropped(&to(&w.drop(A, 2, 1), A));
    let second = dropped(&to(&w.drop(A, 2, 1), A));
    w.send(
        A,
        ClientIntent::RequestAction {
            target_id: 0,
            action_type: 2,
        },
    );
    assert_eq!(w.engine.dead_sit(char_id(A)), Some(2));
    assert!(matches!(
        to(&w.pick(A, first.id), A)[0],
        ServerEvent::ItemAdded { .. }
    ));
    assert_eq!(w.engine.dead_sit(char_id(A)), Some(2), "still sitting");
    w.send(
        A,
        ClientIntent::RequestAction {
            target_id: 0,
            action_type: 3,
        },
    );

    // Start a walk (within the click limit) towards the item's side and pick up on the way.
    let dest = (
        if second.x >= 50 { 60 } else { 40 },
        if second.y >= 50 { 51 } else { 49 },
    );
    w.send(
        A,
        ClientIntent::RequestMove {
            dest_x: dest.0,
            dest_y: dest.1,
        },
    );
    assert!(w.engine.is_walking(char_id(A)));
    let events = w.pick(A, second.id);
    assert!(
        matches!(to(&events, A)[0], ServerEvent::ItemAdded { .. }),
        "{events:#?}"
    );
    assert!(w.engine.is_walking(char_id(A)), "the walk goes on");
    assert!(!to(&events, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::UnitStopped { .. })));
}
