//! Public chat and `@item` (spec `specs/items/spec.md` ENG-CHAT-01/02, ENG-CMD-01..06).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, InventoryItem, ItemId,
    MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{ItemData, ItemDb, ItemType, MapGrid, MobDb};
use ro_engine::MapInstance;

const GM: u32 = 1;
const NEAR: u32 = 2;
const FAR: u32 = 3;
const PLAYER: u32 = 4;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn name(n: u32) -> String {
    format!("Char{n}")
}

fn data(id: u32, aegis: &str, name: &str, item_type: ItemType, weight: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: aegis.into(),
        name: name.into(),
        item_type,
        weight,
        locations: if item_type.is_stackable() || item_type == ItemType::PetEgg {
            0
        } else {
            0x2
        },
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
    }
}

fn engine() -> MapInstance {
    let items = ItemDb::from_items(
        "test",
        vec![
            data(501, "Red_Potion", "Red Potion", ItemType::Healing, 70),
            data(909, "Jellopy", "Jellopy", ItemType::Etc, 10),
            data(1201, "Knife", "Knife", ItemType::Weapon, 400),
            // A name that is also another item's id: names win over ids.
            data(4001, "Number_Item", "909", ItemType::Etc, 1),
            data(9001, "Poring_Egg", "Poring Egg", ItemType::PetEgg, 0),
            data(8000, "Anvil", "Anvil", ItemType::Etc, 30_000),
        ],
    )
    .expect("items");
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(Arc::new(items), Arc::new(MobDb::default()));
    for (n, x, group) in [(GM, 50, 99), (NEAR, 60, 0), (FAR, 61, 0), (PLAYER, 51, 0)] {
        let spawn = CharacterSpawn {
            account_id: AccountId(2_000_000 + n),
            char_id: char_id(n),
            name: name(n),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            hair_style: 1,
            hair_color: 1,
            pos: MapPosition { x, y: 50, dir: 0 },
            speed: 150,
            str: 1,
            group_id: group,
            inventory: Default::default(),
        };
        engine.spawn_character(spawn, 0).expect("spawn");
        send(&mut engine, n, ClientIntent::NotifyActorInit);
    }
    engine
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

fn say(engine: &mut MapInstance, n: u32, message: &str) -> Vec<ScopedServerEvent> {
    let text = format!("{} : {message}", name(n)).into_bytes();
    send(engine, n, ClientIntent::Chat { text })
}

fn self_line(text: &str) -> ServerEvent {
    ServerEvent::SelfChat {
        text: text.as_bytes().to_vec(),
    }
}

fn mine(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(n)))
        .map(|e| e.event.clone())
        .collect()
}

fn added(events: &[ServerEvent]) -> Vec<(u16, u32, u16)> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::ItemAdded { item } => Some((item.index, item.item_id, item.amount)),
            _ => None,
        })
        .collect()
}

fn items_of(engine: &MapInstance, n: u32) -> Vec<(u32, u16)> {
    engine
        .inventory_snapshot(char_id(n))
        .expect("inventory")
        .items()
        .map(|i| (i.item_id.0, i.amount))
        .collect()
}

/// ENG-CHAT-01: public chat goes to players within 9 cells, not the speaker, then the echo.
#[test]
fn eng_chat_01_public_chat() {
    let mut e = engine();
    let events = say(&mut e, PLAYER, "hello there");
    let text = format!("{} : hello there", name(PLAYER)).into_bytes();
    assert_eq!(
        events,
        vec![
            ScopedServerEvent::new(
                BroadcastScope::Characters(vec![char_id(GM), char_id(NEAR)]),
                ServerEvent::PublicChat {
                    gid: 2_000_000 + PLAYER,
                    text: text.clone()
                }
            ),
            ScopedServerEvent::new(
                BroadcastScope::SelfOnly(char_id(PLAYER)),
                ServerEvent::SelfChat { text }
            ),
        ],
        "NEAR is 9 cells away (in), FAR 10 (out)"
    );
}

/// ENG-CHAT-02: `@…` from a non-GM is chat; a wrong prefix or an over-long message is dropped.
#[test]
fn eng_chat_02_non_gm_command_is_chat() {
    let mut e = engine();
    let events = say(&mut e, NEAR, "@item 501");
    assert!(events
        .iter()
        .any(|ev| matches!(&ev.event, ServerEvent::PublicChat { text, .. } if text.ends_with(b"@item 501"))));
    assert!(items_of(&e, NEAR).is_empty());

    let wrong = send(
        &mut e,
        NEAR,
        ClientIntent::Chat {
            text: b"Someone : hi".to_vec(),
        },
    );
    assert!(wrong.is_empty());
    // rAthena keeps at most 254 message bytes (255 with its NUL).
    assert!(
        say(&mut e, NEAR, &"x".repeat(255)).is_empty(),
        "255 bytes is too long"
    );
    assert!(!say(&mut e, NEAR, &"x".repeat(254)).is_empty());
}

