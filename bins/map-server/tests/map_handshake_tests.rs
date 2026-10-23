//! Restart-to-char-select and transfer validation (existing behaviour, BIN-SRV-06),
//! on the in-memory harness.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use axum::{extract::State, Json};
use common::*;
use map_server::accept_transfer;
use ro_core::{AccountId, CharId, MapPosition, PrepareMapTransferRequest};

#[tokio::test]
async fn test_map_restart_to_char_select() {
    let server = TestServer::start(&[(2, 100, 100)], &[]).await;
    let mut client = server.join(2).await;
    client.send(&[0xb2, 0x00, 0x01]).await;
    let ack = client
        .wait_opcode(0x00b3, Duration::from_secs(2))
        .await
        .expect("ZC_RESTART_ACK");
    assert_eq!(ack.payload.as_ref(), &[1]);
    assert!(
        server.aoi(100, 100).await.is_empty(),
        "character left the map"
    );
}

const LONG: Duration = Duration::from_secs(4);
/// `CZ_REQ_DISCONNECT` (0x018a), type 0, as the real client sends it.
const CZ_REQ_DISCONNECT: [u8; 4] = [0x8a, 0x01, 0x00, 0x00];

#[tokio::test]
async fn bin_quit_01_exit_acks_and_leaves_the_socket_to_the_client() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&CZ_REQ_DISCONNECT).await;
    let ack = a
        .wait_opcode(0x018b, LONG)
        .await
        .expect("ZC_ACK_REQ_DISCONNECT");
    assert_eq!(ack.payload.as_ref(), &[0, 0], "result 0: exit allowed");
    let (rest, eof) = a.read_until_eof(Duration::from_millis(400)).await;
    assert!(!eof, "server must not close the socket after the ack");
    assert!(rest.is_empty(), "nothing follows the ack: {rest:02x?}");
    assert!(
        server.aoi(100, 100).await.is_empty(),
        "character left the map"
    );
}

#[tokio::test]
async fn bin_quit_02_observer_sees_the_character_log_out() {
    let server = TestServer::start(&[(1, 100, 100), (2, 102, 100)], &[]).await;
    let mut a = server.join(1).await;
    let mut o = server.join(2).await;
    a.send(&CZ_REQ_DISCONNECT).await;
    assert!(a.wait_opcode(0x018b, LONG).await.is_some(), "0x018b");
    let vanish = o
        .wait_for(LONG, |f| {
            f.opcode.0 == 0x0080 && frame_u32(f, 2) == account(1)
        })
        .await
        .expect("observer gets 0x0080 for A");
    assert_eq!(frame_u8(&vanish, 6), 2, "vanish type 2: logged out");
    let after = a.collect(Duration::from_millis(200)).await;
    assert!(
        after.iter().all(|f| f.opcode.0 != 0x0080),
        "the quitter gets no vanish of its own"
    );
}

#[tokio::test]
async fn bin_quit_03_exit_persists_the_position_and_releases_the_character() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut a = server.join(1).await;
    a.send(&cz_request_move(102, 100)).await;
    assert!(server.wait_position(1, (102, 100), LONG).await);
    a.send(&CZ_REQ_DISCONNECT).await;
    assert!(a.wait_opcode(0x018b, LONG).await.is_some(), "0x018b");

    // The old socket stays open: the quit itself must save and release, not the close.
    let deadline = tokio::time::Instant::now() + LONG;
    let stored = loop {
        let stored = server.repo.stored(1).map(|(map, pos)| (map, pos.x, pos.y));
        if stored.as_ref().map(|s| (s.1, s.2)) == Some((102, 100))
            || tokio::time::Instant::now() >= deadline
        {
            break stored;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(stored, Some((MAP.to_string(), 102, 100)), "position saved");

    let _again = server.join(1).await;
    assert_eq!(
        server.position(1).await.map(|p| (p.x, p.y)),
        Some((102, 100)),
        "re-enters where it quit"
    );
    a.close().await;
}

fn transfer_request(char_id: u32, map: &str) -> PrepareMapTransferRequest {
    PrepareMapTransferRequest {
        token: "token".to_string(),
        account_id: AccountId(account(1)),
        char_id: CharId(char_id),
        login_id1: 1,
        destination_map: map.to_string(),
        destination_pos: MapPosition {
            x: 100,
            y: 100,
            dir: 0,
        },
        character: None,
    }
}

#[tokio::test]
async fn bin_srv_06_accept_transfer_validation() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let res = accept_transfer(
        State(server.service.clone()),
        Json(transfer_request(char_id(1), "morocc.gat")),
    )
    .await
    .0;
    assert!(res.accepted);

    for invalid_map in [
        "",
        "morocc",
        "morocc.txt",
        "../prontera.gat",
        "invalid/map.gat",
        "a_very_long_map_name_that_exceeds_limits_and_is_not_standard.gat",
    ] {
        let res = accept_transfer(
            State(server.service.clone()),
            Json(transfer_request(char_id(2), invalid_map)),
        )
        .await
        .0;
        assert!(!res.accepted, "Expected map '{invalid_map}' to be rejected");
    }
}
