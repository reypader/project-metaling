//! Engine-foundation packet scenarios PKT-03..PKT-13 (spec §3).
//! Offsets follow `reference/catalog/packets/map_units.md`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::{Bytes, BytesMut};
use ro_core::{
    ids::{AccountId, CharId, MapPosition, Sex},
    message::{ClientIntent, ServerEvent, UnitAppearance},
    TARGET_PACKETVER,
};
use ro_packets::{
    codec::PacketDecoder,
    coords::{pack_pos2, pack_pos_dir, unpack_pos2, unpack_pos_dir},
    incoming::Phase0IntentParser,
    outgoing::{is_implemented_outbound_opcode, ModernEventEncoder},
    table::RathenaPacketLengthTable,
    EventEncoder, PacketCodecError, PacketIntentParser, PacketOpcode, RawPacket,
};
use tokio_util::codec::Decoder;

const VER: u32 = TARGET_PACKETVER;

fn parse(opcode: u16, payload: &[u8]) -> Result<Option<ClientIntent>, PacketCodecError> {
    let raw = RawPacket::new(PacketOpcode(opcode), Bytes::copy_from_slice(payload));
    Phase0IntentParser.parse_intent(&raw, VER)
}

fn encode(event: &ServerEvent) -> Bytes {
    ModernEventEncoder
        .encode_event(event, VER)
        .unwrap_or_else(|err| panic!("{event:?} should encode: {err}"))
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}
fn i16_at(b: &[u8], off: usize) -> i16 {
    i16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn str_at(b: &[u8], off: usize, len: usize) -> String {
    let field = &b[off..off + len];
    let end = field.iter().position(|&c| c == 0).unwrap_or(len);
    String::from_utf8(field[..end].to_vec()).expect("utf8 name")
}

/// Asserts the frame decodes as exactly one packet of `opcode` through the length table.
fn assert_frames_cleanly(bytes: &Bytes, opcode: u16) {
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, VER);
    let mut buf = BytesMut::from(bytes.as_ref());
    let raw = decoder
        .decode(&mut buf)
        .expect("frame decodes")
        .expect("whole frame present");
    assert_eq!(raw.opcode, PacketOpcode(opcode));
    assert!(
        buf.is_empty(),
        "table length must equal encoded length for {opcode:#06x}"
    );
}

fn alice() -> UnitAppearance {
    UnitAppearance {
        gid: 2_000_001,
        char_id: Some(CharId(150_001)),
        name: "Alice".to_string(),
        job: 4001,
        sex: Sex::Female,
        hair_style: 7,
        hair_color: 3,
        head_dir: 2,
        pos: MapPosition {
            x: 100,
            y: 140,
            dir: 6,
        },
        speed: 112,
        dead_sit: 2,
        clevel: 42,
    }
}

// ---------- PKT-03 ----------

#[test]
fn pkt_03_request_move_decodes() {
    let payload = pack_pos_dir(104, 100, 0);
    assert_eq!(
        parse(0x035f, &payload).expect("decodes"),
        Some(ClientIntent::RequestMove {
            dest_x: 104,
            dest_y: 100
        })
    );
    let edge = pack_pos_dir(1023, 1023, 0);
    assert_eq!(
        parse(0x035f, &edge).expect("decodes"),
        Some(ClientIntent::RequestMove {
            dest_x: 1023,
            dest_y: 1023
        })
    );
}

#[test]
fn pkt_03_change_direction_decodes() {
    assert_eq!(
        parse(0x0361, &[2, 0, 4]).expect("decodes"),
        Some(ClientIntent::ChangeDirection {
            head_dir: 2,
            body_dir: 4
        })
    );
}

#[test]
fn pkt_03_change_direction_out_of_range_is_invalid_field() {
    for payload in [[3u8, 0, 4], [0, 0, 8], [0, 0, 255], [255, 0, 0]] {
        assert!(
            matches!(
                parse(0x0361, &payload),
                Err(PacketCodecError::InvalidField { .. })
            ),
            "{payload:?} must be rejected"
        );
    }
}

