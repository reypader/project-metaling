//! Equipment in the engine (spec `specs/equipment/spec.md` ENG-EQ-*, ENG-STAT-*). The expected
//! packet orders are the captured ones (catalog `packets/equipment.md` "Captured weapon swap",
//! "Captured equip sequences", capture 3 and capture 4).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex, UnitLooks,
};
use ro_db::{AmmoType, ItemData, ItemDb, ItemType, MapGrid, MobDb, WeaponType};
use ro_engine::MapInstance;

const A: u32 = 1;
const B: u32 = 2;

const KNIFE: u32 = 1201;
const GLADIUS: u32 = 1219;
const DAMASCUS: u32 = 1222;
const JUR: u32 = 1250;
const ROD: u32 = 1601;
const BOW: u32 = 1701;
const ARROW: u32 = 1750;
const WHIP: u32 = 1960;
const GUARD: u32 = 2101;
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
const EQP_HAND_R: u32 = 0x02;
const EQP_GARMENT: u32 = 0x04;
const EQP_ACC_R: u32 = 0x08;
const EQP_HAND_L: u32 = 0x20;
const EQP_SHOES: u32 = 0x40;
const EQP_ACC_L: u32 = 0x80;
const EQP_ARMS: u32 = 0x22;
const EQP_ACC: u32 = 0x88;
const EQP_HEAD_TOP: u32 = 0x100;
const EQP_HEAD_MID: u32 = 0x200;
const EQP_HELM: u32 = 0x301;
const EQP_COSTUME_HEAD_TOP: u32 = 0x400;
const EQP_AMMO: u32 = 0x8000;
/// The equipment window's and the costume window's unequip-all masks (captured).
const UNEQUIP_ALL_WINDOW: u32 = 0x83ff;
const UNEQUIP_ALL_COSTUME: u32 = 0x3f_7c00;

/// `SP_*` ids of the pushed vars.
const SP_ATK2: u16 = 42;
const SP_MATK1: u16 = 43;
const SP_MATK2: u16 = 44;
const SP_DEF1: u16 = 45;
const SP_DEF2: u16 = 46;
const SP_CRITICAL: u16 = 52;

/// Soft DEF (`SP_DEF1`) of the test characters: base level 10, VIT 1, AGI 1 (`(10 + 1) / 2 +
/// 1 / 5`, truncated). Hard DEF changes send it with `SP_DEF2`.
const SOFT_DEF: i32 = 5;
/// Status MATK (`SP_MATK2`) at base level 10 with INT/DEX/LUK 1.
const STATUS_MATK: i32 = 3;

/// `LOOK_*` types.
const LOOK_WEAPON: u8 = 2;
const LOOK_HEAD_BOTTOM: u8 = 3;
const LOOK_HEAD_TOP: u8 = 4;
const LOOK_HEAD_MID: u8 = 5;
const LOOK_SHOES: u8 = 9;
const LOOK_ROBE: u8 = 12;

const ASSASSIN: u16 = 12;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
}

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

/// What character `n` receives, in order (its own and the area events).
fn seen_by(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(n),
            BroadcastScope::Characters(chars) => chars.contains(&char_id(n)),
            _ => false,
        })
        .map(|e| e.event.clone())
        .collect()
}

fn self_events(events: Vec<ScopedServerEvent>, n: u32) -> Vec<ServerEvent> {
    events
        .into_iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| e.event)
        .collect()
}

