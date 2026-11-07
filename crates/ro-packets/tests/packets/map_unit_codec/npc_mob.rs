//! NPC and mob entry packets (PKT-NPC-01, 02, PKT-MOB-01).

use super::*;

#[test]
fn pkt_npc_01_npc_standentry_fields() {
    let b = encode(&ServerEvent::UnitStanding { unit: kafra() });
    assert_eq!(b.len(), 108);
    assert_eq!(b[4], 0x06, "object_type NPC");
    assert_eq!(u32_at(&b, 5), 110_000_005, "aid = NPC id");
    assert_eq!(u32_at(&b, 9), 0, "GID 0 for NPCs");
    assert_eq!(i16_at(&b, 13), 200);
    assert_eq!(i32_at(&b, 19), 0x4, "effect_state");
    assert_eq!(i16_at(&b, 23), 117, "job = sprite");
    assert_eq!(u16_at(&b, 25), 0, "head");
    assert_eq!(b[62], 0, "sex");
    let pos: [u8; 3] = b[63..66].try_into().unwrap();
    assert_eq!(unpack_pos_dir(&pos), (152, 326, 4));
    assert_eq!((b[66], b[67]), (0, 0), "x_size/y_size 0 for NPCs");
    assert_eq!(b[68], 0, "state");
    assert_eq!(i16_at(&b, 69), 0, "clevel");
    assert_eq!((i32_at(&b, 73), i32_at(&b, 77)), (-1, -1));
    assert_eq!(
        str_at(&b, 84, 24),
        "Kafra Employee#prt1_wit",
        "23 bytes + NUL"
    );
    assert_eq!(b[107], 0);
    assert_frames_cleanly(&b, 0x09ff);
}

#[test]
fn pkt_npc_01_mob_sprite_npc_and_player_sizes() {
    let mut unit = kafra();
    unit.object_type = UnitObjectType::MobSpriteNpc;
    let b = encode(&ServerEvent::UnitStanding { unit });
    assert_eq!(b[4], 0x0C);

    let p = encode(&ServerEvent::UnitStanding { unit: alice() });
    assert_eq!((p[4], p[66], p[67]), (0, 5, 5), "players unchanged");
}

#[test]
fn pkt_npc_01_raw_dir_is_masked_to_four_bits() {
    let mut unit = kafra();
    unit.pos.dir = 55;
    let b = encode(&ServerEvent::UnitStanding { unit });
    let pos: [u8; 3] = b[63..66].try_into().unwrap();
    assert_eq!(unpack_pos_dir(&pos), (152, 326, 7));
}

#[test]
fn pkt_npc_02_ack_npc_name_encodes_0adf() {
    let b = encode(&ServerEvent::AckNpcName {
        gid: 110_000_005,
        name: "Kafra Employee#prt1".into(),
    });
    assert_eq!(b.len(), 58);
    assert_eq!(u16_at(&b, 0), 0x0adf);
    assert_eq!(u32_at(&b, 2), 110_000_005);
    assert_eq!(i32_at(&b, 6), 0, "group_id");
    assert_eq!(str_at(&b, 10, 24), "Kafra Employee#prt1");
    assert!(b[34..58].iter().all(|&x| x == 0), "title empty");
    assert_frames_cleanly(&b, 0x0adf);
    assert!(is_implemented_outbound_opcode(PacketOpcode(0x0adf), VER));
}

/// PKT-MOB-01: catalog `mechanics/mobs.md` §1.5 values in all three entry packets.
#[test]
fn pkt_mob_01_mob_entry_fields() {
    let unit = poring();
    let cases = [
        (
            encode(&ServerEvent::UnitStanding { unit: unit.clone() }),
            0x09ff,
            0,
        ),
        (
            encode(&ServerEvent::UnitSpawned { unit: unit.clone() }),
            0x09fe,
            -1,
        ),
    ];
    for (b, opcode, shift) in cases {
        // 0x09fe lacks the 0x09ff `state` byte, so its tail sits one byte earlier.
        let tail = |off: usize| off.wrapping_add_signed(shift);
        assert_eq!(b[4], 0x05, "{opcode:#06x}: object_type mob");
        assert_eq!(u32_at(&b, 5), 110_000_042, "aid = mob gid");
        assert_eq!(u32_at(&b, 9), 0, "GID 0 for mobs");
        assert_eq!(i16_at(&b, 13), 150, "speed");
        assert_eq!(u16_at(&b, 23), 1002, "job = mob id");
        assert_eq!(u16_at(&b, 25), 0, "head");
        assert_eq!(
            (i16_at(&b, 41), i16_at(&b, 43)),
            (0, 0),
            "head/body palette"
        );
        assert_eq!(b[62], 0, "sex");
        assert_eq!((b[66], b[67]), (0, 0), "x_size/y_size 0");
        assert_eq!(i16_at(&b, tail(69)), 5, "clevel");
        assert_eq!(
            (i32_at(&b, tail(73)), i32_at(&b, tail(77))),
            (-1, -1),
            "full HP"
        );
        assert_eq!(b[tail(81)], 2, "is_boss: MVP");
        assert_eq!(
            str_at(&b, tail(84), 24),
            "Elusive Wraith Dead (Ni",
            "23 bytes + NUL"
        );
        assert_frames_cleanly(&b, opcode);
    }

    let walking = encode(&ServerEvent::UnitWalking {
        unit,
        start: (100, 200),
        dest: (101, 201),
        move_start_tick: 0,
    });
    assert_eq!(walking[4], 0x05);
    assert_frames_cleanly(&walking, 0x09fd);
}

#[test]
fn pkt_mob_01_npc_and_player_bytes_unchanged() {
    let n = encode(&ServerEvent::UnitStanding { unit: kafra() });
    assert_eq!((n[4], n[66], n[67]), (0x06, 0, 0));
    let p = encode(&ServerEvent::UnitStanding { unit: alice() });
    assert_eq!((p[4], p[66], p[67]), (0, 5, 5));
}
