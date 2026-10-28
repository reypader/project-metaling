//! Items and chat through the map-server (spec `specs/items/spec.md` BIN-ITEM-01..06,
//! BIN-CHAT-01/02). The item registry is the shipped `crates/ro-db/data/item`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;
use map_server::{load_registries, MapServiceOptions, RegistryPaths};
use ro_db::{MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_packets::RawPacket;

const A: u32 = 1;
const B: u32 = 2;
const SHORT: Duration = Duration::from_millis(150);
const LONG: Duration = Duration::from_secs(2);
const POTION: u32 = 501;
const KNIFE: u32 = 1201;

fn chat(n: u32, message: &str) -> Vec<u8> {
    chat_raw(&format!("Char{n} : {message}"))
}

fn chat_raw(text: &str) -> Vec<u8> {
    let len = u16::try_from(4 + text.len()).expect("short");
    let mut frame = vec![0xf3, 0x00];
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(text.as_bytes());
    frame
}

fn cz_drop(index: u16, amount: u16) -> Vec<u8> {
    let mut frame = vec![0x63, 0x03];
    frame.extend_from_slice(&index.to_le_bytes());
    frame.extend_from_slice(&amount.to_le_bytes());
    frame
}

fn cz_pickup(id: u32) -> Vec<u8> {
    let mut frame = vec![0x62, 0x03];
    frame.extend_from_slice(&id.to_le_bytes());
    frame
}

/// `(wire index, item id, amount)` of each `0x0b09` / `0x0b39` entry (payload after the
/// dynamic header: inv_type, then entries).
fn list_entries(frames: &[RawPacket]) -> Vec<(u16, u32, u16)> {
    let mut out = Vec::new();
    for f in frames {
        let (size, amount_at) = match f.opcode.0 {
            0x0b09 => (34, Some(7)),
            0x0b39 => (68, None),
            _ => continue,
        };
        for e in f.payload[1..].chunks(size) {
            let index = u16::from_le_bytes([e[0], e[1]]);
            let id = u32::from_le_bytes([e[2], e[3], e[4], e[5]]);
            let amount = amount_at.map_or(1, |at| u16::from_le_bytes([e[at], e[at + 1]]));
            out.push((index, id, amount));
        }
    }
    out
}

/// BIN-ITEM-01: the entry lists are on the wire right after `0x007d`, in order.
#[tokio::test]
async fn bin_item_01_entry_lists_on_the_wire() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_inventory(A, inventory(&[(POTION, 5), (KNIFE, 1)]));
    let server = TestServer::start_with(repo, &[]).await;
    let (_a, frames) = enter(&server, A).await;
    assert_eq!(
        &ops(&frames)[..8],
        [0x01d7, 0x0b08, 0x0b09, 0x0b39, 0x0b0b, 0x0a9b, 0x00b0, 0x00b0],
        "{frames:?}"
    );
    assert_eq!(list_entries(&frames), [(2, POTION, 5), (3, KNIFE, 1)]);
    let weight = u32::from_le_bytes(frames[6].payload[2..6].try_into().unwrap());
    assert_eq!(weight, 5 * 70 + 400);
    // The knife isn't worn: LOOK_WEAPON with both hands empty (specs/map-entry-settings §5).
    assert_eq!(&frames[0].payload[4..], &[2, 0, 0, 0, 0, 0, 0, 0, 0]);
}

/// BIN-ITEM-02: `@item`, logout, relog: the item is listed; holes compact.
#[tokio::test]
async fn bin_item_02_item_survives_relog_and_holes_compact() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_gm(A);
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    a.send(&chat(A, "@item 501 5")).await;
    assert!(a.wait_opcode(0x0b41, LONG).await.is_some());
    a.send(&chat(A, "@item Knife")).await;
    assert!(a.wait_opcode(0x0b41, LONG).await.is_some());
    // Drop the whole potion stack: slot 0 becomes a hole.
    a.send(&cz_drop(2, 5)).await;
    assert!(a.wait_opcode(0x00af, LONG).await.is_some());
    a.close().await;
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo.stored_inventory(A).is_some_and(|i| i.slots.len() == 1)).await,
        "saved on logout"
    );

    let (_a, frames) = enter(&server, A).await;
    assert_eq!(
        list_entries(&frames),
        [(2, KNIFE, 1)],
        "the knife moved to slot 0"
    );
}

/// BIN-ITEM-03: a cross-map warp keeps the inventory and saves it.
#[tokio::test]
async fn bin_item_03_warp_keeps_and_saves() {
    let warp = NpcSpawn {
        name: "to_geffen".into(),
        unique_name: "to_geffen".into(),
        x: 105,
        y: 100,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: NpcState::Normal,
        trigger: Some((0, 0)),
        duplicate_of: None,
        kind: NpcKind::Warp(WarpDest {
            map: "geffen".into(),
            x: 100,
            y: 100,
            trigger_while_hidden: false,
        }),
        source: "test:1".into(),
    };
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_inventory(A, inventory(&[(POTION, 3)]));
    let loader = maps_loader(vec![(MAP, MapNpcs { npcs: vec![warp] })]);
    let server = TestServer::start_with_loader(repo, loader).await;
    let (mut a, _) = enter(&server, A).await;
    a.send(&cz_request_move(105, 100)).await;
    assert!(a.wait_opcode(0x0091, LONG).await.is_some(), "warped");
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo.inventory_saves_of(A) == 1).await,
        "saved on the map change"
    );
    assert_eq!(repo.stored_inventory(A), Some(inventory(&[(POTION, 3)])));
    a.send(&[0x7d, 0x00]).await;
    let frames = a.collect(SHORT).await;
    assert_eq!(list_entries(&frames), [(2, POTION, 3)], "listed on geffen");
}

