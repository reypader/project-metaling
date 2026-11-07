//! Opcode constants and names the engine-foundation spec fixes.

use ro_packets::PacketOpcode;

#[test]
fn opcode_names_fixed_per_spec() {
    assert_eq!(PacketOpcode::ZC_NOTIFY_MOVEENTRY11, PacketOpcode(0x09fd));
    assert_eq!(PacketOpcode::ZC_NOTIFY_NEWENTRY11, PacketOpcode(0x09fe));
    assert_eq!(PacketOpcode::ZC_NOTIFY_STANDENTRY11, PacketOpcode(0x09ff));
    assert_eq!(PacketOpcode::ZC_ACK_REQNAME_BYGID, PacketOpcode(0x0af7));
    assert_eq!(PacketOpcode::ZC_ACK_REQNAMEALL, PacketOpcode(0x0a30));
    assert_eq!(PacketOpcode::ZC_CHANGE_DIRECTION, PacketOpcode(0x009c));
    assert_eq!(PacketOpcode::ZC_STOPMOVE, PacketOpcode(0x0088));

    assert_eq!(PacketOpcode(0x09fd).name(), Some("ZC_NOTIFY_MOVEENTRY11"));
    assert_eq!(PacketOpcode(0x09fe).name(), Some("ZC_NOTIFY_NEWENTRY11"));
    assert_eq!(PacketOpcode(0x09ff).name(), Some("ZC_NOTIFY_STANDENTRY11"));
    assert_eq!(PacketOpcode(0x0af7).name(), Some("ZC_ACK_REQNAME_BYGID"));
    assert_eq!(PacketOpcode(0x0a30).name(), Some("ZC_ACK_REQNAMEALL"));
    assert_eq!(PacketOpcode(0x009c).name(), Some("ZC_CHANGE_DIRECTION"));
    assert_eq!(PacketOpcode(0x0088).name(), Some("ZC_STOPMOVE"));
    assert_eq!(PacketOpcode(0x0080).name(), Some("ZC_NOTIFY_VANISH"));
    assert_eq!(PacketOpcode(0x0087).name(), Some("ZC_NOTIFY_PLAYERMOVE"));
    assert_eq!(PacketOpcode(0x0091).name(), Some("ZC_NPCACK_MAPMOVE"));
}
