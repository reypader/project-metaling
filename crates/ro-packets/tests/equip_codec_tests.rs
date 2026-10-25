//! Equipment packets (spec `specs/equipment/spec.md` PKT-EQ-02..07). Byte vectors come from the
//! captures in `reference/catalog/packets/equipment.md` ("Captured weapon swap", "Captured equip
//! sequences").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use ro_core::{
    AccountId, CharId, CharacterLooks, CharacterSummary, ClientIntent, MapPosition, ServerEvent,
    Sex, UnitAppearance, UnitLooks, UnitObjectType, TARGET_PACKETVER,
};
use ro_packets::{
    decode_frame, encode_character_info, is_implemented_outbound_opcode, DecodedFrame,
    EventEncoder, ModernEventEncoder, PacketCodecError, PacketOpcode, RawPacket,
};

fn encode(event: ServerEvent) -> Vec<u8> {
    ModernEventEncoder
        .encode_event(&event, TARGET_PACKETVER)
        .expect("encodes")
        .to_vec()
}

fn decode(op: u16, payload: &[u8]) -> Result<DecodedFrame, PacketCodecError> {
    let raw = RawPacket::new(PacketOpcode(op), Bytes::copy_from_slice(payload));
    decode_frame(&raw, TARGET_PACKETVER)
}

fn intent(op: u16, payload: &[u8]) -> ClientIntent {
    match decode(op, payload).expect("decodes") {
        DecodedFrame::Intent(intent) => intent,
        other => panic!("{op:#06x} not decoded: {other:?}"),
    }
}

