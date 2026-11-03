//! Skill-cast and item-use packets (spec `specs/effects/spec.md` PKT-EFF-01/02). Bytes from
//! `reference/catalog/captures/scripting_1.txt` (file lines).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;
use ro_core::{ClientIntent, ServerEvent, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, is_implemented_outbound_opcode, lookup_packet_length, DecodedFrame, EventEncoder,
    ModernEventEncoder, PacketLength, PacketOpcode, RawPacket,
};

const A: u32 = 0x001e_8480;

fn encode(event: ServerEvent) -> Vec<u8> {
    ModernEventEncoder
        .encode_event(&event, TARGET_PACKETVER)
        .expect("encodes")
        .to_vec()
}

/// Opcode (little-endian) followed by the captured payload bytes.
fn frame(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let mut v = opcode.to_le_bytes().to_vec();
    v.extend_from_slice(payload);
    v
}

fn decode(opcode: u16, payload: &'static [u8]) -> ClientIntent {
    let raw = RawPacket::new(PacketOpcode(opcode), Bytes::from_static(payload));
    match decode_frame(&raw, TARGET_PACKETVER).expect("decodes") {
        DecodedFrame::Intent(intent) => intent,
        other => panic!("expected an intent, got {other:?}"),
    }
}

/// PKT-EFF-01: `0x0438` and `0x0439` decode at the §2.2/§2.7 offsets; short frames fail.
#[test]
fn pkt_eff_01_decode_use_skill_and_use_item() {
    // 6723: Heal lv 10 on self.
    assert_eq!(
        decode(0x0438, &[0x0a, 0x00, 0x1c, 0x00, 0x80, 0x84, 0x1e, 0x00]),
        ClientIntent::UseSkill {
            level: 10,
            skill_id: 28,
            target: A,
        }
    );
    // 7879: Red Potion at index 7.
    assert_eq!(
        decode(0x0439, &[0x07, 0x00, 0x80, 0x84, 0x1e, 0x00]),
        ClientIntent::UseItem { index: 7 }
    );
    for (op, len) in [(0x0438, 10), (0x0439, 8)] {
        assert_eq!(
            lookup_packet_length(PacketOpcode(op), TARGET_PACKETVER),
            Some(PacketLength::Fixed(len))
        );
        let short = RawPacket::new(PacketOpcode(op), Bytes::from_static(&[0x01]));
        assert!(decode_frame(&short, TARGET_PACKETVER).is_err());
    }
}

/// PKT-EFF-02: the server packets encode byte for byte like the capture.
#[test]
fn pkt_eff_02_encode_matches_capture() {
    // 1687: `0x0b1a` First Aid on self, element 0, no delay.
    assert_eq!(
        encode(ServerEvent::SkillCasting {
            src: A,
            dst: A,
            x: 0,
            y: 0,
            skill_id: 142,
            element: 0,
            delay_ms: 0,
        }),
        frame(
            0x0b1a,
            &[
                0x80, 0x84, 0x1e, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8e, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ]
        )
    );
    // 6724: `0x0b1a` Heal, element 6 (Holy).
    assert_eq!(
        encode(ServerEvent::SkillCasting {
            src: A,
            dst: A,
            x: 0,
            y: 0,
            skill_id: 28,
            element: 6,
            delay_ms: 0,
        }),
        frame(
            0x0b1a,
            &[
                0x80, 0x84, 0x1e, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00,
                0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ]
        )
    );
    // 2004: `0x01de` Bash lv 10 for 237 on a Poring (sdelay 510, ddelay 480).
    assert_eq!(
        encode(ServerEvent::SkillDamage {
            skill_id: 5,
            src: A,
            dst: 0x068e_e54c,
            tick: 0x0dfd_8d5d,
            src_speed: 510,
            dmg_speed: 480,
            damage: 237,
            level: 10,
            div: 1,
            damage_type: 8,
        }),
        frame(
            0x01de,
            &[
                0x05, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x4c, 0xe5, 0x8e, 0x06, 0x5d, 0x8d, 0xfd, 0x0d,
                0xfe, 0x01, 0x00, 0x00, 0xe0, 0x01, 0x00, 0x00, 0xed, 0x00, 0x00, 0x00, 0x0a, 0x00,
                0x01, 0x00, 0x08
            ]
        )
    );
    // 6726: `0x09cb` Heal 64 on self.
    assert_eq!(
        encode(ServerEvent::SkillNoDamage {
            skill_id: 28,
            value: 64,
            target: A,
            src: A,
            ok: true,
        }),
        frame(
            0x09cb,
            &[
                0x1c, 0x00, 0x40, 0x00, 0x00, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x80, 0x84, 0x1e, 0x00,
                0x01
            ]
        )
    );
    // 6725: `0x0983` EFST_POSTDELAY 500 ms on the caster.
    assert_eq!(
        encode(ServerEvent::StatusIconStarted {
            efst: 46,
            gid: A,
            state: 1,
            total_ms: 500,
            remain_ms: 500,
            values: [0; 3],
        }),
        frame(
            0x0983,
            &[
                0x2e, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x01, 0xf4, 0x01, 0x00, 0x00, 0xf4, 0x01, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ]
        )
    );
    // 5844: `0x00b1` SP_ZENY 4000 after a Mammonite.
    assert_eq!(
        encode(ServerEvent::LongParameterChanged {
            var_id: 20,
            value: 4000,
        }),
        frame(0x00b1, &[0x14, 0x00, 0xa0, 0x0f, 0x00, 0x00])
    );
    // 7880: `0x01c8` Red Potion used, 4 left.
    assert_eq!(
        encode(ServerEvent::ItemUseAck {
            index: 7,
            item_id: 501,
            gid: A,
            amount: 4,
            ok: true,
        }),
        frame(
            0x01c8,
            &[0x07, 0x00, 0xf5, 0x01, 0x00, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x04, 0x00, 0x01]
        )
    );
    // 6081: `0x0110` Mammonite, cause 5 (zeny).
    assert_eq!(
        encode(ServerEvent::SkillFail {
            skill_id: 42,
            cause: 5,
        }),
        frame(
            0x0110,
            &[0x2a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05]
        )
    );
    // The failure ack (not captured): amount 0, result 0.
    assert_eq!(
        encode(ServerEvent::ItemUseAck {
            index: 7,
            item_id: 501,
            gid: A,
            amount: 0,
            ok: false,
        }),
        frame(
            0x01c8,
            &[0x07, 0x00, 0xf5, 0x01, 0x00, 0x00, 0x80, 0x84, 0x1e, 0x00, 0x00, 0x00, 0x00]
        )
    );
    for op in [0x0b1a, 0x01de, 0x09cb, 0x01c8, 0x00b1] {
        assert!(is_implemented_outbound_opcode(
            PacketOpcode(op),
            TARGET_PACKETVER
        ));
    }
}