fn param_of(events: &[ServerEvent], var_id: u16) -> Option<i32> {
    events.iter().find_map(|e| match e {
        ServerEvent::ParameterChanged { var_id: v, value } if *v == var_id => Some(*value),
        _ => None,
    })
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

fn param(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

fn look(look_type: u8, value: u32, value2: u32) -> ServerEvent {
    ServerEvent::SpriteChanged {
        gid: gid(A),
        look_type,
        value,
        value2,
    }
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

/// ENG-STAT-02: a login wearing a Knife pushes `SP_ATK2` 17 and its `0x00bd` has
/// `atk_bonus` 17.
#[test]
fn eng_stat_02_login_push_and_window_carry_gear_values() {
    let mut engine = instance();
    let burst = engine
        .spawn_character(spawn_data(A, vec![worn(KNIFE, EQP_HAND_R)]), 0)
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_ATK2), Some(17));
    assert_eq!(attack_ranges(&burst), vec![1]);

    let init = self_events(actor_init(&mut engine, A, 10), A);
    let window = window(&init);
    assert_eq!(window.atk_bonus, 17);
    assert_eq!(window.hard_def, 0);
}

/// ENG-STAT-02 (MATK, capture 4): a Rod's `MagicAttack` is the weapon side, `SP_MATK1` and the
/// window's `matk_bonus`; the status side is the INT/DEX/LUK/level formula.
#[test]
fn eng_stat_02_weapon_matk_is_the_matk1_side() {
    let mut engine = instance();
    let burst = engine
        .spawn_character(spawn_data(A, vec![worn(ROD, EQP_HAND_R)]), 0)
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_MATK1), Some(30));
    assert_eq!(param_of(&burst, SP_MATK2), Some(STATUS_MATK));
    let window = window(&self_events(actor_init(&mut engine, A, 10), A));
    assert_eq!((window.matk_bonus, window.matk), (30, STATUS_MATK as i16));
}

/// ENG-EQ-11 (status part): spawning with worn gear builds the gear status from the masks:
/// a bow's range, and the hard DEF of a Guard and a Hat each counted once.
#[test]
fn eng_eq_11_spawn_builds_gear_status_from_the_masks() {
    let mut engine = instance();
    let burst = engine
        .spawn_character(
            spawn_data(A, vec![worn(BOW, EQP_ARMS), worn(HAT, EQP_HEAD_TOP)]),
            0,
        )
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_ATK2), Some(15), "the bow counts once");
    assert_eq!(param_of(&burst, SP_DEF2), Some(2));
    assert_eq!(param_of(&burst, SP_DEF1), Some(SOFT_DEF));
    assert_eq!(attack_ranges(&burst), vec![5]);

    let init = self_events(actor_init(&mut engine, A, 10), A);
    assert_eq!(attack_ranges(&init), vec![5], "the status block's 0x013a");

    let mut engine = instance();
    let burst = engine
        .spawn_character(
            spawn_data(
                A,
                vec![
                    worn(KNIFE, EQP_HAND_R),
                    worn(GUARD, EQP_HAND_L),
                    worn(HAT, EQP_HEAD_TOP),
                ],
            ),
            0,
        )
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_DEF2), Some(22));
    let window = window(&self_events(actor_init(&mut engine, A, 10), A));
    assert_eq!((window.atk_bonus, window.hard_def), (17, 22));
}

/// ENG-EQ-11 (looks): an observer's entry packet carries the spawned gear, and the cache follows
/// later equips.
#[test]
fn eng_eq_11_entry_packets_carry_the_looks() {
    let mut engine = instance();
    enter(
        &mut engine,
        spawn_data(
            A,
            vec![
                worn(KNIFE, EQP_HAND_R),
                worn(GUARD, EQP_HAND_L),
                worn(HAT, EQP_HEAD_TOP),
                worn(BRAVERY_BAG, EQP_GARMENT),
                item(GOGGLES),
            ],
        ),
        0,
    );
    let expected = UnitLooks {
        weapon: KNIFE,
        shield: GUARD,
        head_bottom: 0,
        head_top: 16,
        head_mid: 0,
        robe: 2,
    };
    assert_eq!(looks_seen_on_entry(&mut engine), expected);

    engine.remove_character(char_id(B)).expect("B leaves");
    equip(&mut engine, 6, EQP_HEAD_TOP | EQP_HEAD_MID);
    assert_eq!(
        looks_seen_on_entry(&mut engine),
        UnitLooks {
            head_top: 1,
            ..expected
        },
        "Goggles replaced the Hat"
    );
}

/// ENG-EQ-03, captured swap: Knife A worn, equip Knife B. Swapping back mirrors it.
#[test]
fn eng_eq_03_captured_swap() {
    let mut engine = pair(vec![worn(KNIFE, EQP_HAND_R), item(KNIFE)]);
    let out = equip(&mut engine, 3, EQP_HAND_R);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            off(2, EQP_HAND_R),
            ack(3, EQP_HAND_R, 0),
            hands(KNIFE, 0),
        ],
        "one net diff at the end of the tick: a Knife for a Knife changes nothing"
    );
    assert_eq!(seen_by(&out, B), vec![hands(0, 0), hands(KNIFE, 0)]);

    let out = equip(&mut engine, 2, EQP_HAND_R);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            off(3, EQP_HAND_R),
            ack(2, EQP_HAND_R, 0),
            hands(KNIFE, 0),
        ]
    );
}

