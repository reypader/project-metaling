//! Stat-up and level-effect packets (spec `specs/stats/spec.md` PKT-STAT-01). Byte vectors come
//! from `reference/catalog/captures/attribute_sequence.txt` and `attribute_sequence_2.txt`
//! (catalog `packets/status.md` "Stat-up request and acknowledgement").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;
use ro_core::{ClientIntent, ServerEvent, TARGET_PACKETVER};
use ro_packets::{
    decode_frame, is_implemented_outbound_opcode, lookup_packet_length, DecodedFrame, EventEncoder,
    ModernEventEncoder, PacketCodecError, PacketLength, PacketOpcode, RawPacket,
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

/// `opcode` little-endian, then `payload`.
fn frame(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = opcode.to_le_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

/// PKT-STAT-01: `0x00bb` parses into `RaiseStat`; the amount is unsigned (a captured 231 was
/// honoured); a truncated frame is an error, which drops the session.
#[test]
fn pkt_stat_01_decodes_the_stat_up_request() {
    for (payload, var, amount) in [
        (&[0x0d, 0x00, 0x01][..], 13, 1),
        (&[0x10, 0x00, 0x63][..], 16, 99),
        (&[0x0e, 0x00, 0xe7][..], 14, 231),
    ] {
        assert_eq!(
            decode(0x00bb, payload).expect("decodes"),
            DecodedFrame::Intent(ClientIntent::RaiseStat { var, amount }),
            "captured {payload:02x?}"
        );
    }
    assert!(matches!(
        decode(0x00bb, &[0x0d, 0x00]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
}

/// PKT-STAT-01: `0x00bc` and `0x019b` encode the captured bytes.
#[test]
fn pkt_stat_01_encodes_the_ack_and_the_effect() {
    assert_eq!(
        encode(ServerEvent::StatusChangeAck {
            var_id: 13,
            ok: true,
            value: 2
        }),
        frame(0x00bc, &[0x0d, 0x00, 0x01, 0x02]),
        "capture 2 line 632"
    );
    assert_eq!(
        encode(ServerEvent::StatusChangeAck {
            var_id: 16,
            ok: false,
            value: 0
        }),
        frame(0x00bc, &[0x10, 0x00, 0x00, 0x00]),
        "failure form"
    );
    assert_eq!(
        encode(ServerEvent::SpecialEffect {
            gid: 2_000_001,
            effect: 0
        }),
        frame(0x019b, &[0x81, 0x84, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00]),
        "capture 2 line 434"
    );
    for (op, len) in [(0x00bc, 6), (0x019b, 10)] {
        assert!(
            is_implemented_outbound_opcode(PacketOpcode(op), TARGET_PACKETVER),
            "{op:#06x}"
        );
        assert_eq!(
            lookup_packet_length(PacketOpcode(op), TARGET_PACKETVER),
            Some(PacketLength::Fixed(len))
        );
    }
}

/// PKT-TRAIT-01: `0x0b24` parses into `RaiseTrait` (captured `db 00 01 00`, `rathena_traits_sequence.txt`
/// line 488); a negative amount parses and the engine drops it; a truncated frame is an error,
/// which drops the session.
#[test]
fn pkt_trait_01_decodes_the_trait_up_request() {
    for (payload, var, amount) in [
        (&[0xdb, 0x00, 0x01, 0x00][..], 219, 1),
        (&[0xdf, 0x00, 0x05, 0x00][..], 223, 5),
        (&[0xdb, 0x00, 0xff, 0xff][..], 219, -1),
    ] {
        assert_eq!(
            decode(0x0b24, payload).expect("decodes"),
            DecodedFrame::Intent(ClientIntent::RaiseTrait { var, amount }),
            "{payload:02x?}"
        );
    }
    assert!(matches!(
        decode(0x0b24, &[0xdb, 0x00, 0x01]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
    assert_eq!(
        lookup_packet_length(PacketOpcode(0x0b24), TARGET_PACKETVER),
        Some(PacketLength::Fixed(6))
    );
}
