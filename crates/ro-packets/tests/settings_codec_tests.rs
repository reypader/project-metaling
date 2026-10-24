//! Map-entry settings packets and the load confirm (spec PKT-SET-01..03).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::{Bytes, BytesMut};
use ro_core::{ClientIntent, ServerEvent, TARGET_PACKETVER};
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
    assert_eq!(encode(ServerEvent::SkillList), [0x0f, 0x01, 0x04, 0x00]);

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
        ServerEvent::SkillList,
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