/// ENG-EQ-04: a bow over Knife + Guard, per slot, then taking the bow off.
#[test]
fn eng_eq_04_two_hander_over_knife_and_guard() {
    let mut engine = pair(vec![
        worn(KNIFE, EQP_HAND_R),
        worn(GUARD, EQP_HAND_L),
        item(BOW),
    ]);
    let out = equip(&mut engine, 4, EQP_ARMS);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(KNIFE, 0),
            off(3, EQP_HAND_L),
            hands(0, BOW),
            off(2, EQP_HAND_R),
            ack(4, EQP_ARMS, 0),
            hands(BOW, 0),
            hands(BOW, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
            param(SP_ATK2, 15),
            range(5),
        ],
        "acks and looks as captured, then one net diff (ENG-STAT-02)"
    );
    assert_eq!(
        seen_by(&out, B),
        vec![hands(KNIFE, 0), hands(0, BOW), hands(BOW, 0), hands(BOW, 0)]
    );

    let out = take_off(&mut engine, 4);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(4, EQP_ARMS),
            param(SP_ATK2, 0),
            range(1),
        ],
        "two identical looks before the ack"
    );
}

/// ENG-EQ-05: headgear, garment, shoes.
#[test]
fn eng_eq_05_goggles_over_a_hat() {
    let mut engine = pair(vec![worn(HAT, EQP_HEAD_TOP), item(GOGGLES)]);
    let out = equip(&mut engine, 3, EQP_HEAD_TOP | EQP_HEAD_MID);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(2, EQP_HEAD_TOP),
            look(LOOK_HEAD_TOP, 0, 0),
            ack(3, EQP_HEAD_TOP | EQP_HEAD_MID, 1),
            look(LOOK_HEAD_TOP, 1, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 5),
        ],
        "only LOOK_HEAD_TOP; the net diff after the ack"
    );
    assert_eq!(
        seen_by(&out, B),
        vec![look(LOOK_HEAD_TOP, 0, 0), look(LOOK_HEAD_TOP, 1, 0)]
    );
}

#[test]
fn eng_eq_05_garment_and_shoes() {
    let mut engine = pair(vec![item(BRAVERY_BAG), item(SHOES)]);
    let out = equip(&mut engine, 2, EQP_GARMENT);
    assert_eq!(
        seen_by(&out, A),
        vec![
            ack(2, EQP_GARMENT, 2),
            look(LOOK_ROBE, 2, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 20),
        ]
    );
    let out = take_off(&mut engine, 2);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(2, EQP_GARMENT),
            look(LOOK_ROBE, 0, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
        ]
    );

    let out = equip(&mut engine, 3, EQP_SHOES);
    assert_eq!(
        seen_by(&out, A),
        vec![
            ack(3, EQP_SHOES, 0),
            look(LOOK_SHOES, SHOES, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 10),
        ],
        "LOOK_SHOES carries the item id"
    );
    let out = take_off(&mut engine, 3);
    assert_eq!(
        seen_by(&out, A),
        vec![
            look(LOOK_SHOES, 0, 0),
            off(3, EQP_SHOES),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
        ],
        "the shoes look comes before the ack"
    );
}

