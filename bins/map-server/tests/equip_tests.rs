//! Equipment through the map-server (spec `specs/equipment/spec.md` BIN-EQ-01..03). The item
//! registry is the shipped `crates/ro-db/data/item`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;
use ro_core::{AccountId, CharacterLooks};
use ro_db::{MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};
use ro_packets::RawPacket;
use ro_persistence::CharacterRepository;

const A: u32 = 1;
const B: u32 = 2;
const SHORT: Duration = Duration::from_millis(150);
const LONG: Duration = Duration::from_secs(2);
const KNIFE: u32 = 1201;
/// Head top, `View` 16.
const HAT: u32 = 2220;
const EQP_HAND_R: u32 = 0x0002;
const EQP_HEAD_TOP: u32 = 0x0100;
const LOOK_WEAPON: u8 = 2;
/// `W_DAGGER`, the Knife's weapon type.
const W_DAGGER: u16 = 1;

fn cz_equip(index: u16, position: u32) -> Vec<u8> {
    let mut frame = vec![0x98, 0x09];
    frame.extend_from_slice(&index.to_le_bytes());
    frame.extend_from_slice(&position.to_le_bytes());
    frame
}

/// `(index, location, result)` of a `0x0999`.
fn equip_ack(frame: &RawPacket) -> (u16, u32, u8) {
    (
        frame_u16(frame, 2),
        frame_u32(frame, 4),
        frame_u8(frame, 10),
    )
}

/// `(item id, wear state)` of each `0x0b39` entry (68 bytes each, after `inv_type`).
fn worn_states(frames: &[RawPacket]) -> Vec<(u32, u32)> {
    frames
        .iter()
        .filter(|f| f.opcode.0 == 0x0b39)
        .flat_map(|f| f.payload[1..].chunks(68))
        .map(|e| {
            let id = u32::from_le_bytes(e[2..6].try_into().unwrap());
            let wear = u32::from_le_bytes(e[11..15].try_into().unwrap());
            (id, wear)
        })
        .collect()
}

/// Whether `frame` is a `LOOK_WEAPON` `0x01d7` for `n` showing `weapon` in the right hand.
fn is_weapon_look(frame: &RawPacket, n: u32, weapon: u32) -> bool {
    frame.opcode.0 == 0x01d7
        && frame_u32(frame, 2) == account(n)
        && frame_u8(frame, 6) == LOOK_WEAPON
        && frame_u32(frame, 7) == weapon
}

/// Equips the item at wire `index` and returns the `0x0999`.
async fn equip(client: &mut Client, index: u16, position: u32) -> (u16, u32, u8) {
    client.send(&cz_equip(index, position)).await;
    let ack = client.wait_opcode(0x0999, LONG).await.expect("0x0999");
    equip_ack(&ack)
}

/// BIN-EQ-01: `0x0998` on a knife gives `0x0999` and `0x01d7`; a second client sees the look.
#[tokio::test]
async fn bin_eq_01_equip_over_tcp_seen_by_observer() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100), (B, 104, 100)]);
    repo.set_inventory(A, inventory(&[(KNIFE, 1)]));
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    let (mut b, _) = enter(&server, B).await;
    a.collect(SHORT).await;

    assert_eq!(equip(&mut a, 2, EQP_HAND_R).await, (2, EQP_HAND_R, 0));
    let after = a.collect(SHORT).await;
    assert!(
        after.iter().any(|f| is_weapon_look(f, A, KNIFE)),
        "the wearer gets the look after the ack: {:?}",
        ops(&after)
    );
    assert!(
        b.wait_for(LONG, |f| is_weapon_look(f, A, KNIFE))
            .await
            .is_some(),
        "the observer sees the knife"
    );
}

/// BIN-EQ-02: equip, log out, log back in: still worn, and the char list has the looks.
#[tokio::test]
async fn bin_eq_02_worn_gear_survives_relog_and_shows_in_char_list() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_inventory(A, inventory(&[(KNIFE, 1), (HAT, 1)]));
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    assert_eq!(equip(&mut a, 2, EQP_HAND_R).await.2, 0);
    assert_eq!(equip(&mut a, 3, EQP_HEAD_TOP).await.2, 0);
    a.close().await;

    let repo = server.repo.clone();
    let expected = CharacterLooks {
        weapon_type: W_DAGGER,
        head_top: 16,
        ..CharacterLooks::default()
    };
    assert!(
        wait_until(|| repo.stored_looks(A) == Some(expected)).await,
        "looks saved on logout: {:?}",
        repo.stored_looks(A)
    );
    let list = repo
        .list_by_account(AccountId(account(A)))
        .await
        .expect("char list");
    assert_eq!(list[0].looks, expected, "the char list carries the looks");

    let (_a, frames) = enter(&server, A).await;
    assert_eq!(
        worn_states(&frames),
        [(KNIFE, EQP_HAND_R), (HAT, EQP_HEAD_TOP)],
        "still worn after the relog"
    );
    assert!(
        frames.iter().any(|f| is_weapon_look(f, A, KNIFE)),
        "the entry LOOK_WEAPON shows the knife"
    );
}

/// BIN-EQ-03: equip, then a cross-map warp: still worn, the map change saves the looks, and an
/// observer on the new map sees the knife in the entry packet.
#[tokio::test]
async fn bin_eq_03_warp_keeps_gear_and_looks() {
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
    let repo = InMemoryRepo::seeded_on(&[(A, MAP, 100, 100), (B, "geffen.gat", 102, 100)]);
    repo.set_inventory(A, inventory(&[(KNIFE, 1)]));
    let loader = maps_loader(vec![(MAP, MapNpcs { npcs: vec![warp] })]);
    let server = TestServer::start_with_loader(repo, loader).await;
    let (mut b, _) = enter(&server, B).await;
    let (mut a, _) = enter(&server, A).await;
    assert_eq!(equip(&mut a, 2, EQP_HAND_R).await.2, 0);

    a.send(&cz_request_move(105, 100)).await;
    assert!(a.wait_opcode(0x0091, LONG).await.is_some(), "warped");
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo
            .stored_looks(A)
            .is_some_and(|l| l.weapon_type == W_DAGGER))
        .await,
        "the map change saves the looks"
    );
    a.send(&[0x7d, 0x00]).await;
    let frames = a.collect(SHORT).await;
    assert_eq!(
        worn_states(&frames),
        [(KNIFE, EQP_HAND_R)],
        "worn on geffen"
    );
    assert!(frames.iter().any(|f| is_weapon_look(f, A, KNIFE)));

    // Entry packet `weapon` field (frame offset 27): the item id.
    let entry = b
        .wait_for(LONG, |f| is_entry(f) && entry_aid(f) == account(A))
        .await
        .expect("the observer sees A arrive");
    assert_eq!(frame_u32(&entry, 27), KNIFE);
}
