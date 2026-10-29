//! Traits through the map-server (spec `specs/traits/spec.md` BIN-TRAIT-01): a trait-up, a
//! logout, a relog: the traits, the trait-point pool and AP come back. The Postgres side of the
//! save is `ro-persistence`'s PER-TRAIT-01.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;
use ro_core::{CharacterProgress, CharacterStatus};
use ro_packets::RawPacket;

const A: u32 = 1;
const LONG: Duration = Duration::from_secs(2);
const DRAGON_KNIGHT: u16 = 4252;
const SP_POW: u16 = 219;
const SP_TRAITPOINT: u16 = 231;
const SP_AP: u16 = 232;
const SP_MAXAP: u16 = 233;

/// `CZ_ADVANCED_STATUS_CHANGE`: raise trait `var` by `amount`.
fn raise_trait(var: u16, amount: i16) -> Vec<u8> {
    let mut frame = vec![0x24, 0x0b];
    frame.extend_from_slice(&var.to_le_bytes());
    frame.extend_from_slice(&amount.to_le_bytes());
    frame
}

/// The value of the first `0x00b0` for `var` in `frames`.
fn param(frames: &[RawPacket], var: u16) -> Option<u32> {
    frames
        .iter()
        .find(|f| f.opcode.0 == 0x00b0 && frame_u16(f, 2) == var)
        .map(|f| frame_u32(f, 4))
}

/// BIN-TRAIT-01: a DK at B200 with stored AP raises POW by 2, logs out and back in: POW 2, the
/// pool and AP are saved and come back in the login block.
#[tokio::test]
async fn bin_trait_01_trait_up_survives_relog() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_progress(
        A,
        CharacterProgress {
            job_class: DRAGON_KNIGHT,
            base_level: 200,
            status: CharacterStatus {
                trait_point: 7,
                ap: 150,
                ..CharacterStatus::default()
            },
        },
    );
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, _) = enter(&server, A).await;
    a.send(&raise_trait(SP_POW, 2)).await;
    let ack = a.wait_opcode(0x00bc, LONG).await.expect("trait-up ack");
    assert_eq!(&ack.payload[..], &[0xdb, 0x00, 0x01, 0x02], "POW 0 -> 2");
    a.close().await;
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo
            .stored_progress(A)
            .is_some_and(|p| p.status.traits[0] == 2
                && p.status.trait_point == 5
                && p.status.ap == 150))
        .await,
        "saved on logout: {:?}",
        repo.stored_progress(A)
    );

    let (_a, frames) = enter(&server, A).await;
    let pow = frames
        .iter()
        .find(|f| f.opcode.0 == 0x0141 && frame_u32(f, 2) == u32::from(SP_POW))
        .expect("POW couple on login");
    assert_eq!(frame_u32(pow, 6), 2, "allocated POW");
    assert_eq!(param(&frames, SP_TRAITPOINT), Some(5));
    assert_eq!(param(&frames, SP_AP), Some(150));
    assert_eq!(param(&frames, SP_MAXAP), Some(200));
}