/// ENG-EQ-05, costumes (capture 4 lines 800–972): the costume top covers the normal top's
/// look; head looks go out only when the resolved view changes.
#[test]
fn eng_eq_05_costume_top_over_normal_tops() {
    let mut engine = pair(vec![
        worn(SAHKKAT, EQP_HEAD_TOP),
        item(C_SAHKKAT),
        item(CAP),
        worn(FLU_MASK, EQP_HEAD_LOW),
    ]);
    let costume = equip(&mut engine, 3, EQP_COSTUME_HEAD_TOP);
    assert_eq!(
        seen_by(&costume, A),
        vec![ack(3, EQP_COSTUME_HEAD_TOP, 67)],
        "same view as the Sahkkat underneath: no look"
    );

    let out = equip(&mut engine, 4, EQP_HELM);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(5, EQP_HEAD_LOW),
            look(LOOK_HEAD_BOTTOM, 0, 0),
            off(2, EQP_HEAD_TOP),
            ack(4, EQP_HELM, 326),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 4),
        ],
        "the costume covers the top: no LOOK_HEAD_TOP either way"
    );

    let out = take_off(&mut engine, 3);
    assert_eq!(
        seen_by(&out, A),
        vec![off(3, EQP_COSTUME_HEAD_TOP), look(LOOK_HEAD_TOP, 326, 0)],
        "removing the costume shows the normal top"
    );

    let out = equip(&mut engine, 2, EQP_HEAD_TOP);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(4, EQP_HELM),
            look(LOOK_HEAD_TOP, 0, 0),
            ack(2, EQP_HEAD_TOP, 67),
            look(LOOK_HEAD_TOP, 67, 0),
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 14),
        ],
        "a low + mid + top headgear sends only LOOK_HEAD_TOP when it comes off"
    );
}

/// ENG-EQ-05: a low + mid + top headgear over three items: each displaced item's look, then one
/// `LOOK_HEAD_TOP` (capture 4 lines 908–924).
#[test]
fn eng_eq_05_helm_over_three_headgears() {
    let mut engine = pair(vec![
        worn(FLU_MASK, EQP_HEAD_LOW),
        worn(ONE_EYED_GLASS, EQP_HEAD_MID),
        worn(HAT, EQP_HEAD_TOP),
        item(CAP),
    ]);
    let out = equip(&mut engine, 5, EQP_HELM);
    assert_eq!(
        seen_by(&out, B),
        vec![
            look(LOOK_HEAD_BOTTOM, 0, 0),
            look(LOOK_HEAD_MID, 0, 0),
            look(LOOK_HEAD_TOP, 0, 0),
            look(LOOK_HEAD_TOP, 326, 0),
        ]
    );
    let pushes: Vec<_> = seen_by(&out, A)
        .into_iter()
        .filter(|e| {
            matches!(
                e,
                ServerEvent::ParameterChanged {
                    var_id: SP_DEF2,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(
        pushes,
        vec![param(SP_DEF2, 4)],
        "one net diff: Hat 2 + Glass 0 + Mask 0 → Cap 4"
    );
}

/// ENG-EQ-06, failures that reach the client: not identified, broken, already worn, wrong
/// position → result 2; empty, out-of-range or unknown items → nothing.
#[test]
fn eng_eq_06_refusals() {
    let mut unidentified = InventoryItem::new(ItemId(KNIFE), 1);
    unidentified.identified = false;
    let mut broken = InventoryItem::new(ItemId(KNIFE), 1);
    broken.attribute = 1;
    let mut engine = pair(vec![
        Some(unidentified),
        Some(broken),
        worn(KNIFE, EQP_HAND_R),
        item(HAT),
        None,
        item(UNKNOWN),
    ]);
    for (index, position) in [
        (2, EQP_HAND_R),
        (3, EQP_HAND_R),
        (4, EQP_HAND_R),
        (5, EQP_HAND_R),
    ] {
        let out = equip(&mut engine, index, position);
        assert_eq!(seen_by(&out, A), vec![refused(index, 2)], "index {index}");
        assert!(seen_by(&out, B).is_empty());
    }
    for index in [0, 1, 6, 7, 200] {
        let out = equip(&mut engine, index, EQP_HAND_R);
        assert!(seen_by(&out, A).is_empty(), "index {index}: no ack");
    }
}

/// ENG-EQ-07, take-off: success order, and the failure forms.
#[test]
fn eng_eq_07_take_off() {
    let mut engine = pair(vec![worn(KNIFE, EQP_HAND_R), item(HAT)]);
    for (index, reply) in [(0, off_failed(2)), (1, off_failed(2)), (200, off_failed(2))] {
        assert_eq!(seen_by(&take_off(&mut engine, index), A), vec![reply]);
    }
    assert_eq!(
        seen_by(&take_off(&mut engine, 3), A),
        vec![off_failed(3)],
        "not worn"
    );
    assert_eq!(
        seen_by(&take_off(&mut engine, 9), A),
        vec![off_failed(9)],
        "empty slot"
    );

    let out = take_off(&mut engine, 2);
    assert_eq!(
        seen_by(&out, A),
        vec![hands(0, 0), off(2, EQP_HAND_R), param(SP_ATK2, 0)]
    );
    assert_eq!(seen_by(&out, B), vec![hands(0, 0)]);
}

/// ENG-EQ-08, unequip all: in `EQI` order, one push per item, then `0x0bae`. A two-hander comes
/// off once; the request mask filters.
#[test]
fn eng_eq_08_unequip_all() {
    let mut engine = pair(vec![
        worn(KNIFE, EQP_HAND_R),
        worn(GUARD, EQP_HAND_L),
        worn(HAT, EQP_HEAD_TOP),
        worn(BRAVERY_BAG, EQP_GARMENT),
        worn(C_SAHKKAT, EQP_COSTUME_HEAD_TOP),
    ]);
    let out = unequip_all(&mut engine, UNEQUIP_ALL_WINDOW);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(5, EQP_GARMENT),
            look(LOOK_ROBE, 0, 0),
            off(4, EQP_HEAD_TOP),
            hands(KNIFE, 0),
            off(3, EQP_HAND_L),
            hands(0, 0),
            off(2, EQP_HAND_R),
            ServerEvent::UnequipAllAck { failed: false },
            param(SP_DEF1, SOFT_DEF),
            param(SP_DEF2, 0),
            param(SP_ATK2, 0),
        ],
        "the costume stays, and still covers the Hat's look; one diff for all four items"
    );
    let out = unequip_all(&mut engine, UNEQUIP_ALL_COSTUME);
    assert_eq!(
        seen_by(&out, A),
        vec![
            off(6, EQP_COSTUME_HEAD_TOP),
            look(LOOK_HEAD_TOP, 0, 0),
            ServerEvent::UnequipAllAck { failed: false },
        ]
    );

    let mut engine = pair(vec![worn(BOW, EQP_ARMS)]);
    let out = unequip_all(&mut engine, UNEQUIP_ALL_WINDOW);
    assert_eq!(
        seen_by(&out, A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            ServerEvent::UnequipAllAck { failed: false },
            param(SP_ATK2, 0),
            range(1),
        ]
    );
}

/// ENG-EQ-01 in the flow (capture 4 lines 493–550): accessories go right, then left, then
/// replace the right one.
#[test]
fn eng_eq_01_accessory_slots_in_the_flow() {
    let mut engine = pair(vec![item(CLIP), item(CLIP), item(CLIP)]);
    assert_eq!(
        seen_by(&equip(&mut engine, 2, EQP_ACC), A),
        vec![ack(2, EQP_ACC_R, 0)]
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 3, EQP_ACC), A),
        vec![ack(3, EQP_ACC_L, 0)]
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 4, EQP_ACC), A),
        vec![off(2, EQP_ACC_R), ack(4, EQP_ACC_R, 0)]
    );
}