/// BIN-ITEM-04: autosave writes after the interval, only when the inventory changed.
#[tokio::test]
async fn bin_item_04_autosave() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_gm(A);
    let options = MapServiceOptions {
        autosave_interval: Duration::from_millis(1200),
    };
    let server = TestServer::start_with_options(repo, grid_loader(&[]), options).await;
    let (mut a, _) = enter(&server, A).await;
    let repo = server.repo.clone();
    // Unchanged: no autosave even after the interval.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(
        repo.inventory_saves_of(A),
        0,
        "unchanged inventories aren't saved"
    );
    a.send(&chat(A, "@item 501 2")).await;
    assert!(a.wait_opcode(0x0b41, LONG).await.is_some());
    assert!(
        wait_until_for(|| repo.inventory_saves_of(A) == 1, Duration::from_secs(4)).await,
        "the change is autosaved"
    );
    assert_eq!(repo.stored_inventory(A), Some(inventory(&[(POTION, 2)])));
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(
        repo.inventory_saves_of(A),
        1,
        "no second save without a change"
    );
}

/// BIN-ITEM-05: two clients: the observer sees the drop and the pickup.
#[tokio::test]
async fn bin_item_05_drop_and_pickup_seen_by_observer() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100), (B, 104, 100)]);
    repo.set_inventory(A, inventory(&[(POTION, 3)]));
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    let (mut b, _) = enter(&server, B).await;
    a.collect(SHORT).await;

    a.send(&cz_drop(2, 1)).await;
    let fall = b
        .wait_opcode(0x0add, LONG)
        .await
        .expect("observer sees 0x0add");
    let id = u32::from_le_bytes(fall.payload[0..4].try_into().unwrap());
    let dropped = a.collect(SHORT).await;
    assert_eq!(ops(&dropped), [0x0add, 0x00b0, 0x00af], "dropper order");
    assert_eq!(&dropped[2].payload[..], &[2, 0, 1, 0]);

    a.send(&cz_pickup(id)).await;
    let mine = a.collect(SHORT).await;
    assert_eq!(ops(&mine), [0x0b41, 0x00b0, 0x08c8, 0x00a1]);
    let seen = b.collect(SHORT).await;
    assert_eq!(ops(&seen), [0x08c8, 0x00a1]);
}

/// BIN-ITEM-06: unset item/mob dirs give empty registries; a bad dir fails startup.
#[tokio::test]
async fn bin_item_06_registry_dirs() {
    let empty = load_registries(RegistryPaths::default()).expect("unset dirs are fine");
    assert!(empty.items.is_empty() && empty.mobs.is_empty());
    let missing = std::env::temp_dir().join("metaling-no-such-item-dir");
    for paths in [
        RegistryPaths {
            items: Some(&missing),
            ..RegistryPaths::default()
        },
        RegistryPaths {
            mobs: Some(&missing),
            ..RegistryPaths::default()
        },
    ] {
        assert!(load_registries(paths).is_err());
    }

    // A server with empty registries runs; an unknown stored item still lists as Etc.
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_inventory(A, inventory(&[(POTION, 1)]));
    let loader = map_server::map_data_loader(None, None, None, empty);
    let server = TestServer::start_with_loader(repo, loader).await;
    let (_a, frames) = enter(&server, A).await;
    assert_eq!(list_entries(&frames), [(2, POTION, 1)]);
    let entry_type = frames
        .iter()
        .find(|f| f.opcode.0 == 0x0b09)
        .map(|f| f.payload[1 + 6])
        .expect("normal list");
    assert_eq!(entry_type, 3, "unknown ids list as Etc");
}

/// BIN-CHAT-01: chat without the speaker's `"<name> : "` prefix closes the session.
#[tokio::test]
async fn bin_chat_01_prefix_mismatch_closes() {
    let server = TestServer::start(&[(A, 100, 100)], &[]).await;
    let (mut a, _) = enter(&server, A).await;
    // Too short to hold a message: ignored, the session stays.
    a.send(&chat_raw("Char1 : ")).await;
    a.send(&cz_request_time(1)).await;
    assert!(
        a.wait_opcode(0x007f, LONG).await.is_some(),
        "still connected"
    );
    a.send(&chat_raw("Someone : hi")).await;
    let (_, eof) = a.read_until_eof(LONG).await;
    assert!(eof, "the server closed the session");
}

/// BIN-CHAT-02: a non-GM `@item` is broadcast as chat.
#[tokio::test]
async fn bin_chat_02_non_gm_command_is_chat() {
    let server = TestServer::start(&[(A, 100, 100), (B, 104, 100)], &[]).await;
    let (mut a, _) = enter(&server, A).await;
    let (mut b, _) = enter(&server, B).await;
    a.collect(SHORT).await;
    a.send(&chat(A, "@item 501")).await;
    let heard = b.wait_opcode(0x008d, LONG).await.expect("B hears it");
    assert_eq!(
        u32::from_le_bytes(heard.payload[0..4].try_into().unwrap()),
        account(A)
    );
    assert_eq!(&heard.payload[4..], b"Char1 : @item 501\0");
    let echo = a.wait_opcode(0x008e, LONG).await.expect("echo");
    assert_eq!(&echo.payload[..], b"Char1 : @item 501\0");
    assert!(a.collect(SHORT).await.iter().all(|f| f.opcode.0 != 0x0b41));
}
