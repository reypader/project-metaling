//! Combat packets (spec `specs/combat/spec.md` PKT-CMB-01). Bytes from
//! `reference/catalog/captures/combat_sequence_1.txt` and `combat_sequence_2.txt` (proxy #).

use bytes::Bytes;
use ro_core::{ClientIntent, ServerEvent, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, is_implemented_outbound_opcode, lookup_packet_length, DecodedFrame, PacketLength,
    PacketOpcode, RawPacket,
};

use crate::common::encode;

fn with_opcode(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = opcode.to_le_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

fn assert_fixed(opcode: u16, len: usize) {
    assert_eq!(
        lookup_packet_length(PacketOpcode(opcode), TARGET_PACKETVER),
        Some(PacketLength::Fixed(len)),
        "table length of {opcode:#06x}"
    );
}

/// PKT-CMB-01: `0x08c8` with every field set; capture 1 #571 (Knife hit 12 on a Poring).
#[test]
fn pkt_cmb_01_damage_encodes_08c8() {
    let bytes = encode(ServerEvent::Damage {
        source_id: 2_000_000,
        target_id: 0x068e_e4de,
        tick: 0x0935_4409,
        src_speed: 590,
        dmg_speed: 480,
        damage: 12,
        div: 1,
        damage_type: 0,
        damage2: 0,
    });
    assert_eq!(
        bytes,
        with_opcode(
            0x08c8,
            &[
                0x80, 0x84, 0x1e, 0x00, 0xde, 0xe4, 0x8e, 0x06, 0x09, 0x44, 0x35, 0x09, 0x4e, 0x02,
                0x00, 0x00, 0xe0, 0x01, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00,
            ]
        )
    );

    // A crit (type 10) with a large damage and a damage2: the fields keep their offsets.
    let bytes = encode(ServerEvent::Damage {
        source_id: 1,
        target_id: 2,
        tick: 3,
        src_speed: 4,
        dmg_speed: 5,
        damage: 70_000,
        div: 2,
        damage_type: 10,
        damage2: 47,
    });
    assert_eq!(bytes.len(), 34);
    assert_eq!(&bytes[22..26], &70_000i32.to_le_bytes());
    assert_eq!(bytes[26], 0, "is_sp_damage");
    assert_eq!(&bytes[27..29], &[0x02, 0x00]);
    assert_eq!(bytes[29], 10);
    assert_eq!(&bytes[30..34], &47i32.to_le_bytes());
}

/// PKT-CMB-01: `0x0977` (capture 1 #574: hp 43 of 55), `0x0139` (#939), `0x0acc` (#687/#688),
/// `0x0110` cause 84 (capture 2 #2138) and `0x07fa` reason 1 (capture 1 #3428).
#[test]
fn pkt_cmb_01_combat_events_encode_as_captured() {
    assert_eq!(
        encode(ServerEvent::MobHpBar {
            gid: 0x068e_e4de,
            hp: 43,
            max_hp: 55,
        }),
        with_opcode(
            0x0977,
            &[0xde, 0xe4, 0x8e, 0x06, 0x2b, 0x00, 0x00, 0x00, 0x37, 0x00, 0x00, 0x00]
        )
    );

    assert_eq!(
        encode(ServerEvent::AttackOutOfRange {
            target_id: 0x068e_e521,
            target_x: 149,
            target_y: 192,
            x: 150,
            y: 189,
            range: 1,
        }),
        with_opcode(
            0x0139,
            &[0x21, 0xe5, 0x8e, 0x06, 0x95, 0x00, 0xc0, 0x00, 0x96, 0x00, 0xbd, 0x00, 0x01, 0x00]
        )
    );

    assert_eq!(
        encode(ServerEvent::ExpGained {
            gid: 2_000_000,
            var_id: 1,
            amount: 150,
        }),
        with_opcode(
            0x0acc,
            &[
                0x80, 0x84, 0x1e, 0x00, 0x96, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00,
            ]
        )
    );
    assert_eq!(
        encode(ServerEvent::ExpGained {
            gid: 2_000_000,
            var_id: 2,
            amount: 40,
        }),
        with_opcode(
            0x0acc,
            &[
                0x80, 0x84, 0x1e, 0x00, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
                0x00, 0x00,
            ]
        )
    );

    assert_eq!(
        encode(ServerEvent::SkillFail {
            skill_id: 0,
            cause: 84,
        }),
        with_opcode(
            0x0110,
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x54]
        )
    );

    assert_eq!(
        encode(ServerEvent::ItemDeleted {
            index: 10,
            amount: 1,
            reason: 1,
        }),
        with_opcode(0x07fa, &[0x01, 0x00, 0x0a, 0x00, 0x01, 0x00])
    );
}

/// PKT-CMB-01: `0x0118` (capture 2 #781, no payload) parses into `CancelAttack`.
#[test]
fn pkt_cmb_01_cancel_lockon_decodes() {
    let raw = RawPacket::new(PacketOpcode(0x0118), Bytes::new());
    assert_eq!(
        decode_frame(&raw, TARGET_PACKETVER).expect("decodes"),
        DecodedFrame::Intent(ClientIntent::CancelAttack)
    );
}

/// PKT-CMB-01: table lengths and the implemented-outbound list.
#[test]
fn pkt_cmb_01_lengths_and_implemented() {
    for (opcode, len) in [
        (0x0118, 2),
        (0x08c8, 34),
        (0x0977, 14),
        (0x0139, 16),
        (0x0acc, 18),
        (0x0110, 14),
        (0x07fa, 8),
    ] {
        assert_fixed(opcode, len);
    }
    for opcode in [
        PacketOpcode::ZC_NOTIFY_ACT,
        PacketOpcode::ZC_HP_INFO,
        PacketOpcode::ZC_ATTACK_FAILURE_FOR_DISTANCE,
        PacketOpcode::ZC_NOTIFY_EXP2,
        PacketOpcode::ZC_ACK_TOUSESKILL,
        PacketOpcode::ZC_DELETE_ITEM_FROM_BODY,
    ] {
        assert!(
            is_implemented_outbound_opcode(opcode, TARGET_PACKETVER),
            "{opcode} is implemented"
        );
    }
    assert_eq!(PacketOpcode(0x0977).name(), Some("ZC_HP_INFO"));
    assert_eq!(PacketOpcode(0x0acc).name(), Some("ZC_NOTIFY_EXP2"));
    assert_eq!(PacketOpcode(0x0110).name(), Some("ZC_ACK_TOUSESKILL"));
    assert_eq!(
        PacketOpcode(0x0139).name(),
        Some("ZC_ATTACK_FAILURE_FOR_DISTANCE")
    );
    assert_eq!(
        PacketOpcode(0x07fa).name(),
        Some("ZC_DELETE_ITEM_FROM_BODY")
    );
}