/// ENG-EQ-13, dual wield (capture 3 lines 721–740, 835–852).
#[test]
fn eng_eq_13_dual_wield() {
    let mut engine = assassin(vec![
        worn(KNIFE, EQP_HAND_R),
        item(DAMASCUS),
        item(GLADIUS),
        item(JUR),
    ]);
    assert_eq!(
        seen_by(&equip(&mut engine, 3, EQP_ARMS), A),
        vec![
            ack(3, EQP_HAND_L, 0),
            hands(KNIFE, DAMASCUS),
            param(SP_ATK2, 135),
        ]
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 4, EQP_ARMS), A),
        vec![
            hands(0, DAMASCUS),
            off(2, EQP_HAND_R),
            ack(4, EQP_HAND_R, 0),
            hands(GLADIUS, DAMASCUS),
            param(SP_ATK2, 223),
        ],
        "both hands taken: the right one is replaced"
    );
    assert_eq!(
        seen_by(&equip(&mut engine, 5, EQP_ARMS), A),
        vec![
            hands(GLADIUS, 0),
            off(3, EQP_HAND_L),
            hands(0, JUR),
            off(4, EQP_HAND_R),
            ack(5, EQP_ARMS, 0),
            hands(JUR, 0),
            hands(JUR, 0),
            param(SP_ATK2, 125),
            param(SP_CRITICAL, 2),
        ],
        "a katar doubles CRITICAL (ENG-STAT-03)"
    );
}

