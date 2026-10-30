//! Skill packets (spec `specs/skills/spec.md` PKT-SKILL-01). Bytes from
//! `reference/catalog/captures/rathena_skills_sequence.txt` (file lines).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;
use ro_core::{ClientIntent, ServerEvent, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, is_implemented_outbound_opcode, lookup_packet_length, DecodedFrame, EventEncoder,
    ModernEventEncoder, PacketLength, PacketOpcode, RawPacket,
};

fn encode(event: ServerEvent) -> Vec<u8> {
    ModernEventEncoder
        .encode_event(&event, TARGET_PACKETVER)
        .expect("encodes")
        .to_vec()
}

/// PKT-SKILL-01: `0x0112` parses into `UpgradeSkill`; `0x010e` (11 bytes) and `0x0441`
/// (4 bytes) encode as captured.
#[test]
fn pkt_skill_01_skill_packets() {
    // 697: `0x0112` payload `02 00` (SM_SWORD).
    let raw = RawPacket::new(PacketOpcode(0x0112), Bytes::from_static(&[0x02, 0x00]));
    assert_eq!(
        decode_frame(&raw, TARGET_PACKETVER).expect("decodes"),
        DecodedFrame::Intent(ClientIntent::UpgradeSkill { id: 2 })
    );
    let short = RawPacket::new(PacketOpcode(0x0112), Bytes::from_static(&[0x02]));
    assert!(decode_frame(&short, TARGET_PACKETVER).is_err());
    assert_eq!(
        lookup_packet_length(PacketOpcode(0x0112), TARGET_PACKETVER),
        Some(PacketLength::Fixed(4))
    );

    // 481: `0x010e` NV_BASIC lv 1, sp 0, range2 0, upFlag 1.
    assert_eq!(
        encode(ServerEvent::SkillUpdate {
            id: 1,
            level: 1,
            sp: 0,
            range: 0,
            upgradable: true,
        }),
        [0x0e, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]
    );
    // Bash lv 5: sp 8, range2 1.
    assert_eq!(
        encode(ServerEvent::SkillUpdate {
            id: 5,
            level: 5,
            sp: 8,
            range: 1,
            upgradable: true,
        }),
        [0x0e, 0x01, 0x05, 0x00, 0x05, 0x00, 0x08, 0x00, 0x01, 0x00, 0x01]
    );
    // `0x0441` SM_TWOHAND (3).
    assert_eq!(
        encode(ServerEvent::SkillDelete { id: 3 }),
        [0x41, 0x04, 0x03, 0x00]
    );
    for (op, len) in [(0x010e, 11), (0x0441, 4)] {
        assert_eq!(
            lookup_packet_length(PacketOpcode(op), TARGET_PACKETVER),
            Some(PacketLength::Fixed(len))
        );
        assert!(is_implemented_outbound_opcode(
            PacketOpcode(op),
            TARGET_PACKETVER
        ));
    }
}
