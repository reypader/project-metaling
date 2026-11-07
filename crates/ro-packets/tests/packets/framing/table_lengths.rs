//! Packet-length table rows checked against captures.

use super::*;

/// PKT-ITEM-01: item packet lengths match the 20260219 table and the capture.
#[test]
fn pkt_item_01_table_lengths() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;
    let expected = [
        (0x0add, PacketLength::Fixed(24)),
        (0x00a1, PacketLength::Fixed(6)),
        (0x009d, PacketLength::Fixed(19)),
        (0x0b41, PacketLength::Fixed(70)),
        (0x0b08, PacketLength::Dynamic),
        (0x0b0b, PacketLength::Fixed(4)),
        (0x01d7, PacketLength::Fixed(15)),
        (0x00af, PacketLength::Fixed(6)),
        (0x0b09, PacketLength::Dynamic),
        (0x0b39, PacketLength::Dynamic),
        (0x0a9b, PacketLength::Dynamic),
    ];
    for (op, len) in expected {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            Some(len),
            "opcode {op:#06x}"
        );
    }
}

/// PKT-EQ-01: equipment packet lengths match the 20260219 table, the catalog and the capture.
#[test]
fn pkt_eq_01_table_lengths() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;
    let expected = [
        (0x0999, 11), // ZC_ACK_WEAR_EQUIP_V5
        (0x099a, 9),  // ZC_ACK_TAKEOFF_EQUIP_V5
        (0x0bae, 3),  // ZC_ACK_TAKEOFF_EQUIP_ALL
        (0x013c, 4),  // ZC_EQUIP_ARROW
        (0x013b, 4),  // ZC_ACTION_FAILURE
        (0x0291, 4),  // ZC_MSG
        (0x07e2, 8),  // ZC_MSG_VALUE
        (0x07e6, 8),  // ZC_MSG_SKILL
        (0x09cd, 8),  // ZC_MSG_COLOR
        (0x0983, 29), // ZC_MSG_STATE_CHANGE3
        (0x0196, 9),  // ZC_SC_END
        (0x0998, 8),  // CZ_REQ_WEAR_EQUIP_V5
        (0x00ab, 4),  // CZ_REQ_TAKEOFF_EQUIP
        (0x0bf5, 6),  // CZ_REQ_TAKEOFF_EQUIP_ALL
        (0x013a, 4),  // ZC_ATTACK_RANGE
    ];
    for (op, len) in expected {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            Some(PacketLength::Fixed(len)),
            "opcode {op:#06x}"
        );
    }
}

/// PF-21: `0x080e` ZC_NOTIFY_HP_TO_GROUPM_R2 is 14 bytes (`u32 AID, i32 hp, i32 max_hp`), per
/// `reference/catalog/captures/equip_sequence.txt` line 267.
#[test]
fn pf21_party_hp_length() {
    use ro_packets::{PacketLength, PacketLengthTable};
    assert_eq!(
        RathenaPacketLengthTable.packet_length(PacketOpcode(0x080e), ro_core::TARGET_PACKETVER),
        Some(PacketLength::Fixed(14))
    );
}

/// PF-21: rows the captures misframed (catalog PF-21 table; `0x010e` after `@jlevel` and skill
/// points stalled the proxy log in `equip_sequence_3.txt` line 2967).
#[test]
fn pf21_capture_3_rows() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let len =
        |op| RathenaPacketLengthTable.packet_length(PacketOpcode(op), ro_core::TARGET_PACKETVER);
    for (op, n) in [
        (0x010e, 11),
        (0x0441, 4),
        (0x00b6, 6),
        (0x0148, 8),
        (0x09cb, 17),
        (0x0b1a, 29),
        (0x00c0, 7),
        (0x019b, 10),
        (0x07fa, 8),
        (0x0139, 16),
    ] {
        assert_eq!(len(op), Some(PacketLength::Fixed(n)), "0x{op:04x}");
    }
    assert_eq!(len(0x00b4), Some(PacketLength::Dynamic));
}

/// specs/stats: the stat-up request and its ack must frame, or a stat-up capture misframes.
#[test]
fn stats_stat_up_rows() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let len =
        |op| RathenaPacketLengthTable.packet_length(PacketOpcode(op), ro_core::TARGET_PACKETVER);
    assert_eq!(
        len(0x00bb),
        Some(PacketLength::Fixed(5)),
        "CZ_STATUS_CHANGE"
    );
    assert_eq!(
        len(0x00bc),
        Some(PacketLength::Fixed(6)),
        "ZC_STATUS_CHANGE_ACK"
    );
}