/// ENG-STAT-01 in the flow: a Rod pushes ATK2, then both MATK vars (capture 4 line 1399).
#[test]
fn eng_stat_01_rod_pushes_the_matk_pair() {
    let mut engine = pair(vec![item(ROD)]);
    let out = seen_by(&equip(&mut engine, 2, EQP_HAND_R), A);
    assert_eq!(
        out,
        vec![
            ack(2, EQP_HAND_R, 0),
            hands(ROD, 0),
            param(SP_ATK2, 15),
            param(SP_MATK2, STATUS_MATK),
            param(SP_MATK1, 30),
        ]
    );
}

/// ENG-STAT-03: every equip change bumps the inventory revision, so autosave sees it.
#[test]
fn eng_stat_03_equip_changes_bump_the_revision() {
    let mut engine = pair(vec![item(KNIFE), worn(HAT, EQP_HEAD_TOP)]);
    let revision = |engine: &MapInstance| engine.inventory_revision(char_id(A)).expect("A");
    let start = revision(&engine);
    equip(&mut engine, 2, EQP_HAND_R);
    let after_equip = revision(&engine);
    assert!(after_equip > start);
    take_off(&mut engine, 2);
    let after_take_off = revision(&engine);
    assert!(after_take_off > after_equip);
    unequip_all(&mut engine, UNEQUIP_ALL_WINDOW);
    assert!(revision(&engine) > after_take_off);
    let refused = revision(&engine);
    equip(&mut engine, 9, EQP_HAND_R);
    assert_eq!(revision(&engine), refused, "a refusal changes nothing");
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

/// ENG-EQ-09: arrows on a bow: `0x0983` to the area, `0x013c` and `0x013b` 3 to self, no
/// `0x0999`, no push. The client asks with position 0 or `0x8000`.
#[test]
fn eng_eq_09_arrows_on_a_bow() {
    for position in [0, EQP_AMMO] {
        let mut engine = pair(vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, 0)]);
        let out = equip(&mut engine, 3, position);
        assert_eq!(
            seen_by(&out, A),
            vec![
                arrow_icon(),
                ServerEvent::AmmoEquipped { index: 3 },
                ServerEvent::ActionFailure { failure_type: 3 },
            ],
            "position {position:#x}"
        );
        assert_eq!(seen_by(&out, B), vec![arrow_icon()]);
    }
}

/// ENG-EQ-09: arrows with no bow, or with a katar (capture 4): `0x0291` 1691, then result 2.
#[test]
fn eng_eq_09_arrows_need_a_bow() {
    for weapon in [
        vec![],
        vec![worn(KNIFE, EQP_HAND_R)],
        vec![worn(JUR, EQP_ARMS)],
    ] {
        let mut inventory = vec![stack(ARROW, 100, 0)];
        inventory.extend(weapon);
        let mut engine = pair(inventory);
        assert_eq!(
            seen_by(&equip(&mut engine, 2, EQP_AMMO), A),
            vec![ServerEvent::ClientMessage { msg_id: 1691 }, refused(2, 2)]
        );
    }
}

/// ENG-EQ-09: taking the bow off takes the arrows off too, before the one push.
#[test]
fn eng_eq_09_bow_take_off_cascades() {
    let mut engine = pair(vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, EQP_AMMO)]);
    assert_eq!(
        seen_by(&take_off(&mut engine, 2), A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            off(3, EQP_AMMO),
            param(SP_ATK2, 0),
            range(1),
        ]
    );
}

/// ENG-EQ-09: a Knife over the bow: the bow comes off without a cascade, and the arrows come
/// off after the Knife's look because they don't pair with it. A whip keeps them.
#[test]
fn eng_eq_09_weapon_change_cascades_when_the_ammo_no_longer_fits() {
    let mut engine = pair(vec![
        worn(BOW, EQP_ARMS),
        stack(ARROW, 100, EQP_AMMO),
        item(KNIFE),
        item(WHIP),
    ]);
    assert_eq!(
        seen_by(&equip(&mut engine, 4, EQP_HAND_R), A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            ack(4, EQP_HAND_R, 0),
            hands(KNIFE, 0),
            off(3, EQP_AMMO),
            param(SP_ATK2, 17),
            range(1),
        ]
    );

    let mut engine = pair(vec![
        worn(BOW, EQP_ARMS),
        stack(ARROW, 100, EQP_AMMO),
        item(WHIP),
    ]);
    let out = seen_by(&equip(&mut engine, 4, EQP_HAND_R), A);
    assert!(
        !out.contains(&off(3, EQP_AMMO)),
        "a whip takes arrows: {out:?}"
    );
}

