use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharacterInventory, CharacterSpawn, ClientIntent, InventoryItem,
    ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{MapGrid, MobDb};

use super::*;
use crate::item::test_support::test_items;
use crate::map_instance::MapInstance;
use crate::protocol_consts::DEAD;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn engine_with(capacity: u32) -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::with_capacity("prontera.gat", grid, capacity);
    engine.set_registries(crate::Registries {
        items: Arc::new(test_items()),
        mobs: Arc::new(MobDb::default()),
        ..Default::default()
    });
    engine
}

fn enter(engine: &mut MapInstance, n: u32, cell: (u16, u16)) {
    let spawn = CharacterSpawn {
        account_id: AccountId(2_000_000 + n),
        char_id: char_id(n),
        name: format!("C{n}"),
        sex: Sex::Male,
        job_class: 0,
        base_level: 1,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition {
            x: cell.0,
            y: cell.1,
            dir: 0,
        },
        speed: 150,
        status: Default::default(),
        group_id: 0,
        inventory: CharacterInventory {
            slots: vec![Some(InventoryItem::new(ItemId(501), 5))],
            unique_item_counter: 0,
        },
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    send(engine, n, ClientIntent::NotifyActorInit, 0);
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

fn jellopy() -> InventoryItem {
    InventoryItem::new(ItemId(909), 1)
}

fn result_of(events: &[ScopedServerEvent], n: u32) -> Option<u8> {
    events.iter().find_map(|e| match (&e.scope, &e.event) {
        (BroadcastScope::SelfOnly(c), ServerEvent::ItemAddFailed { result })
            if *c == char_id(n) =>
        {
            Some(*result)
        }
        (BroadcastScope::SelfOnly(c), ServerEvent::ItemAdded { .. }) if *c == char_id(n) => Some(0),
        _ => None,
    })
}

/// ENG-ITEM-04: a dead picker gets `0x0080` type 1 to the area (self too), then result 6.
#[test]
fn eng_item_04_dead_picker() {
    let mut engine = engine_with(64);
    enter(&mut engine, 1, (50, 50));
    enter(&mut engine, 2, (52, 50));
    let floor = engine
        .add_floor_item(jellopy(), (50, 51), FloorItemSpec::default(), 0)
        .expect("created");
    let id = engine.gid(floor);
    engine
        .step_tick(0, &mut Vec::new())
        .expect("flush the 0x0add");
    let e = engine.world.entity_by_char(char_id(1)).expect("A");
    engine.world.dead_sit[e.index()] = DEAD;
    let events = send(
        &mut engine,
        1,
        ClientIntent::PickUpItem { floor_id: id },
        20,
    );
    assert_eq!(events.len(), 2, "{events:#?}");
    assert_eq!(
        events[0],
        ScopedServerEvent::new(
            BroadcastScope::Characters(vec![char_id(1), char_id(2)]),
            ServerEvent::UnitVanished {
                gid: 2_000_001,
                vanish_type: 1
            }
        )
    );
    assert_eq!(result_of(&events, 1), Some(6));
    assert!(engine.world.floor_items.contains(floor), "the item stays");
}

/// ENG-ITEM-07: a dead dropper gets `0x00af` count 0 only.
#[test]
fn eng_item_07_dead_dropper() {
    let mut engine = engine_with(64);
    enter(&mut engine, 1, (50, 50));
    let e = engine.world.entity_by_char(char_id(1)).expect("A");
    engine.world.dead_sit[e.index()] = DEAD;
    let events = send(
        &mut engine,
        1,
        ClientIntent::DropItem {
            index: 2,
            amount: 1,
        },
        20,
    );
    assert_eq!(
        events,
        vec![ScopedServerEvent::new(
            BroadcastScope::SelfOnly(char_id(1)),
            ServerEvent::ItemThrown {
                index: 2,
                amount: 0
            }
        )]
    );
}

/// ENG-ITEM-04: the loot tick per owner tier.
#[test]
fn eng_item_04_owner_tiers() {
    let mut engine = engine_with(64);
    for n in 1..=4 {
        enter(&mut engine, n, (50, 50));
    }
    let spec = FloorItemSpec {
        owners: [Some(char_id(1)), Some(char_id(2)), Some(char_id(3))],
        owner_ticks: [3_000, 5_000, 7_000],
        ..FloorItemSpec::default()
    };
    // Owner 1 takes it at once.
    let floor = engine
        .add_floor_item(jellopy(), (50, 51), spec, 0)
        .expect("item");
    let id = engine.gid(floor);
    assert_eq!(
        result_of(
            &send(&mut engine, 1, ClientIntent::PickUpItem { floor_id: id }, 0),
            1
        ),
        Some(0)
    );

    // Each later tier waits for the previous tier's tick.
    for (n, ready) in [(2, 3_000), (3, 5_000), (4, 7_000)] {
        let floor = engine
            .add_floor_item(jellopy(), (50, 51), spec, 0)
            .expect("item");
        let id = engine.gid(floor);
        let early = send(
            &mut engine,
            n,
            ClientIntent::PickUpItem { floor_id: id },
            ready - 1,
        );
        assert_eq!(result_of(&early, n), Some(6), "char {n} too early");
        let on_time = send(
            &mut engine,
            n,
            ClientIntent::PickUpItem { floor_id: id },
            ready,
        );
        assert_eq!(result_of(&on_time, n), Some(0), "char {n} on time");
    }

    // No owner: anyone at once.
    let floor = engine
        .add_floor_item(jellopy(), (50, 51), FloorItemSpec::default(), 7_000)
        .expect("item");
    let id = engine.gid(floor);
    assert_eq!(
        result_of(
            &send(
                &mut engine,
                4,
                ClientIntent::PickUpItem { floor_id: id },
                7_000
            ),
            4
        ),
        Some(0)
    );
}

/// ENG-ITEM-08: the id counter wraps and skips ids in use; running out fails cleanly.
#[test]
fn eng_item_08_ids_wrap_and_capacity() {
    let mut engine = engine_with(64);
    engine.next_floor_id = MAX_FLOOR_ID - 1;
    let last = engine
        .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
        .expect("item");
    assert_eq!(engine.gid(last), MAX_FLOOR_ID - 1);
    let wrapped = engine
        .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
        .expect("item");
    assert_eq!(engine.gid(wrapped), MIN_FLOOR_ID);
    engine.next_floor_id = MIN_FLOOR_ID;
    let skipped = engine
        .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
        .expect("item");
    assert_eq!(engine.gid(skipped), MIN_FLOOR_ID + 1, "2 is taken");

    let mut tiny = engine_with(2);
    assert!(tiny
        .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
        .is_some());
    assert!(tiny
        .add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
        .is_some());
    let before = tiny.next_floor_id;
    assert!(
        tiny.add_floor_item(jellopy(), (50, 50), FloorItemSpec::default(), 0)
            .is_none(),
        "capacity exhausted"
    );
    assert_eq!(tiny.world.floor_items.len(), 2);
    assert!(tiny.floor_expiry.len() == 2 && tiny.next_floor_id != before);
}
