//! Map-entry settings and the load confirm over TCP (spec `specs/map-entry-settings/spec.md`
//! BIN-SET-01).
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

    client.send(&[0x7d, 0x00]).await;
    let mut seen = Vec::new();
    for opcode in [0x010f, 0x0b20, 0x02c9, 0x02da, 0x02d9, 0x0b8d] {
        let frame = client
            .wait_opcode(opcode, LONG)
            .await
            .unwrap_or_else(|| panic!("{opcode:#06x} after ACTORINIT"));
        seen.push((opcode, frame_len(&frame)));
    }
    assert_eq!(
        seen,
        [
            (0x010f, 4),
            (0x0b20, 271),
            (0x02c9, 3),
            (0x02da, 3),
            (0x02d9, 10),
            (0x0b8d, 69)
        ]
    );

    client.send(&[0x47, 0x04]).await;
    let confirm = client
        .wait_opcode(0x0b1b, LONG)
        .await
        .expect("ZC_LOAD_CONFIRM");
    assert!(confirm.payload.is_empty());
}