/// ENG-EQ-08: unequip-all with a bow and arrows removes the arrows once, by the cascade.
#[test]
fn eng_eq_08_unequip_all_removes_arrows_once() {
    let mut engine = pair(vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, EQP_AMMO)]);
    assert_eq!(
        seen_by(&unequip_all(&mut engine, UNEQUIP_ALL_WINDOW), A),
        vec![
            hands(0, 0),
            hands(0, 0),
            off(2, EQP_ARMS),
            off(3, EQP_AMMO),
            ServerEvent::UnequipAllAck { failed: false },
            param(SP_ATK2, 0),
            range(1),
        ]
    );
}

/// ENG-EQ-12: worn arrows add `0x0983` + `0x013c` between the two inventory lists on every
/// actor init, to self only.
#[test]
fn eng_eq_12_map_entry_resends_the_ammo() {
    let mut engine = instance();
    engine
        .spawn_character(
            spawn_data(A, vec![worn(BOW, EQP_ARMS), stack(ARROW, 100, EQP_AMMO)]),
            0,
        )
        .expect("spawn");
    enter(&mut engine, spawn_data(B, vec![]), 0);
    for now in [10, 20] {
        let out = actor_init(&mut engine, A, now);
        let lists: Vec<_> = seen_by(&out, A)
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    ServerEvent::InventoryNormalItems { .. }
                        | ServerEvent::InventoryEquipItems { .. }
                        | ServerEvent::StatusIconStarted { .. }
                        | ServerEvent::AmmoEquipped { .. }
                )
            })
            .map(|e| match e {
                ServerEvent::InventoryNormalItems { .. } => "normal",
                ServerEvent::InventoryEquipItems { .. } => "equip",
                ServerEvent::StatusIconStarted { .. } => "icon",
                _ => "ammo",
            })
            .collect();
        assert_eq!(
            lists,
            vec!["normal", "icon", "ammo", "equip"],
            "actor init at {now}"
        );
        assert!(
            !seen_by(&out, B).iter().any(|e| matches!(
                e,
                ServerEvent::StatusIconStarted { .. } | ServerEvent::AmmoEquipped { .. }
            )),
            "the observer gets neither"
        );
    }
}

/// ENG-EQ-10 (unit level; the client won't send it): dropping a worn weapon unequips it after
/// the floor item appears and before `SP_WEIGHT` and `0x00af`. Dropping part of worn arrows
/// keeps them worn.
#[test]
fn eng_eq_10_dropping_worn_items() {
    let mut engine = pair(vec![worn(KNIFE, EQP_HAND_R), stack(ARROW, 100, EQP_AMMO)]);
    let drop = |engine: &mut MapInstance, index, amount| {
        send(engine, A, ClientIntent::DropItem { index, amount }, 100)
    };
    let out = drop(&mut engine, 2, 1);
    let seen: Vec<_> = seen_by(&out, A)
        .into_iter()
        .map(|e| match e {
            ServerEvent::FloorItemDropped { .. } => param(0, -1),
            other => other,
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            param(0, -1),
            hands(0, 0),
            off(2, EQP_HAND_R),
            param(24, 10_000),
            ServerEvent::ItemThrown {
                index: 2,
                amount: 1
            },
            param(SP_ATK2, 0),
        ],
        "0x0add, then the unequip, then SP_WEIGHT and 0x00af; the diff at the end of the tick"
    );
    assert!(
        seen_by(&out, B).contains(&hands(0, 0)),
        "the observer sees the look"
    );

    let out = drop(&mut engine, 3, 5);
    assert!(
        !seen_by(&out, A)
            .iter()
            .any(|e| matches!(e, ServerEvent::UnequipAck { .. })),
        "a partial drop keeps the arrows worn"
    );
    let out = actor_init(&mut engine, A, 200);
    assert!(seen_by(&out, A).contains(&ServerEvent::AmmoEquipped { index: 3 }));
}
