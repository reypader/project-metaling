//! Equipment in the engine (spec `specs/equipment/spec.md` ENG-EQ-*, ENG-STAT-*). The expected
//! packet orders are the captured ones (catalog `packets/equipment.md` "Captured weapon swap",
//! "Captured equip sequences", capture 3 and capture 4).

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharacterInventory, CharacterSpawn, CharacterStatus, ClientIntent,
    InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent,
    Sex, UnitLooks,
};
use ro_db::{AmmoType, ItemData, ItemDb, ItemType, MapGrid, MobDb, WeaponType};
use ro_engine::MapInstance;

use crate::common::{
    char_id, gid, look, param, param_of, seen_by, A, ARROW, ASSASSIN, B, BOW, EQP_AMMO, EQP_ARMS,
    EQP_HAND_L, EQP_HAND_R, EQP_HEAD_TOP, EQP_SHOES, GUARD, KNIFE, LOOK_HEAD_BOTTOM, LOOK_HEAD_MID,
    LOOK_HEAD_TOP, LOOK_ROBE, LOOK_SHOES, LOOK_WEAPON, SP_ATK2, SP_CRITICAL, SP_DEF1, SP_DEF2,
};

mod equip;
mod gear_stat;
mod unequip;

const GLADIUS: u32 = 1219;
const DAMASCUS: u32 = 1222;
const JUR: u32 = 1250;
const ROD: u32 = 1601;
const WHIP: u32 = 1960;
const FLU_MASK: u32 = 2218;
const HAT: u32 = 2220;
const GOGGLES: u32 = 2225;
const SHOES: u32 = 2403;
const BRAVERY_BAG: u32 = 2576;
const CLIP: u32 = 2607;
const SAHKKAT: u32 = 5267;
const CAP: u32 = 5304;
const ONE_EYED_GLASS: u32 = 18874;
const C_SAHKKAT: u32 = 19639;
/// Not in the registry.
const UNKNOWN: u32 = 9999;

const EQP_HEAD_LOW: u32 = 0x01;
const EQP_GARMENT: u32 = 0x04;
const EQP_ACC_R: u32 = 0x08;
const EQP_ACC_L: u32 = 0x80;
const EQP_ACC: u32 = 0x88;
const EQP_HEAD_MID: u32 = 0x200;
const EQP_HELM: u32 = 0x301;
const EQP_COSTUME_HEAD_TOP: u32 = 0x400;
/// The equipment window's and the costume window's unequip-all masks (captured).
const UNEQUIP_ALL_WINDOW: u32 = 0x83ff;
const UNEQUIP_ALL_COSTUME: u32 = 0x3f_7c00;

const SP_MATK1: u16 = 43;
const SP_MATK2: u16 = 44;

/// Soft DEF (`SP_DEF1`) of the test characters: base level 10, VIT 1, AGI 1 (`(10 + 1) / 2 +
/// 1 / 5`, truncated). Hard DEF changes send it with `SP_DEF2`.
const SOFT_DEF: i32 = 5;
/// Status MATK (`SP_MATK2`) at base level 10 with INT/DEX/LUK 1.
const STATUS_MATK: i32 = 3;

fn item_data(id: u32, item_type: ItemType, locations: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: format!("Item_{id}"),
        name: format!("Item {id}"),
        item_type,
        weight: 100,
        locations,
        ..ItemData::default()
    }
}

fn weapon(id: u32, weapon_type: WeaponType, locations: u32, attack: u32) -> ItemData {
    ItemData {
        weapon_type,
        attack,
        range: 1,
        ..item_data(id, ItemType::Weapon, locations)
    }
}

fn armor(id: u32, locations: u32, defense: u32, view: u32) -> ItemData {
    ItemData {
        defense,
        view,
        ..item_data(id, ItemType::Armor, locations)
    }
}

fn items() -> Arc<ItemDb> {
    let bow = ItemData {
        range: 5,
        ..weapon(BOW, WeaponType::Bow, EQP_ARMS, 15)
    };
    let rod = ItemData {
        magic_attack: 30,
        ..weapon(ROD, WeaponType::Staff, EQP_HAND_R, 15)
    };
    Arc::new(
        ItemDb::from_items(
            "test",
            vec![
                weapon(KNIFE, WeaponType::Dagger, EQP_HAND_R, 17),
                weapon(GLADIUS, WeaponType::Dagger, EQP_HAND_R, 105),
                weapon(DAMASCUS, WeaponType::Dagger, EQP_HAND_R, 118),
                weapon(JUR, WeaponType::Katar, EQP_ARMS, 125),
                bow,
                rod,
                armor(GUARD, EQP_HAND_L, 20, 1),
                armor(FLU_MASK, EQP_HEAD_LOW, 0, 8),
                armor(HAT, EQP_HEAD_TOP, 2, 16),
                armor(GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID, 5, 1),
                armor(SHOES, EQP_SHOES, 10, 0),
                armor(BRAVERY_BAG, EQP_GARMENT, 20, 2),
                armor(CLIP, EQP_ACC, 0, 0),
                armor(SAHKKAT, EQP_HEAD_TOP, 14, 67),
                armor(CAP, EQP_HELM, 4, 326),
                armor(ONE_EYED_GLASS, EQP_HEAD_MID, 0, 23),
                armor(C_SAHKKAT, EQP_COSTUME_HEAD_TOP, 0, 67),
                ItemData {
                    ammo_type: Some(AmmoType::Arrow),
                    attack: 25,
                    ..item_data(ARROW, ItemType::Ammo, EQP_AMMO)
                },
                weapon(WHIP, WeaponType::Whip, EQP_HAND_R, 120),
            ],
        )
        .expect("items"),
    )
}

