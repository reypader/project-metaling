use super::*;

#[test]
fn test_20260219_incoming_packet_lengths() {
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;

    assert_eq!(
        table.packet_length(PacketOpcode(0x0064), ver),
        Some(PacketLength::Fixed(55))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0277), ver),
        Some(PacketLength::Fixed(84))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0065), ver),
        Some(PacketLength::Fixed(17))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0066), ver),
        Some(PacketLength::Fixed(3))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0436), ver),
        Some(PacketLength::Fixed(23))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x007d), ver),
        Some(PacketLength::Fixed(2))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0360), ver),
        Some(PacketLength::Fixed(6))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x035f), ver),
        Some(PacketLength::Fixed(5))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0361), ver),
        Some(PacketLength::Fixed(5))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0437), ver),
        Some(PacketLength::Fixed(7))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0368), ver),
        Some(PacketLength::Fixed(6))
    );
}

#[test]
fn test_20260219_outgoing_packet_lengths() {
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;

    assert_eq!(
        table.packet_length(PacketOpcode(0x0ac4), ver),
        Some(PacketLength::Dynamic)
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x083e), ver),
        Some(PacketLength::Fixed(26))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x006b), ver),
        Some(PacketLength::Dynamic)
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0ac5), ver),
        Some(PacketLength::Fixed(156))
    );
    // Effects M0 proxy rows (catalog `scripting/skills.md` §5).
    for (op, len) in [
        (0x0111, 39),
        (0x0117, 18),
        (0x01b9, 6),
        (0x043d, 8),
        (0x08e2, 27),
        (0x0a27, 8),
    ] {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            Some(PacketLength::Fixed(len)),
            "opcode {op:#06x}"
        );
    }
    assert_eq!(
        table.packet_length(PacketOpcode(0x0b6f), ver),
        Some(PacketLength::Fixed(177))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x02eb), ver),
        Some(PacketLength::Fixed(13))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x007f), ver),
        Some(PacketLength::Fixed(6))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0080), ver),
        Some(PacketLength::Fixed(7))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0087), ver),
        Some(PacketLength::Fixed(12))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x08c8), ver),
        Some(PacketLength::Fixed(34))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x09fe), ver),
        Some(PacketLength::Dynamic)
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x09ff), ver),
        Some(PacketLength::Dynamic)
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x09fd), ver),
        Some(PacketLength::Dynamic)
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0af7), ver),
        Some(PacketLength::Fixed(32))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0b72), ver),
        Some(PacketLength::Dynamic)
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x00b3), ver),
        Some(PacketLength::Fixed(3))
    );
    assert_eq!(
        table.packet_length(PacketOpcode(0x0829), ver),
        Some(PacketLength::Fixed(12))
    );
    assert_eq!(
        table.packet_length(PacketOpcode::ZC_INVENTORY_EXPANSION_INFO, ver),
        Some(PacketLength::Fixed(4))
    );
    assert_eq!(
        table.packet_length(PacketOpcode::ZC_AID, ver),
        Some(PacketLength::Fixed(6))
    );
    assert_eq!(table.packet_length(PacketOpcode(0x099d), ver), None);
}

#[test]
fn pkt_01_engine_foundation_lengths() {
    use PacketLength::{Dynamic, Fixed};
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;
    let expected = [
        (0x035f, Fixed(5)),
        (0x0361, Fixed(5)),
        (0x0368, Fixed(6)),
        (0x0437, Fixed(7)),
        (0x0360, Fixed(6)),
        (0x0283, Fixed(6)),
        (0x0b18, Fixed(4)),
        (0x02eb, Fixed(13)),
        (0x007f, Fixed(6)),
        (0x0087, Fixed(12)),
        (0x0088, Fixed(10)),
        (0x009c, Fixed(9)),
        (0x09fd, Dynamic),
        (0x09fe, Dynamic),
        (0x09ff, Dynamic),
        (0x0080, Fixed(7)),
        (0x0a30, Fixed(106)),
        (0x0adf, Fixed(58)),
        (0x0af7, Fixed(32)),
        (0x08c8, Fixed(34)),
        (0x00b0, Fixed(8)),
        (0x0091, Fixed(22)),
    ];
    for (op, len) in expected {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            Some(len),
            "opcode {op:#06x}"
        );
    }
    assert_eq!(table.packet_length(PacketOpcode(0xffff), ver), None);
    assert_eq!(table.packet_length(PacketOpcode(0x0086), ver), None);
}

#[test]
fn test_legacy_opcodes_are_rejected() {
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;

    let legacy = [
        0x0067, 0x0068, 0x0069, 0x006a, 0x006d, 0x0071, 0x0072, 0x0073, 0x007e, 0x0085, 0x0086,
        0x0089, 0x008a, 0x0094, 0x0095, 0x009b,
    ];
    for op in legacy {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            None,
            "Legacy opcode {:#06x} must be None",
            op
        );
    }
}
