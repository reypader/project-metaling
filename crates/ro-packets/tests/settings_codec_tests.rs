//! Map-entry settings packets, the status block and the load confirm (spec PKT-SET-01..04).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use ro_core::{ClientIntent, ServerEvent, SkillInfo, StatusWindow, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, is_implemented_outbound_opcode, lookup_packet_length, DecodedFrame, EventEncoder,
    ModernEventEncoder, PacketDecoder, PacketLength, PacketOpcode, RawPacket,
};
use tokio_util::codec::Decoder;

fn encode(event: ServerEvent) -> Vec<u8> {
    ModernEventEncoder
        .encode_event(&event, TARGET_PACKETVER)
        .expect("encodes")
        .to_vec()
}

#[test]
fn pkt_set_01_decodes_blocking_play_cancel() {
    let raw = RawPacket::new(PacketOpcode(0x0447), Bytes::new());
    let frame = decode_frame(&raw, TARGET_PACKETVER).expect("decodes");
    assert_eq!(
        frame,
        DecodedFrame::Intent(ClientIntent::BlockingPlayCancel)
    );
}

#[test]
fn pkt_set_02_encodes_settings_packets() {
    assert_eq!(encode(ServerEvent::FriendsList), [0x01, 0x02, 0x04, 0x00]);
    assert_eq!(
        encode(ServerEvent::SkillList { skills: Vec::new() }),
        [0x0f, 0x01, 0x04, 0x00]
    );

    let tab0 = encode(ServerEvent::ShortcutKeyList { tab: 0 });
    assert_eq!(tab0.len(), 271);
    assert_eq!(&tab0[..5], &[0x20, 0x0b, 0x00, 0x00, 0x00]);
    assert!(tab0[5..].iter().all(|b| *b == 0));

    // Capture proxy #92 and #93-96 (file lines 102-106; misframe corrected in settings.md).
    assert_eq!(
        encode(ServerEvent::PartyConfig {
            deny_invites: false
        }),
        [0xc9, 0x02, 0x00]
    );
    assert_eq!(
        encode(ServerEvent::EquipWindowConfig { show_equip: false }),
        [0xda, 0x02, 0x00]
    );
    assert_eq!(
        encode(ServerEvent::ConfigChanged {
            config_type: 2,
            value: 0
        }),
        [0xd9, 0x02, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    );

    // Capture proxy #97 (file line 107): 69 bytes, length 0x45, success 1.
    let repute = encode(ServerEvent::ReputationList);
    assert_eq!(repute.len(), 69);
    assert_eq!(&repute[..5], &[0x8d, 0x0b, 0x45, 0x00, 0x01]);

    assert_eq!(encode(ServerEvent::LoadConfirmed), [0x1b, 0x0b]);

    for op in [
        0x0201, 0x010f, 0x0b20, 0x02c9, 0x02da, 0x02d9, 0x0b8d, 0x0b1b,
    ] {
        assert!(
            is_implemented_outbound_opcode(PacketOpcode(op), TARGET_PACKETVER),
            "{op:#06x} should be marked implemented"
        );
    }
}

#[test]
fn pkt_set_03_table_lengths_frame_the_burst() {
    use PacketLength::{Dynamic, Fixed};
    let expected = [
        (0x010f, Dynamic),
        (0x0b20, Fixed(271)),
        (0x02c9, Fixed(3)),
        (0x02da, Fixed(3)),
        (0x02d9, Fixed(10)),
        (0x0b8d, Dynamic),
        (0x0b1b, Fixed(2)),
        (0x0b1d, Fixed(2)),
        (0x0acb, Fixed(12)),
        (0x00bd, Fixed(44)),
        (0x0141, Fixed(14)),
        (0x013a, Fixed(4)),
    ];
    for (op, length) in expected {
        assert_eq!(
            lookup_packet_length(PacketOpcode(op), TARGET_PACKETVER),
            Some(length),
            "{op:#06x}"
        );
    }

    let mut stream = BytesMut::new();
    for event in [
        ServerEvent::SkillList {
            skills: vec![nv_basic()],
        },
        ServerEvent::ShortcutKeyList { tab: 0 },
        ServerEvent::PartyConfig {
            deny_invites: false,
        },
        ServerEvent::EquipWindowConfig { show_equip: false },
        ServerEvent::ConfigChanged {
            config_type: 1,
            value: 0,
        },
        ServerEvent::ReputationList,
        ServerEvent::LoadConfirmed,
    ] {
        stream.extend_from_slice(&encode(event));
    }
    let mut decoder = PacketDecoder::default();
    let mut opcodes = Vec::new();
    while let Some(packet) = decoder.decode(&mut stream).expect("frames") {
        opcodes.push(packet.opcode.raw());
    }
    assert_eq!(
        opcodes,
        [0x010f, 0x0b20, 0x02c9, 0x02da, 0x02d9, 0x0b8d, 0x0b1b]
    );
    assert!(stream.is_empty());
}

fn nv_basic() -> SkillInfo {
    SkillInfo {
        id: 1,
        inf: 0,
        level: 0,
        sp: 0,
        range: 0,
        name: Arc::from("NV_BASIC"),
        upgradable: true,
    }
}

#[test]
fn pkt_set_04_encodes_skill_entry_and_status_block() {
    // rAthena capture packet_rathena_item_2026-10-24.txt file line 88 (proxy #78): `0f 01 29 00`,
    // then NV_BASIC with every number 0 and the name NUL-padded to 24 bytes.
    let skills = encode(ServerEvent::SkillList {
        skills: vec![nv_basic()],
    });
    assert_eq!(skills.len(), 41);
    assert_eq!(&skills[..6], &[0x0f, 0x01, 0x29, 0x00, 0x01, 0x00]);
    assert!(
        skills[6..16].iter().all(|b| *b == 0),
        "inf, level, sp, range"
    );
    assert_eq!(&skills[16..24], b"NV_BASIC");
    assert!(skills[24..40].iter().all(|b| *b == 0), "name padding");
    assert_eq!(skills[40], 1, "upgradable");

    assert_eq!(
        encode(ServerEvent::LongLongParameterChanged {
            var_id: 22,
            value: 99_999_999
        }),
        [0xcb, 0x0a, 0x16, 0x00, 0xff, 0xe0, 0xf5, 0x05, 0x00, 0x00, 0x00, 0x00]
    );

    let status = encode(ServerEvent::StatusWindow {
        status: StatusWindow {
            status_point: 48,
            stats: [1, 2, 3, 4, 5, 6],
            costs: [2, 2, 2, 2, 2, 2],
            hit: 178,
            ..StatusWindow::default()
        },
    });
    assert_eq!(status.len(), 44);
    assert_eq!(
        &status[..16],
        &[0xbd, 0x00, 0x30, 0x00, 1, 2, 2, 2, 3, 2, 4, 2, 5, 2, 6, 2]
    );
    assert_eq!(&status[32..34], &[0xb2, 0x00], "hit at offset 32");

    assert_eq!(
        encode(ServerEvent::CoupleStatus {
            var_id: 13,
            base: 9,
            bonus: 1
        }),
        [0x41, 0x01, 0x0d, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]
    );
    // rAthena capture file line 61 (proxy #51): `3a 01 01 00`.
    assert_eq!(
        encode(ServerEvent::AttackRange { range: 1 }),
        [0x3a, 0x01, 0x01, 0x00]
    );
    // rAthena capture file line 96 (proxy #86): `be 00 f7 00 00`.
    assert_eq!(
        encode(ServerEvent::StatusCost {
            var_id: 0xf7,
            value: 0
        }),
        [0xbe, 0x00, 0xf7, 0x00, 0x00]
    );

    for op in [0x0acb, 0x00bd, 0x0141, 0x013a, 0x00be] {
        assert!(
            is_implemented_outbound_opcode(PacketOpcode(op), TARGET_PACKETVER),
            "{op:#06x} should be marked implemented"
        );
    }
}
