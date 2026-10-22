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