/// ENG-CMD-01: `@item 501 5` adds 5 Red Potions in one `0x0b41`.
#[test]
fn eng_cmd_01_stackable() {
    let mut e = engine();
    let events = mine(&say(&mut e, GM, "@item 501 5"), GM);
    assert_eq!(added(&events), vec![(2, 501, 5)]);
    assert_eq!(
        events.last(),
        Some(&self_line("Items added to your inventory."))
    );
    assert!(!events
        .iter()
        .any(|ev| matches!(ev, ServerEvent::PublicChat { .. })));
    assert_eq!(items_of(&e, GM), vec![(501, 5)]);
}

/// ENG-CMD-02: `@item Knife 3` adds 3 separate slots.
#[test]
fn eng_cmd_02_non_stackable() {
    let mut e = engine();
    let events = mine(&say(&mut e, GM, "@item Knife 3"), GM);
    assert_eq!(
        added(&events),
        vec![(2, 1201, 1), (3, 1201, 1), (4, 1201, 1)]
    );
    assert_eq!(items_of(&e, GM), vec![(1201, 1); 3]);
}

/// ENG-CMD-03: quoted names, aegis names, ids; lookup order is aegis, name, then id.
#[test]
fn eng_cmd_03_lookup() {
    let mut e = engine();
    say(&mut e, GM, "@item \"Red Potion\" 2");
    say(&mut e, GM, "@item red_potion");
    say(&mut e, GM, "@item 501");
    assert_eq!(items_of(&e, GM), vec![(501, 4)]);
    // "909" is the display name of item 4001, so it wins over id 909.
    say(&mut e, GM, "@item 909");
    assert_eq!(items_of(&e, GM), vec![(501, 4), (4001, 1)]);
    say(&mut e, GM, "@item JELLOPY");
    assert_eq!(items_of(&e, GM), vec![(501, 4), (4001, 1), (909, 1)]);
}

/// ENG-CMD-04: an unknown item or bad syntax gets a message and adds nothing.
#[test]
fn eng_cmd_04_errors() {
    let mut e = engine();
    assert_eq!(
        mine(&say(&mut e, GM, "@item Excalibur 1"), GM),
        vec![self_line("No item matches 'Excalibur'.")]
    );
    let usage = vec![self_line(
        "Usage: @item <item name or id>[:<item>...] [amount]",
    )];
    assert_eq!(mine(&say(&mut e, GM, "@item"), GM), usage);
    assert_eq!(mine(&say(&mut e, GM, "@item \"Red Potion 2"), GM), usage);
    // One bad name in a list stops the whole command.
    assert_eq!(
        mine(&say(&mut e, GM, "@item 501:nope 2"), GM),
        vec![self_line("No item matches 'nope'.")]
    );
    assert!(items_of(&e, GM).is_empty());
    assert_eq!(
        mine(&say(&mut e, GM, "@item Poring_Egg"), GM),
        vec![self_line("Pet eggs can't be created with @item yet."),],
        "nothing was added, so no success line"
    );
}

/// ENG-CMD-05: overweight or full sends the `0x0b41` failure with the last code.
#[test]
fn eng_cmd_05_failures() {
    let mut e = engine();
    assert_eq!(
        mine(&say(&mut e, GM, "@item Anvil"), GM),
        vec![ServerEvent::ItemAddFailed { result: 2 }]
    );
    assert_eq!(
        mine(&say(&mut e, GM, "@item 501 30001"), GM),
        vec![ServerEvent::ItemAddFailed { result: 5 }]
    );
    // Knives weigh 40.0 and STR 1 carries 2030.0: 50 fit, the 51st is overweight and the loop
    // stops there.
    let events = mine(&say(&mut e, GM, "@item Knife 60"), GM);
    assert_eq!(added(&events).len(), 50);
    assert_eq!(
        events.last(),
        Some(&ServerEvent::ItemAddFailed { result: 2 })
    );
    assert_eq!(
        events
            .iter()
            .filter(|ev| matches!(ev, ServerEvent::ItemAddFailed { .. }))
            .count(),
        1,
        "only the last failure is sent"
    );
}

/// ENG-CMD-06: a `:` list, amount <= 0 is 1, and an unknown command from a GM.
#[test]
fn eng_cmd_06_lists_amounts_unknown() {
    let mut e = engine();
    let events = mine(&say(&mut e, GM, "@item 501:Jellopy:Knife 2"), GM);
    assert_eq!(
        added(&events),
        vec![(2, 501, 2), (3, 909, 2), (4, 1201, 1), (5, 1201, 1)]
    );
    say(&mut e, GM, "@item Jellopy -3");
    say(&mut e, GM, "@item Jellopy 0");
    assert_eq!(
        e.inventory_snapshot(char_id(GM)).expect("inv").slots[1]
            .as_ref()
            .map(|i| i.amount),
        Some(4)
    );
    assert_eq!(
        mine(&say(&mut e, GM, "@warp prontera"), GM),
        vec![self_line("Unknown command: @warp")]
    );
    let given = InventoryItem::new(ItemId(501), 1);
    assert_eq!(e.give_item(char_id(PLAYER), &given), Ok(0));
}
