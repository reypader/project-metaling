//! Skills and potions through the map-server (spec `specs/effects/spec.md` BIN-EFF-01): First
//! Aid, Bash on a spawned mob and a potion over TCP, with the shipped data and the built-in
//! definitions, in the §2 orders on the caster's and an observer's connection.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::*;
use ro_core::{CharacterProgress, CharacterStatus, LearnedSkill, SkillFlag};
use ro_db::{MapMobSpawns, MapNpcs, MobId, MobSpawn};
use ro_packets::RawPacket;

/// The caster and an observer.
const A: u32 = 1;
const B: u32 = 2;
/// The first mob gid on a map.
const MOB_GID: u32 = 110_000_000;
const SWORDMAN: u16 = 1;
const NV_BASIC: u16 = 1;
const SM_BASH: u16 = 5;
const NV_FIRSTAID: u16 = 142;
const RED_POTION: u32 = 501;
const SP_HP: u16 = 5;
const SP_SP: u16 = 7;
const SP_WEIGHT: u16 = 24;
const WINDOW: Duration = Duration::from_millis(300);
/// The observer reads after the caster's window, so its frames are already in flight.
const OBSERVER: Duration = Duration::from_millis(100);
/// Longer than any `canact` gate a cast here sets.
const GATE: Duration = Duration::from_millis(1200);

/// One shipped Poring next to A at (100, 100).
fn poring_loader() -> map_server::MapDataLoader {
    let spawn = MobSpawn {
        name: "Poring".into(),
        mob_id: MobId(1002),
        x: 101,
        y: 100,
        span: (0, 0),
        amount: 1,
        delay1: 5000,
        delay2: 0,
        level: None,
        boss: false,
        event: None,
        source: "test:poring".into(),
    };
    data_loader(
        &[],
        MapNpcs::default(),
        MapMobSpawns {
            spawns: vec![spawn],
        },
    )
}

fn row(id: u16, level: u8) -> LearnedSkill {
    LearnedSkill {
        id,
        level,
        flag: SkillFlag::Permanent,
    }
}

/// A: a base-50 Swordman at 10 HP with First Aid, Bash 10 and five Red Potions in slot 0.
/// B: a Novice two cells away.
fn seeded_repo() -> InMemoryRepo {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100), (B, 102, 101)]);
    repo.set_progress(
        A,
        CharacterProgress {
            job_class: SWORDMAN,
            base_level: 50,
            status: CharacterStatus {
                job_level: 10,
                hp: 10,
                // Clamped to the max at spawn.
                sp: 9999,
                ..CharacterStatus::default()
            },
            change_level: [0; 3],
        },
    );
    repo.set_skills(
        A,
        vec![row(NV_BASIC, 9), row(SM_BASH, 10), row(NV_FIRSTAID, 1)],
    );
    repo.set_inventory(A, inventory(&[(RED_POTION, 5)]));
    repo
}

fn use_skill(level: u16, skill: u16, target: u32) -> Vec<u8> {
    let mut b = vec![0x38, 0x04];
    b.extend_from_slice(&level.to_le_bytes());
    b.extend_from_slice(&skill.to_le_bytes());
    b.extend_from_slice(&target.to_le_bytes());
    b
}

fn use_item(index: u16, aid: u32) -> Vec<u8> {
    let mut b = vec![0x39, 0x04];
    b.extend_from_slice(&index.to_le_bytes());
    b.extend_from_slice(&aid.to_le_bytes());
    b
}

/// What a frame says, for order checks: the opcode, and the var of an `0x00b0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    Op(u16),
    Param(u16),
}

/// The frames of `ops` (and the `0x00b0` vars in `vars`) in arrival order.
fn seen(frames: &[RawPacket], ops: &[u16], vars: &[u16]) -> Vec<Seen> {
    frames
        .iter()
        .filter_map(|f| match f.opcode.0 {
            0x00b0 if vars.contains(&frame_u16(f, 2)) => Some(Seen::Param(frame_u16(f, 2))),
            op if ops.contains(&op) => Some(Seen::Op(op)),
            _ => None,
        })
        .collect()
}

fn find(frames: &[RawPacket], op: u16) -> &RawPacket {
    frames
        .iter()
        .find(|f| f.opcode.0 == op)
        .unwrap_or_else(|| panic!("{op:#06x} in {:04x?}", ops(frames)))
}

