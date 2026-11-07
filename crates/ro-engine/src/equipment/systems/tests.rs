use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, ClientIntent,
    InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent,
    Sex,
};
use ro_db::{MapGrid, MobDb};

use crate::equipment::EQP_HAND_R;
use crate::item::test_support::test_items;
use crate::map_instance::MapInstance;
use crate::protocol_consts::{DEAD, VANISH_DIED};

const KNIFE: u32 = 1201;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn send(engine: &mut MapInstance, n: u32, intent: ClientIntent) -> Vec<ScopedServerEvent> {
    engine.push_intent(SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(2_000_000 + n)),
        Some(char_id(n)),
        intent,
    ));
    let mut out = Vec::new();
    engine.step_tick(0, &mut out).expect("tick");
    out
}

fn enter(engine: &mut MapInstance, n: u32, slots: Vec<Option<InventoryItem>>) {
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
            x: 50,
            y: 50,
            dir: 0,
        },
        speed: 150,
        status: Default::default(),
        group_id: 0,
        inventory: CharacterInventory {
            slots,
            unique_item_counter: 0,
        },
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    send(engine, n, ClientIntent::NotifyActorInit);
}

/// A wearing a Knife (index 2) with a second one in its bag (index 3), B watching, A dead.
fn dead_wearer() -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(crate::Registries {
        items: Arc::new(test_items()),
        mobs: Arc::new(MobDb::default()),
        ..Default::default()
    });
    let mut worn = InventoryItem::new(ItemId(KNIFE), 1);
    worn.equip = EQP_HAND_R;
    let spare = InventoryItem::new(ItemId(KNIFE), 1);
    enter(&mut engine, 1, vec![Some(worn), Some(spare)]);
    enter(&mut engine, 2, vec![]);
    let e = engine.world.entity_by_char(char_id(1)).expect("A");
    engine.world.dead_sit[e.index()] = DEAD;
    engine
}

fn equips(engine: &MapInstance) -> Vec<u32> {
    let e = engine.world.entity_by_char(char_id(1)).expect("A");
    let inventory = engine.world.inventories.get(e).expect("inventory");
    (0..2)
        .map(|slot| inventory.get(slot).map_or(0, |item| item.equip))
        .collect()
}

fn died_to_the_area() -> Vec<ScopedServerEvent> {
    vec![ScopedServerEvent::new(
        BroadcastScope::Characters(vec![char_id(1), char_id(2)]),
        ServerEvent::UnitVanished {
            gid: 2_000_001,
            vanish_type: VANISH_DIED,
        },
    )]
}

/// ENG-EQ-06: a dead player's equip and take-off get `0x0080` type 1 to the area and no ack.
#[test]
fn eng_eq_06_dead_equip_and_take_off() {
    let mut engine = dead_wearer();
    let equip = ClientIntent::EquipItem {
        index: 3,
        position: EQP_HAND_R,
    };
    assert_eq!(send(&mut engine, 1, equip), died_to_the_area());
    assert_eq!(
        send(&mut engine, 1, ClientIntent::UnequipItem { index: 2 }),
        died_to_the_area()
    );
    assert_eq!(equips(&engine), vec![EQP_HAND_R, 0], "nothing changed");
}

/// ENG-EQ-08: a dead player's unequip-all gets `0x0bae` result 1 and removes nothing (our
/// ruling; rAthena runs it, spec §2.5).
#[test]
fn eng_eq_08_dead_unequip_all_is_refused() {
    let mut engine = dead_wearer();
    let out = send(
        &mut engine,
        1,
        ClientIntent::UnequipAll { location: 0x83ff },
    );
    assert_eq!(
        out,
        vec![ScopedServerEvent::new(
            BroadcastScope::SelfOnly(char_id(1)),
            ServerEvent::UnequipAllAck { failed: true },
        )]
    );
    assert_eq!(equips(&engine), vec![EQP_HAND_R, 0]);
}
