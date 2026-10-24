//! Map-entry settings and the load confirm over TCP (spec `specs/map-entry-settings/spec.md`
//! BIN-SET-01, updated by §5).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;

const LONG: Duration = Duration::from_secs(4);

#[tokio::test]
async fn bin_set_01_login_gets_settings_and_load_confirm() {
    let server = TestServer::start(&[(1, 100, 100)], &[]).await;
    let mut client = Client::connect(server.addr).await;
    client.send(&cz_enter(1)).await;
    let burst = client.read_exact_bytes(23).await;
    assert_eq!(&burst[0..2], &[0x83, 0x02], "burst starts with ZC_AID");
    assert_eq!(
        &burst[10..12],
        &[0xeb, 0x02],
        "ZC_ACCEPT_ENTER ends the burst"
    );
    let friends = client.read_exact_bytes(4).await;
    assert_eq!(friends, [0x01, 0x02, 0x04, 0x00], "empty ZC_FRIENDS_LIST");

    // §5: MOTD, 0x0091 to the own cell, the status push, then 0x09e7, 0x0a24, 0x0ade.
    let login = client.read_login_burst().await;
    let opcodes: Vec<u16> = login.iter().map(|f| f.opcode.0).collect();
    assert_eq!(&opcodes[..3], &[0x008e, 0x0091, 0x00b0], "{opcodes:04x?}");
    assert_eq!(
        &opcodes[opcodes.len() - 3..],
        &[0x09e7, 0x0a24, 0x0ade],
        "{opcodes:04x?}"
    );
    let map_move = &login[1].payload;
    assert_eq!(&map_move[..MAP.len()], MAP.as_bytes());
    assert_eq!(
        &map_move[16..],
        &[100, 0, 100, 0],
        "the character's own cell"
    );

    client.send(&[0x7d, 0x00]).await;
    let look = client.wait_opcode(0x01d7, LONG).await.expect("0x01d7");
    assert_eq!(frame_len(&look), 15);
    assert!(client.wait_opcode(0x099b, LONG).await.is_some(), "0x099b");
    let mut seen = Vec::new();
    for opcode in [0x010f, 0x0b20, 0x00bd, 0x02c9, 0x02da, 0x02d9, 0x0b8d] {
        let frame = client
            .wait_opcode(opcode, LONG)
            .await
            .unwrap_or_else(|| panic!("{opcode:#06x} after ACTORINIT"));
        seen.push((opcode, frame_len(&frame)));
    }
    assert_eq!(
        seen,
        [
            (0x010f, 41),
            (0x0b20, 271),
            (0x00bd, 44),
            (0x02c9, 3),
            (0x02da, 3),
            (0x02d9, 10),
            (0x0b8d, 69)
        ]
    );

    // The client reloads after the login 0x0091 and sends 0x007d again: inventory only.
    client.collect(Duration::from_millis(150)).await;
    client.send(&[0x7d, 0x00]).await;
    let again = client.collect(Duration::from_millis(300)).await;
    let again: Vec<u16> = again.iter().map(|f| f.opcode.0).collect();
    assert_eq!(
        again,
        [0x0b08, 0x0b0b, 0x0a9b, 0x00b0, 0x00b0],
        "{again:04x?}"
    );

    client.send(&[0x47, 0x04]).await;
    let confirm = client
        .wait_opcode(0x0b1b, LONG)
        .await
        .expect("ZC_LOAD_CONFIRM");
    assert!(confirm.payload.is_empty());
}
