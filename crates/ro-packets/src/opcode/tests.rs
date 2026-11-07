use super::*;

#[test]
fn test_packet_opcode_name_and_display_known() {
    assert_eq!(PacketOpcode::CA_LOGIN.name(), Some("CA_LOGIN"));
    assert_eq!(PacketOpcode::CA_LOGIN.to_string(), "0x0064 (CA_LOGIN)");

    assert_eq!(PacketOpcode::ZC_NOTIFY_TIME.name(), Some("ZC_NOTIFY_TIME"));
    assert_eq!(
        PacketOpcode::ZC_NOTIFY_TIME.to_string(),
        "0x007f (ZC_NOTIFY_TIME)"
    );

    assert_eq!(
        PacketOpcode::HC_REFUSE_DELETECHAR.name(),
        Some("HC_REFUSE_DELETECHAR")
    );
    assert_eq!(
        PacketOpcode::HC_ACK_DELETE_CHAR.name(),
        Some("HC_REFUSE_DELETECHAR")
    );
    assert_eq!(
        PacketOpcode::HC_ACK_DELETE_CHAR.to_string(),
        "0x0070 (HC_REFUSE_DELETECHAR)"
    );

    assert_eq!(
        PacketOpcode::HC_ACK_MAKE_CHAR.name(),
        Some("HC_ACK_MAKE_CHAR")
    );
    assert_eq!(
        PacketOpcode::HC_ACCEPT_MAKECHAR.name(),
        Some("HC_ACK_MAKE_CHAR")
    );
    assert_eq!(
        PacketOpcode::HC_ACCEPT_MAKECHAR.to_string(),
        "0x0b6f (HC_ACK_MAKE_CHAR)"
    );
}

#[test]
fn test_packet_opcode_name_and_display_unknown() {
    let unknown = PacketOpcode(0x0001);
    assert_eq!(unknown.name(), None);
    assert_eq!(unknown.to_string(), "0x0001");

    let unknown_high = PacketOpcode(0xffff);
    assert_eq!(unknown_high.name(), None);
    assert_eq!(unknown_high.to_string(), "0xffff");
}
