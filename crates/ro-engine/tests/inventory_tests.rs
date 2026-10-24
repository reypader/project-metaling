//! Inventory entry lists and warps (spec `specs/items/spec.md` ENG-INV-04, ENG-INV-06).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex,
};
use ro_db::{
    ItemData, ItemDb, ItemType, MapGrid, MapNpcs, MobDb, NpcKind, NpcSpawn, NpcState, WarpDest,
};
use ro_engine::{MapInstance, MapTransfer};

const A: u32 = 1;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn data(id: u32, item_type: ItemType, weight: u32, locations: u32, view: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: format!("Item_{id}"),
        name: format!("Item {id}"),
        item_type,
        weight,
        locations,
        view,
        view_id: None,
        slots: 0,
        stack_amount: 0,
        stack_inventory: false,
        unique_id: false,
        bind_on_equip: false,
        drop_effect: 0,
        no_drop: false,
        no_drop_override: 100,
    }
}

fn items() -> Arc<ItemDb> {
    let mut alias = data(1202, ItemType::Weapon, 400, 0x2, 1);
    alias.view_id = Some(ItemId(1201));
    Arc::new(
        ItemDb::from_items(
            "test",
            vec![
                data(501, ItemType::Healing, 70, 0, 0),
                data(909, ItemType::Etc, 10, 0, 0),
                data(1201, ItemType::Weapon, 400, 0x2, 1),
                alias,
                data(2220, ItemType::Armor, 200, 0x100, 16),
            ],
        )
        .expect("items"),
    )
}

fn spawn_data(n: u32, x: u16, y: u16, inventory: CharacterInventory) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(2_000_000 + n),
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
            str: 11,
            ..CharacterStatus::default()
        },
        group_id: 0,
        inventory,
    }
}

fn instance(npcs: Vec<NpcSpawn>) -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(items(), Arc::new(MobDb::default()));
    engine.spawn_npcs(&MapNpcs { npcs });
    engine
}

fn actor_init(engine: &mut MapInstance, n: u32, now: u64) -> Vec<ScopedServerEvent> {
    engine.push_intent(SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(2_000_000 + n)),
        Some(char_id(n)),
        ClientIntent::NotifyActorInit,
    ));
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick");
    out
}

fn to_self(events: &[ScopedServerEvent], n: u32) -> Vec<&ServerEvent> {
    events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| &e.event)
        .collect()
}

/// The inventory part of an ActorInit: everything up to SP_MAXWEIGHT.
fn inventory_part(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    let mine = to_self(events, n);
    let end = mine
        .iter()
        .position(|e| matches!(e, ServerEvent::ParameterChanged { var_id: 25, .. }))
        .expect("SP_MAXWEIGHT sent");
    mine[..=end].iter().map(|e| (*e).clone()).collect()
}

fn item(id: u32, amount: u16) -> Option<InventoryItem> {
    Some(InventoryItem::new(ItemId(id), amount))
}