fn item(id: u32) -> Option<InventoryItem> {
    Some(InventoryItem::new(ItemId(id), 1))
}

fn worn(id: u32, equip: u32) -> Option<InventoryItem> {
    let mut item = InventoryItem::new(ItemId(id), 1);
    item.equip = equip;
    Some(item)
}

fn spawn_data(n: u32, inventory: Vec<Option<InventoryItem>>) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(gid(n)),
        char_id: char_id(n),
        name: format!("Char{n}"),
        sex: Sex::Male,
        job_class: 0,
        base_level: 10,
        hair_style: 2,
        hair_color: 3,
        pos: MapPosition {
            x: 50,
            y: 50,
            dir: 0,
        },
        speed: 150,
        status: CharacterStatus::default(),
        group_id: 0,
        inventory: CharacterInventory {
            slots: inventory,
            unique_item_counter: 0,
        },
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    }
}

fn instance() -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(ro_engine::Registries {
        items: items(),
        mobs: Arc::new(MobDb::default()),
        ..Default::default()
    });
    engine
}

fn send(
    engine: &mut MapInstance,
    n: u32,
    intent: ClientIntent,
    now: u64,
) -> Vec<ScopedServerEvent> {
    engine.push_intent(SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(gid(n))),
        Some(char_id(n)),
        intent,
    ));
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick");
    out
}

fn actor_init(engine: &mut MapInstance, n: u32, now: u64) -> Vec<ScopedServerEvent> {
    send(engine, n, ClientIntent::NotifyActorInit, now)
}

/// A spawned and visible character.
fn enter(engine: &mut MapInstance, spawn: CharacterSpawn, now: u64) -> Vec<ScopedServerEvent> {
    let n = spawn.char_id.0 - 150_000;
    engine.spawn_character(spawn, now).expect("spawn");
    actor_init(engine, n, now)
}

/// `A` wearing `inventory`, and an observer `B` next to it.
fn pair(inventory: Vec<Option<InventoryItem>>) -> MapInstance {
    let mut engine = instance();
    enter(&mut engine, spawn_data(A, inventory), 0);
    enter(&mut engine, spawn_data(B, vec![]), 0);
    engine
}

fn assassin(inventory: Vec<Option<InventoryItem>>) -> MapInstance {
    let mut engine = instance();
    enter(
        &mut engine,
        CharacterSpawn {
            job_class: ASSASSIN,
            ..spawn_data(A, inventory)
        },
        0,
    );
    engine
}

fn equip(engine: &mut MapInstance, index: u16, position: u32) -> Vec<ScopedServerEvent> {
    send(engine, A, ClientIntent::EquipItem { index, position }, 100)
}

fn take_off(engine: &mut MapInstance, index: u16) -> Vec<ScopedServerEvent> {
    send(engine, A, ClientIntent::UnequipItem { index }, 100)
}

fn unequip_all(engine: &mut MapInstance, location: u32) -> Vec<ScopedServerEvent> {
    send(engine, A, ClientIntent::UnequipAll { location }, 100)
}

fn self_events(events: Vec<ScopedServerEvent>, n: u32) -> Vec<ServerEvent> {
    events
        .into_iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| e.event)
        .collect()
}

fn attack_ranges(events: &[ServerEvent]) -> Vec<u16> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::AttackRange { range } => Some(*range),
            _ => None,
        })
        .collect()
}

fn window(events: &[ServerEvent]) -> ro_core::StatusWindow {
    events
        .iter()
        .find_map(|e| match e {
            ServerEvent::StatusWindow { status } => Some(*status),
            _ => None,
        })
        .expect("0x00bd sent")
}

fn hands(value: u32, value2: u32) -> ServerEvent {
    look(LOOK_WEAPON, value, value2)
}

fn ack(index: u16, location: u32, view: u16) -> ServerEvent {
    ServerEvent::EquipAck {
        index,
        location,
        view,
        result: 0,
    }
}

fn refused(index: u16, result: u8) -> ServerEvent {
    ServerEvent::EquipAck {
        index,
        location: 0,
        view: 0,
        result,
    }
}

fn off(index: u16, location: u32) -> ServerEvent {
    ServerEvent::UnequipAck {
        index,
        location,
        failed: false,
    }
}

fn off_failed(index: u16) -> ServerEvent {
    ServerEvent::UnequipAck {
        index,
        location: 0,
        failed: true,
    }
}

fn range(range: u16) -> ServerEvent {
    ServerEvent::AttackRange { range }
}

/// The entry packet `B` gets for `A` when it comes into view.
fn looks_seen_on_entry(engine: &mut MapInstance) -> UnitLooks {
    enter(engine, spawn_data(B, vec![]), 200)
        .into_iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(B)))
        .find_map(|e| match e.event {
            ServerEvent::UnitStanding { unit } if unit.gid == gid(A) => Some(unit.looks),
            _ => None,
        })
        .expect("B sees A")
}

fn stack(id: u32, amount: u16, equip: u32) -> Option<InventoryItem> {
    let mut item = InventoryItem::new(ItemId(id), amount);
    item.equip = equip;
    Some(item)
}

fn arrow_icon() -> ServerEvent {
    ServerEvent::StatusIconStarted {
        efst: 695,
        gid: gid(A),
        state: 1,
        total_ms: 9999,
        remain_ms: 9999,
        values: [0; 3],
    }
}