/// BIN-EFF-01: First Aid, Bash on the Poring and a Red Potion over TCP. The caster sees the
/// casts' `0x0b1a`, the SP push and the effect (`0x09cb` then `SP_HP`; `0x01de`), and the
/// potion's `0x01c8`, `SP_WEIGHT`, `SP_HP`. The observer sees `0x0b1a`, the effect and the
/// potion's ack.
#[tokio::test]
async fn bin_eff_01_first_aid_bash_and_potion_over_tcp() {
    let server = TestServer::start_with_loader(seeded_repo(), poring_loader()).await;
    let (mut a, _) = enter(&server, A).await;
    let (mut b, _) = enter(&server, B).await;
    a.collect(WINDOW).await;
    let (aid_a, ops_a, vars_a) = (account(A), [0x0b1a, 0x09cb, 0x01de, 0x01c8], [SP_HP, SP_SP]);

    // First Aid (a self skill: the target id is ignored).
    a.send(&use_skill(1, NV_FIRSTAID, 0)).await;
    let (to_a, to_b) = (a.collect(WINDOW).await, b.collect(OBSERVER).await);
    assert_eq!(
        seen(&to_a, &ops_a, &vars_a),
        [
            Seen::Op(0x0b1a),
            Seen::Param(SP_SP),
            Seen::Op(0x09cb),
            Seen::Param(SP_HP),
        ],
        "{:04x?}",
        ops(&to_a)
    );
    let heal = find(&to_a, 0x09cb);
    assert_eq!(
        (frame_u16(heal, 2), frame_u32(heal, 4), frame_u32(heal, 8)),
        (NV_FIRSTAID, 5, aid_a)
    );
    assert_eq!(
        seen(&to_b, &ops_a, &vars_a),
        [Seen::Op(0x0b1a), Seen::Op(0x09cb)],
        "{:04x?}",
        ops(&to_b)
    );

    // Bash 10 on the Poring, once First Aid's gate has passed.
    tokio::time::sleep(GATE).await;
    a.send(&use_skill(10, SM_BASH, MOB_GID)).await;
    let (to_a, to_b) = (a.collect(WINDOW).await, b.collect(OBSERVER).await);
    let bash = [Seen::Op(0x0b1a), Seen::Param(SP_SP), Seen::Op(0x01de)];
    assert_eq!(
        seen(&to_a, &ops_a, &[SP_SP])[..3],
        bash,
        "{:04x?}",
        ops(&to_a)
    );
    let cast = find(&to_a, 0x0b1a);
    assert_eq!(
        (frame_u32(cast, 2), frame_u32(cast, 6), frame_u16(cast, 14)),
        (aid_a, MOB_GID, SM_BASH)
    );
    let hit = find(&to_a, 0x01de);
    assert_eq!(
        (frame_u16(hit, 2), frame_u32(hit, 4), frame_u32(hit, 8)),
        (SM_BASH, aid_a, MOB_GID)
    );
    assert_eq!(
        seen(&to_b, &ops_a, &vars_a)[..2],
        [Seen::Op(0x0b1a), Seen::Op(0x01de)],
        "{:04x?}",
        ops(&to_b)
    );

    // A Red Potion from slot 0 (wire index 2).
    a.send(&use_item(2, aid_a)).await;
    let (to_a, to_b) = (a.collect(WINDOW).await, b.collect(OBSERVER).await);
    assert_eq!(
        seen(&to_a, &[0x01c8], &[SP_WEIGHT, SP_HP]),
        [Seen::Op(0x01c8), Seen::Param(SP_WEIGHT), Seen::Param(SP_HP),],
        "{:04x?}",
        ops(&to_a)
    );
    let ack = find(&to_a, 0x01c8);
    assert_eq!(
        (
            frame_u16(ack, 2),
            frame_u32(ack, 4),
            frame_u32(ack, 8),
            frame_u16(ack, 12),
            frame_u8(ack, 14),
        ),
        (2, RED_POTION, aid_a, 4, 1)
    );
    assert_eq!(
        seen(&to_b, &[0x01c8], &[]),
        [Seen::Op(0x01c8)],
        "{:04x?}",
        ops(&to_b)
    );
    assert_eq!(find(&to_b, 0x01c8).payload, ack.payload, "the same ack");
}