#[test]
fn pkt_03_change_direction_ignores_gap_byte() {
    // rAthena reads head_dir as one byte at offset 2; offset 3 is never read.
    assert_eq!(
        parse(0x0361, &[2, 0xab, 4]).expect("gap byte ignored"),
        Some(ClientIntent::ChangeDirection {
            head_dir: 2,
            body_dir: 4
        })
    );
}

#[test]
fn pkt_03_truncated_move_and_direction() {
    assert!(matches!(
        parse(0x035f, &[0, 0]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
    assert!(matches!(
        parse(0x0361, &[0, 0]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
}

// ---------- PKT-04 ----------

#[test]
fn pkt_04_request_act_decodes_and_passes_unknown_types() {
    let mut payload = 2_000_002u32.to_le_bytes().to_vec();
    payload.push(2);
    assert_eq!(
        parse(0x0437, &payload).expect("decodes"),
        Some(ClientIntent::RequestAction {
            target_id: 2_000_002,
            action_type: 2
        })
    );
    payload[4] = 99;
    assert_eq!(
        parse(0x0437, &payload).expect("decodes"),
        Some(ClientIntent::RequestAction {
            target_id: 2_000_002,
            action_type: 99
        })
    );
}

#[test]
fn pkt_04_reqname_decodes() {
    assert_eq!(
        parse(0x0368, &2_000_001u32.to_le_bytes()).expect("decodes"),
        Some(ClientIntent::RequestEntityName {
            entity_id: 2_000_001
        })
    );
}

#[test]
fn pkt_04_truncated_act_and_reqname() {
    assert!(matches!(
        parse(0x0437, &[1, 2, 3, 4]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
    assert!(matches!(
        parse(0x0368, &[1, 2, 3]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
}

// ---------- PKT-05/06 ----------

#[test]
fn pkt_05_wbufpos_vector_and_round_trip() {
    assert_eq!(pack_pos_dir(100, 140, 6), [0x19, 0x08, 0xc6]);
    for (x, y, d) in [(0, 0, 0), (1023, 1023, 7), (104, 100, 4)] {
        assert_eq!(unpack_pos_dir(&pack_pos_dir(x, y, d)), (x, y, d));
    }
}

#[test]
fn pkt_06_wbufpos2_round_trip() {
    for v in [
        (0, 0, 0, 0, 8, 8),
        (1023, 1023, 1023, 1023, 15, 15),
        (100, 100, 104, 100, 8, 8),
    ] {
        assert_eq!(unpack_pos2(&pack_pos2(v.0, v.1, v.2, v.3, v.4, v.5)), v);
    }
}

// ---------- PKT-07 ----------

#[test]
fn pkt_07_player_moved_encodes_0087() {
    let b = encode(&ServerEvent::PlayerMoved {
        start: (100, 100),
        dest: (104, 100),
        move_start_tick: 123_456,
    });
    assert_eq!(b.len(), 12);
    assert_eq!(u16_at(&b, 0), 0x0087);
    assert_eq!(u32_at(&b, 2), 123_456);
    let mv: [u8; 6] = b[6..12].try_into().unwrap();
    assert_eq!(unpack_pos2(&mv), (100, 100, 104, 100, 8, 8));
    assert_frames_cleanly(&b, 0x0087);
}

// ---------- PKT-08 ----------

#[test]
fn pkt_08_unit_walking_encodes_09fd() {
    let b = encode(&ServerEvent::UnitWalking {
        unit: alice(),
        start: (100, 100),
        dest: (100, 106),
        move_start_tick: 777,
    });
    assert_eq!(b.len(), 114);
    assert_eq!(u16_at(&b, 0), 0x09fd);
    assert_eq!(u16_at(&b, 2), 114);
    assert_eq!(b[4], 0, "object_type PC");
    assert_eq!(u32_at(&b, 5), 2_000_001, "AID = account id");
    assert_eq!(u32_at(&b, 9), 150_001, "GID = char id");
    assert_eq!(i16_at(&b, 13), 112, "speed");
    assert_eq!(i16_at(&b, 15), 0, "opt1 body_state stays 0");
    assert_eq!(i16_at(&b, 23), 4001, "job");
    assert_eq!(u16_at(&b, 25), 7, "hair style");
    assert_eq!(u32_at(&b, 37), 777, "moveStartTime");
    assert_eq!(i16_at(&b, 45), 3, "head palette = hair color");
    assert_eq!(i16_at(&b, 49), 2, "head dir");
    assert_eq!(b[66], 0, "sex female");
    let mv: [u8; 6] = b[67..73].try_into().unwrap();
    assert_eq!(unpack_pos2(&mv), (100, 100, 100, 106, 8, 8));
    assert_eq!((b[73], b[74]), (5, 5), "x/y size");
    assert_eq!(i16_at(&b, 75), 42, "clevel");
    assert_eq!(i32_at(&b, 79), -1, "max hp");
    assert_eq!(i32_at(&b, 83), -1, "hp");
    assert_eq!(str_at(&b, 90, 24), "Alice");
    assert_frames_cleanly(&b, 0x09fd);
}

#[test]
fn pkt_08_unit_walking_without_char_id_writes_gid_zero() {
    let mut unit = alice();
    unit.char_id = None;
    let b = encode(&ServerEvent::UnitWalking {
        unit,
        start: (1, 1),
        dest: (2, 2),
        move_start_tick: 0,
    });
    assert_eq!(u32_at(&b, 5), 2_000_001);
    assert_eq!(u32_at(&b, 9), 0);
}

// ---------- PKT-09 ----------

#[test]
fn pkt_09_unit_spawned_encodes_09fe() {
    let b = encode(&ServerEvent::UnitSpawned { unit: alice() });
    assert_eq!(b.len(), 107);
    assert_eq!(u16_at(&b, 0), 0x09fe);
    assert_eq!(u16_at(&b, 2), 107);
    assert_eq!(b[4], 0);
    assert_eq!(u32_at(&b, 5), 2_000_001);
    assert_eq!(u32_at(&b, 9), 150_001);
    assert_eq!(i16_at(&b, 13), 112);
    assert_eq!(i16_at(&b, 15), 0, "opt1 stays 0 even while sitting");
    assert_eq!(i16_at(&b, 23), 4001);
    assert_eq!(u16_at(&b, 25), 7);
    assert_eq!(i16_at(&b, 41), 3, "head palette");
    assert_eq!(i16_at(&b, 45), 2, "head dir");
    assert_eq!(b[62], 0, "sex");
    let pos: [u8; 3] = b[63..66].try_into().unwrap();
    assert_eq!(unpack_pos_dir(&pos), (100, 140, 6));
    assert_eq!((b[66], b[67]), (5, 5));
    assert_eq!(i16_at(&b, 68), 42, "clevel (no state byte in 09fe)");
    assert_eq!(i32_at(&b, 72), -1);
    assert_eq!(i32_at(&b, 76), -1);
    assert_eq!(str_at(&b, 83, 24), "Alice");
    assert_frames_cleanly(&b, 0x09fe);
}

#[test]
fn pkt_09_unit_standing_encodes_09ff_with_state() {
    let b = encode(&ServerEvent::UnitStanding { unit: alice() });
    assert_eq!(b.len(), 108);
    assert_eq!(u16_at(&b, 0), 0x09ff);
    assert_eq!(u16_at(&b, 2), 108);
    assert_eq!(u32_at(&b, 5), 2_000_001);
    assert_eq!(u32_at(&b, 9), 150_001);
    assert_eq!(i16_at(&b, 13), 112);
    assert_eq!(i16_at(&b, 15), 0);
    assert_eq!(i16_at(&b, 23), 4001);
    assert_eq!(u16_at(&b, 25), 7);
    assert_eq!(i16_at(&b, 41), 3);
    assert_eq!(i16_at(&b, 45), 2);
    assert_eq!(b[62], 0);
    let pos: [u8; 3] = b[63..66].try_into().unwrap();
    assert_eq!(unpack_pos_dir(&pos), (100, 140, 6));
    assert_eq!(b[68], 2, "state = dead_sit (sitting)");
    assert_eq!(i16_at(&b, 69), 42);
    assert_eq!(i32_at(&b, 73), -1);
    assert_eq!(i32_at(&b, 77), -1);
    assert_eq!(str_at(&b, 84, 24), "Alice");
    assert_frames_cleanly(&b, 0x09ff);
}

#[test]
fn pkt_09_male_sex_byte() {
    let mut unit = alice();
    unit.sex = Sex::Male;
    let b = encode(&ServerEvent::UnitStanding { unit });
    assert_eq!(b[62], 1);
}

// ---------- PKT-10 ----------

#[test]
fn pkt_10_unit_vanished_encodes_0080() {
    for vanish_type in 0..=3u8 {
        let b = encode(&ServerEvent::UnitVanished {
            gid: 2_000_001,
            vanish_type,
        });
        assert_eq!(b.len(), 7);
        assert_eq!(u16_at(&b, 0), 0x0080);
        assert_eq!(u32_at(&b, 2), 2_000_001);
        assert_eq!(b[6], vanish_type);
        assert_frames_cleanly(&b, 0x0080);
    }
}

// ---------- PKT-11 ----------

#[test]
fn pkt_11_ack_entity_name_encodes_0a30() {
    let b = encode(&ServerEvent::AckEntityName {
        gid: 2_000_001,
        name: "Alice".into(),
        party_name: "Party".into(),
        guild_name: "Guild".into(),
        position_name: "Leader".into(),
        title_id: -5,
    });
    assert_eq!(b.len(), 106);
    assert_eq!(u16_at(&b, 0), 0x0a30);
    assert_eq!(u32_at(&b, 2), 2_000_001);
    assert_eq!(str_at(&b, 6, 24), "Alice");
    assert_eq!(str_at(&b, 30, 24), "Party");
    assert_eq!(str_at(&b, 54, 24), "Guild");
    assert_eq!(str_at(&b, 78, 24), "Leader");
    assert_eq!(i32_at(&b, 102), -5);
    assert_frames_cleanly(&b, 0x0a30);
}

#[test]
fn pkt_11_overlong_strings_truncate_safely() {
    let long = "X".repeat(40);
    let b = encode(&ServerEvent::AckEntityName {
        gid: 1,
        name: long.clone(),
        party_name: long.clone(),
        guild_name: long.clone(),
        position_name: long.clone(),
        title_id: 0,
    });
    assert_eq!(b.len(), 106);
    assert_eq!(i32_at(&b, 102), 0, "title not overwritten by long strings");
    let m = encode(&ServerEvent::PlayerMapMove {
        dest_map: long,
        dest_x: 1,
        dest_y: 2,
    });
    assert_eq!(m.len(), 22);
    assert_eq!((u16_at(&m, 18), u16_at(&m, 20)), (1, 2));
}

#[test]
fn pkt_11_full_width_strings_keep_a_nul_terminator() {
    let exact = "Y".repeat(24);
    let b = encode(&ServerEvent::AckEntityName {
        gid: 1,
        name: exact.clone(),
        party_name: exact.clone(),
        guild_name: exact.clone(),
        position_name: exact,
        title_id: 0,
    });
    for end in [29, 53, 77, 101] {
        assert_eq!(b[end], 0, "byte {end} ends a 24-byte field with NUL");
    }
    assert_eq!(str_at(&b, 6, 24), "Y".repeat(23));
}

#[test]
fn pkt_11_player_map_move_encodes_0091() {
    let b = encode(&ServerEvent::PlayerMapMove {
        dest_map: "prontera.gat".into(),
        dest_x: 155,
        dest_y: 181,
    });
    assert_eq!(b.len(), 22);
    assert_eq!(u16_at(&b, 0), 0x0091);
    assert_eq!(str_at(&b, 2, 16), "prontera.gat");
    assert_eq!(u16_at(&b, 18), 155);
    assert_eq!(u16_at(&b, 20), 181);
    assert_frames_cleanly(&b, 0x0091);
}

// ---------- PKT-12 ----------

#[test]
fn pkt_12_accept_enter_par_change_time() {
    let b = encode(&ServerEvent::AcceptEnter {
        account_id: AccountId(2_000_001),
        server_tick: 5,
        pos: MapPosition {
            x: 100,
            y: 100,
            dir: 0,
        },
    });
    assert_eq!(b.len(), 23);
    assert_eq!(u16_at(&b, 0), 0x0283);

    let b = encode(&ServerEvent::ParameterChanged {
        var_id: 24,
        value: 500,
    });
    assert_eq!(b.len(), 8);
    assert_eq!(u16_at(&b, 0), 0x00b0);
    assert_eq!(u16_at(&b, 2), 24);
    assert_eq!(i32_at(&b, 4), 500);

    let b = encode(&ServerEvent::TimeSynchronized { server_tick: 99 });
    assert_eq!(b.len(), 6);
    assert_eq!(u16_at(&b, 0), 0x007f);
    assert_eq!(u32_at(&b, 2), 99);
}

#[test]
fn pkt_12_sit_stand_act_sets_only_src_and_type() {
    for action_type in [2u8, 3] {
        let b = encode(&ServerEvent::EntityAction {
            source_id: 2_000_001,
            target_id: 2_000_002,
            action_type,
            action_tick: 12_345,
        });
        assert_eq!(b.len(), 34);
        assert_eq!(u16_at(&b, 0), 0x08c8);
        assert_eq!(u32_at(&b, 2), 2_000_001);
        assert_eq!(b[29], action_type);
        for (i, &byte) in b.iter().enumerate() {
            if !(0..6).contains(&i) && i != 29 {
                assert_eq!(byte, 0, "byte {i} must be zero for sit/stand");
            }
        }
        assert_frames_cleanly(&b, 0x08c8);
    }
}

// ---------- PKT-13 ----------

#[test]
fn pkt_13_direction_changed_encodes_009c() {
    let b = encode(&ServerEvent::DirectionChanged {
        gid: 2_000_001,
        head_dir: 2,
        body_dir: 4,
    });
    assert_eq!(b.len(), 9);
    assert_eq!(u16_at(&b, 0), 0x009c);
    assert_eq!(u32_at(&b, 2), 2_000_001);
    assert_eq!(u16_at(&b, 6), 2);
    assert_eq!(b[8], 4);
    assert_frames_cleanly(&b, 0x009c);
}

#[test]
fn pkt_13_unit_stopped_encodes_0088() {
    let b = encode(&ServerEvent::UnitStopped {
        gid: 2_000_001,
        x: 101,
        y: 100,
    });
    assert_eq!(b.len(), 10);
    assert_eq!(u16_at(&b, 0), 0x0088);
    assert_eq!(u32_at(&b, 2), 2_000_001);
    assert_eq!(u16_at(&b, 6), 101);
    assert_eq!(u16_at(&b, 8), 100);
    assert_frames_cleanly(&b, 0x0088);
}

#[test]
fn pkt_13_new_opcodes_are_reported_implemented() {
    for op in [
        0x0087, 0x0088, 0x009c, 0x0080, 0x0091, 0x0a30, 0x09fd, 0x09fe, 0x09ff,
    ] {
        assert!(
            is_implemented_outbound_opcode(PacketOpcode(op), VER),
            "{op:#06x}"
        );
    }
    assert!(!is_implemented_outbound_opcode(PacketOpcode(0x0af7), VER));
    assert!(!is_implemented_outbound_opcode(PacketOpcode(0x0086), VER));
}