/// ENG-INV-04: entry list order and contents; repeated on a second ActorInit.
#[test]
fn eng_inv_04_entry_lists() {
    let mut worn = InventoryItem::new(ItemId(2220), 1);
    worn.equip = 0x100;
    let inventory = CharacterInventory {
        slots: vec![
            item(501, 5),
            item(1202, 1),
            None,
            item(909, 2),
            Some(worn),
            item(1_999_999, 4),
        ],
        unique_item_counter: 0,
    };
    let mut engine = instance(vec![]);
    engine
        .spawn_character(spawn_data(A, 50, 50, inventory), 0)
        .expect("spawn");
    let events = actor_init(&mut engine, A, 0);
    let part = inventory_part(&events, A);
    assert_eq!(part.len(), 7, "{part:#?}");
    assert_eq!(part[0], ServerEvent::InventoryStart);
    let ServerEvent::InventoryNormalItems { items: normal } = &part[1] else {
        panic!("normal list, got {:?}", part[1]);
    };
    let rows: Vec<(u16, u32, u8, u16)> = normal
        .iter()
        .map(|e| (e.index, e.item_id, e.item_type, e.amount))
        .collect();
    assert_eq!(
        rows,
        vec![(2, 501, 0, 5), (5, 909, 3, 2), (7, 1_999_999, 3, 4)],
        "slot order, holes skipped, unknown id as Etc"
    );
    let ServerEvent::InventoryEquipItems { items: equip } = &part[2] else {
        panic!("equip list, got {:?}", part[2]);
    };
    let rows: Vec<(u16, u32, u8, u32, u32, u16)> = equip
        .iter()
        .map(|e| {
            (
                e.index,
                e.item_id,
                e.item_type,
                e.location,
                e.wear_state,
                e.look,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![(3, 1201, 5, 0x2, 0, 0), (6, 2220, 4, 0x100, 0x100, 16)],
        "alias view id; weapon sprite not sent in the list; headgear sprite is"
    );
    assert_eq!(part[3], ServerEvent::InventoryEnd);
    assert_eq!(part[4], ServerEvent::EquipSwitchList);
    let weight = 5 * 70 + 400 + 2 * 10 + 200;
    assert_eq!(
        part[5],
        ServerEvent::ParameterChanged {
            var_id: 24,
            value: weight
        }
    );
    assert_eq!(
        part[6],
        ServerEvent::ParameterChanged {
            var_id: 25,
            value: 23_300
        }
    );

    let again = actor_init(&mut engine, A, 20);
    assert_eq!(inventory_part(&again, A), part, "resent on every ActorInit");
}

/// ENG-INV-04: no stackables or no equipment omits that list.
#[test]
fn eng_inv_04_lists_omitted_when_empty() {
    let mut engine = instance(vec![]);
    let only_potions = CharacterInventory {
        slots: vec![item(501, 1)],
        unique_item_counter: 0,
    };
    engine
        .spawn_character(spawn_data(A, 50, 50, only_potions), 0)
        .expect("spawn");
    let part = inventory_part(&actor_init(&mut engine, A, 0), A);
    assert!(matches!(part[1], ServerEvent::InventoryNormalItems { .. }));
    assert_eq!(part[2], ServerEvent::InventoryEnd);

    let mut engine = instance(vec![]);
    let only_knife = CharacterInventory {
        slots: vec![item(1201, 1)],
        unique_item_counter: 0,
    };
    engine
        .spawn_character(spawn_data(A, 50, 50, only_knife), 0)
        .expect("spawn");
    let part = inventory_part(&actor_init(&mut engine, A, 0), A);
    assert!(matches!(part[1], ServerEvent::InventoryEquipItems { .. }));
    assert_eq!(part[2], ServerEvent::InventoryEnd);
}

/// ENG-INV-06: a cross-map warp carries the inventory; the snapshot is equal after arrival.
#[test]
fn eng_inv_06_warp_carries_inventory() {
    let warp = NpcSpawn {
        name: "w".into(),
        unique_name: "w".into(),
        x: 60,
        y: 50,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some((0, 0)),
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: "izlude".into(),
            x: 10,
            y: 10,
            trigger_while_hidden: false,
        }),
        source: "test:1".into(),
    };
    let mut from = instance(vec![warp]);
    let inventory = CharacterInventory {
        slots: vec![item(501, 5), None, item(1201, 1)],
        unique_item_counter: 3,
    };
    from.spawn_character(spawn_data(A, 59, 50, inventory), 0)
        .expect("spawn");
    actor_init(&mut from, A, 0);
    assert_eq!(
        from.give_item(char_id(A), &InventoryItem::new(ItemId(909), 1)),
        Ok(0)
    );
    let before = from.inventory_snapshot(char_id(A)).expect("snapshot");
    assert_eq!(
        before.slots[1].as_ref().map(|i| i.item_id),
        Some(ItemId(909))
    );

    from.push_intent(SessionIntent::new(
        SessionId(1),
        Some(AccountId(2_000_001)),
        Some(char_id(A)),
        ClientIntent::RequestMove {
            dest_x: 60,
            dest_y: 50,
        },
    ));
    let mut out = Vec::new();
    for now in (20..=600).step_by(20) {
        from.step_tick(now, &mut out).expect("tick");
    }
    let mut transfers: Vec<MapTransfer> = Vec::new();
    from.drain_transfers(&mut transfers);
    let transfer = transfers.pop().expect("warped");
    assert_eq!(transfer.spawn.inventory, before, "moved into the transfer");
    assert_eq!(from.inventory_snapshot(char_id(A)), None);

    let mut to = instance(vec![]);
    to.enter_by_warp(transfer.spawn, transfer.dest, transfer.rewarp_count, 600)
        .expect("arrives");
    assert_eq!(to.inventory_snapshot(char_id(A)), Some(before));
    assert_eq!(to.weight(char_id(A)), Some((5 * 70 + 400 + 10, 23_300)));
}