/// `opcode` little-endian, then `payload`.
fn frame(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = opcode.to_le_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// PKT-EQ-02: `0x0998`, `0x00ab` and `0x0bf5` decode with the wire index kept.
#[test]
fn pkt_eq_02_decodes_equip_requests() {
    assert_eq!(
        intent(0x0998, &[0x06, 0x00, 0x02, 0x00, 0x00, 0x00]),
        ClientIntent::EquipItem {
            index: 6,
            position: 0x2
        },
        "captured Knife equip"
    );
    assert_eq!(
        intent(0x0998, &[0x1b, 0x00, 0x00, 0x80, 0x00, 0x00]),
        ClientIntent::EquipItem {
            index: 0x1b,
            position: 0x8000
        },
        "captured arrow equip"
    );
    assert_eq!(
        intent(0x00ab, &[0x1a, 0x00]),
        ClientIntent::UnequipItem { index: 0x1a }
    );
    assert_eq!(
        intent(0x0bf5, &[0xff, 0x83, 0x00, 0x00]),
        ClientIntent::UnequipAll { location: 0x83ff },
        "captured unequip-all button"
    );
    for (op, short) in [
        (0x0998, &[0x06, 0x00, 0x02][..]),
        (0x00ab, &[0x1a][..]),
        (0x0bf5, &[0xff, 0x83][..]),
    ] {
        assert!(
            matches!(
                decode(op, short),
                Err(PacketCodecError::TruncatedPayload { .. })
            ),
            "{op:#06x} truncated"
        );
    }
}

/// PKT-EQ-03: `0x0999` and `0x099a` encode the captured bytes.
#[test]
fn pkt_eq_03_encodes_equip_acks() {
    let ack = |index, location, view, result| {
        encode(ServerEvent::EquipAck {
            index,
            location,
            view,
            result,
        })
    };
    assert_eq!(
        ack(6, 0x2, 0, 0),
        frame(
            0x0999,
            &[0x06, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        ),
        "Knife on"
    );
    assert_eq!(
        ack(0x16, 0, 0, 1),
        frame(
            0x0999,
            &[0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]
        ),
        "Gakkung, level too low"
    );
    assert_eq!(
        ack(0x18, 0x100, 16, 0),
        frame(
            0x0999,
            &[0x18, 0x00, 0x00, 0x01, 0x00, 0x00, 0x10, 0x00, 0x00]
        ),
        "Hat on, view 16"
    );
    let off = |index, location, failed| {
        encode(ServerEvent::UnequipAck {
            index,
            location,
            failed,
        })
    };
    assert_eq!(
        off(2, 0x2, false),
        frame(0x099a, &[0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00]),
        "Knife off"
    );
    assert_eq!(
        off(0x1b, 0x8000, false),
        frame(0x099a, &[0x1b, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00]),
        "arrows off (cascade)"
    );
    assert_eq!(
        off(2, 0, true),
        frame(0x099a, &[0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
        "take-off failure"
    );
}

/// PKT-EQ-04: `0x0bae`, `0x013c`, `0x013b`, `0x0291` and `0x0983` layouts.
#[test]
fn pkt_eq_04_encodes_ammo_and_message_packets() {
    assert_eq!(
        encode(ServerEvent::UnequipAllAck { failed: false }),
        frame(0x0bae, &[0x00])
    );
    assert_eq!(
        encode(ServerEvent::UnequipAllAck { failed: true }),
        frame(0x0bae, &[0x01])
    );
    assert_eq!(
        encode(ServerEvent::AmmoEquipped { index: 0x1b }),
        frame(0x013c, &[0x1b, 0x00])
    );
    assert_eq!(
        encode(ServerEvent::ActionFailure { failure_type: 3 }),
        frame(0x013b, &[0x03, 0x00])
    );
    assert_eq!(
        encode(ServerEvent::ClientMessage { msg_id: 1691 }),
        frame(0x0291, &[0x9b, 0x06]),
        "MSI_FAIL_NEED_EQUIPPED_BOW"
    );
    let icon = encode(ServerEvent::StatusIconStarted {
        efst: 695,
        gid: 2_000_000,
        state: 1,
        total_ms: 9999,
        remain_ms: 9999,
        values: [0; 3],
    });
    let mut captured = vec![
        0xb7, 0x02, 0x80, 0x84, 0x1e, 0x00, 0x01, 0x0f, 0x27, 0x00, 0x00, 0x0f, 0x27, 0x00, 0x00,
    ];
    captured.extend([0u8; 12]);
    assert_eq!(icon, frame(0x0983, &captured), "captured ammo icon");
    assert_eq!(icon.len(), 29);

    for op in [0x0999, 0x099a, 0x0bae, 0x013c, 0x013b, 0x0291, 0x0983] {
        assert!(
            is_implemented_outbound_opcode(PacketOpcode(op), TARGET_PACKETVER),
            "{op:#06x}"
        );
    }
}

/// PKT-EQ-05: `0x01d7` for shoes, headgear and the robe: 15 bytes, `val2` 0.
#[test]
fn pkt_eq_05_encodes_look_changes() {
    let look = |look_type, value| {
        encode(ServerEvent::SpriteChanged {
            gid: 2_000_000,
            look_type,
            value,
            value2: 0,
        })
    };
    let aid = [0x80, 0x84, 0x1e, 0x00];
    let expect = |rest: &[u8]| {
        let mut payload = aid.to_vec();
        payload.extend_from_slice(rest);
        frame(0x01d7, &payload)
    };
    assert_eq!(
        look(9, 2403),
        expect(&[0x09, 0x63, 0x09, 0, 0, 0, 0, 0, 0]),
        "LOOK_SHOES with the item id"
    );
    assert_eq!(
        look(4, 16),
        expect(&[0x04, 0x10, 0, 0, 0, 0, 0, 0, 0]),
        "LOOK_HEAD_TOP"
    );
    assert_eq!(look(3, 5)[6], 3, "LOOK_HEAD_BOTTOM");
    assert_eq!(look(5, 7)[6], 5, "LOOK_HEAD_MID");
    assert_eq!(
        look(12, 2),
        expect(&[0x0c, 0x02, 0, 0, 0, 0, 0, 0, 0]),
        "LOOK_ROBE"
    );
    assert_eq!(look(12, 2).len(), 15);
}

fn wearer(looks: UnitLooks) -> UnitAppearance {
    UnitAppearance {
        object_type: UnitObjectType::Pc,
        gid: 2_000_000,
        char_id: Some(CharId(150_000)),
        name: Arc::from("test"),
        job: 3,
        sex: Sex::Female,
        hair_style: 1,
        hair_color: 0,
        head_dir: 0,
        pos: MapPosition {
            x: 150,
            y: 150,
            dir: 0,
        },
        speed: 150,
        dead_sit: 0,
        clevel: 10,
        effect_state: 0,
        looks,
    }
}

const LOOKS: UnitLooks = UnitLooks {
    weapon: 1201,
    shield: 2101,
    head_bottom: 3,
    head_top: 16,
    head_mid: 7,
    robe: 2,
};

/// PKT-EQ-06: the entry packets put `UnitLooks` at the weapon / shield / accessory / robe offsets
/// (catalog `map_units.md`).
#[test]
fn pkt_eq_06_entry_packets_carry_looks() {
    let unit = wearer(LOOKS);
    let check = |bytes: &[u8], acc2: usize, acc3: usize, robe: usize, name: &str| {
        assert_eq!(u32_at(bytes, 27), 1201, "{name} weapon");
        assert_eq!(u32_at(bytes, 31), 2101, "{name} shield");
        assert_eq!(u16_at(bytes, 35), 3, "{name} accessory (head bottom)");
        assert_eq!(u16_at(bytes, acc2), 16, "{name} accessory2 (head top)");
        assert_eq!(u16_at(bytes, acc3), 7, "{name} accessory3 (head mid)");
        assert_eq!(u16_at(bytes, robe), 2, "{name} robe");
    };
    let standing = encode(ServerEvent::UnitStanding { unit: unit.clone() });
    check(&standing, 37, 39, 47, "0x09ff");
    let spawned = encode(ServerEvent::UnitSpawned { unit: unit.clone() });
    check(&spawned, 37, 39, 47, "0x09fe");
    let walking = encode(ServerEvent::UnitWalking {
        unit,
        start: (150, 150),
        dest: (151, 150),
        move_start_tick: 0,
    });
    check(&walking, 41, 43, 51, "0x09fd");

    // Captured (equip_sequence_2.txt line 222): a bow and Goggles show weapon 1701, shield 0,
    // head top 1, head mid 0.
    let bow = encode(ServerEvent::UnitSpawned {
        unit: wearer(UnitLooks {
            weapon: 1701,
            head_top: 1,
            ..UnitLooks::default()
        }),
    });
    assert_eq!(
        &bow[27..41],
        &[0xa5, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x00, 0x00, 0x00]
    );
}

fn summary(looks: CharacterLooks) -> CharacterSummary {
    CharacterSummary {
        char_id: CharId(150_000),
        account_id: AccountId(2_000_000),
        slot: 0,
        name: "test".into(),
        sex: Sex::Female,
        job_class: 3,
        base_level: 10,
        job_level: 10,
        hair_style: 1,
        hair_color: 0,
        last_map: "prt_fild08".into(),
        last_pos: MapPosition {
            x: 150,
            y: 150,
            dir: 0,
        },
        delete_date: None,
        looks,
    }
}

/// PKT-EQ-07: the char-select entry puts `CharacterLooks` at offsets 90/96/98/100/102/162
/// (catalog `char.md`).
#[test]
fn pkt_eq_07_char_entry_carries_looks() {
    let mut buf = BytesMut::new();
    encode_character_info(
        &mut buf,
        &summary(CharacterLooks {
            weapon_type: 1,
            shield: 4,
            head_top: 16,
            head_mid: 7,
            head_bottom: 3,
            robe: 2,
        }),
    );
    let entry = buf.to_vec();
    assert_eq!(entry.len(), 175);
    assert_eq!(u16_at(&entry, 90), 1, "weapon = W_DAGGER, the type");
    assert_eq!(u16_at(&entry, 96), 3, "accessory = head bottom");
    assert_eq!(u16_at(&entry, 98), 4, "shield view");
    assert_eq!(u16_at(&entry, 100), 16, "accessory2 = head top");
    assert_eq!(u16_at(&entry, 102), 7, "accessory3 = head mid");
    assert_eq!(u32_at(&entry, 162), 2, "robePalette = robe view");

    let mut bare = BytesMut::new();
    encode_character_info(&mut bare, &summary(CharacterLooks::default()));
    for off in [90, 96, 98, 100, 102] {
        assert_eq!(u16_at(&bare, off), 0, "offset {off}");
    }
    assert_eq!(u32_at(&bare, 162), 0);
}

/// PKT-EQ-08: a player's `body` is its job id, in the char-select entry (offset 88) and in all
/// three entry packets (just before `name`); the client draws the body sprite from it. Every
/// capture agrees (Novice 0, Archer 3, Assassin 12, Bard 19, Dancer 20). Non-players send 0.
#[test]
fn pkt_eq_08_body_is_the_job() {
    let mut buf = BytesMut::new();
    encode_character_info(&mut buf, &summary(CharacterLooks::default()));
    assert_eq!(u16_at(&buf, 84), 3, "job");
    assert_eq!(u16_at(&buf, 88), 3, "body = job");

    let body = |bytes: &[u8]| u16_at(bytes, bytes.len() - 26);
    let unit = wearer(UnitLooks::default());
    let standing = encode(ServerEvent::UnitStanding { unit: unit.clone() });
    let spawned = encode(ServerEvent::UnitSpawned { unit: unit.clone() });
    let walking = encode(ServerEvent::UnitWalking {
        unit: unit.clone(),
        start: (150, 150),
        dest: (151, 150),
        move_start_tick: 0,
    });
    assert_eq!(
        [body(&standing), body(&spawned), body(&walking)],
        [3, 3, 3],
        "0x09ff, 0x09fe, 0x09fd"
    );

    let npc = UnitAppearance {
        object_type: UnitObjectType::Npc,
        char_id: None,
        job: 45,
        ..unit
    };
    assert_eq!(
        body(&encode(ServerEvent::UnitStanding { unit: npc })),
        0,
        "NPC"
    );
}
