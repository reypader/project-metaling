//! Packet length lookup table for `TARGET_PACKETVER = 20211103`.
//!
//! Follows definitions from `reference/rathena/src/map/clif_packetdb.hpp`
//! and `reference/rust-ro/lib/packets/`.

use ro_contracts::packet::{PacketLength, PacketLengthTable, PacketOpcode};

/// Standard packet length table implementation referencing rAthena packet databases.
#[derive(Debug, Default, Clone, Copy)]
pub struct RathenaPacketLengthTable;

impl PacketLengthTable for RathenaPacketLengthTable {
    fn packet_length(&self, opcode: PacketOpcode, packetver: u32) -> Option<PacketLength> {
        lookup_packet_length(opcode, packetver)
    }
}

/// Looks up the expected length classification for a packet opcode under a given packetver.
///
/// Returns:
/// - `Some(PacketLength::Fixed(n))` for invariant length frames.
/// - `Some(PacketLength::Dynamic)` for frames with a 2-byte length header at bytes 2..4.
/// - `None` if the opcode is completely unrecognized.
pub fn lookup_packet_length(opcode: PacketOpcode, packetver: u32) -> Option<PacketLength> {
    use PacketLength::{Dynamic, Fixed};

    let op = opcode.raw();

    // Check version-dependent overrides first if applicable.
    if packetver >= 20170315 {
        match op {
            0x0ac4 => return Some(Dynamic),
            0x0ac5 => return Some(Fixed(156)),
            _ => {}
        }
    }

    if packetver >= 20151001 && op == 0x0a39 {
        return Some(Fixed(36));
    }

    // Modern client selection and check blocks (2004Sakexe and newer).
    match op {
        // Phase 0 Login Server Packets
        0x0064 => Some(Fixed(55)), // CA_LOGIN
        0x0277 => Some(Fixed(84)), // CA_LOGIN_PCBANG
        0x0069 => Some(Dynamic),   // AC_ACCEPT_LOGIN
        0x006a => Some(Fixed(23)), // AC_REFUSE_LOGIN
        0x01dc => Some(Dynamic),   // CA_LOGIN2 / AC_ACK_HASH
        0x0204 => Some(Fixed(18)), // CA_EXE_HASHCHECK
        0x02ac => Some(Dynamic),   // CA_SSO_LOGIN_REQ

        // Phase 0 Char Server Packets
        0x0065 => Some(Fixed(17)), // CH_ENTER
        0x0066 => Some(Fixed(3)),  // CH_SELECT_CHAR (PACKETVER >= 20041213 is 3 bytes)
        0x006b => Some(Dynamic),   // HC_ACCEPT_ENTER
        0x006c => Some(Fixed(3)),  // HC_REFUSE_ENTER
        0x006d => Some(Fixed(108)), // HC_ACK_MAKE_CHAR
        0x0067 => Some(Fixed(37)), // CH_MAKE_CHAR (pre-2015)
        0x0068 => Some(Fixed(46)), // CH_DELETE_CHAR
        0x0070 => Some(Fixed(3)),  // HC_ACK_DELETE_CHAR (or 6 in older)
        0x0071 => Some(Fixed(28)), // HC_NOTIFY_ZONESVR
        0x082c => Some(Fixed(10)), // CH_DELETE_CHAR3_CANCEL
        0x082d => Some(Fixed(29)), // HC_ACCEPT_ENTER2
        0x099d => Some(Dynamic),   // HC_ACK_CHARINFO_PER_PAGE
        0x09a0 => Some(Fixed(6)),  // HC_CHARLIST_NOTIFY
        0x09a1 => Some(Fixed(2)),  // CH_CHARLIST_REQ

        // Phase 0 Map Server Packets
        0x0072 => Some(Fixed(19)), // CZ_ENTER
        0x0073 => Some(Fixed(11)), // ZC_ACCEPT_ENTER
        0x02eb => Some(Fixed(13)), // ZC_ACCEPT_ENTER2
        0x007d => Some(Fixed(2)),  // CZ_NOTIFY_ACTORINIT
        0x007e => Some(Fixed(6)),  // CZ_REQUEST_TIME
        0x007f => Some(Fixed(6)),  // ZC_NOTIFY_TIME
        0x0080 => Some(Fixed(7)),  // ZC_NOTIFY_VANISH
        0x0085 => Some(Fixed(5)),  // CZ_REQUEST_MOVE
        0x0087 => Some(Fixed(12)), // ZC_NOTIFY_PLAYERMOVE
        0x0089 => Some(Fixed(7)),  // CZ_REQUEST_ACT
        0x008a => Some(Fixed(29)), // ZC_NOTIFY_ACT
        0x0091 => Some(Fixed(18)), // ZC_NPCACK_MAPMOVE
        0x0094 => Some(Fixed(6)),  // CZ_REQNAME
        0x0095 => Some(Dynamic),   // ZC_ACK_REQNAME
        0x009b => Some(Fixed(5)),  // CZ_CHANGE_DIRECTION
        0x009c => Some(Fixed(53)), // ZC_NOTIFY_STANDENTRY
        0x009d => Some(Fixed(53)), // ZC_NOTIFY_NEWENTRY / ZC_ITEM_ENTRY
        0x00be => Some(Fixed(5)),  // ZC_PAR_CHANGE / ZC_STATUS_CHANGE
        0x00b0 => Some(Fixed(8)),  // ZC_PAR_CHANGE (32-bit count)
        0x00b1 => Some(Fixed(8)),  // ZC_LONGPAR_CHANGE

        // General map client packets from clif_packetdb.hpp
        0x006e => Some(Fixed(3)),
        0x006f => Some(Fixed(2)),
        0x0075 => Some(Dynamic),
        0x0076 => Some(Fixed(9)),
        0x0077 => Some(Fixed(5)),
        0x0079 => Some(Fixed(53)),
        0x007a => Some(Fixed(58)),
        0x007b => Some(Fixed(60)),
        0x007c => Some(Fixed(41)),
        0x0082 => Some(Fixed(2)),
        0x0083 => Some(Fixed(2)),
        0x0084 => Some(Fixed(2)),
        0x008b => Some(Fixed(2)),
        0x008c => Some(Dynamic), // GlobalMessage
        0x008d => Some(Dynamic),
        0x008e => Some(Dynamic),
        0x0090 => Some(Fixed(19)), // CZ_CONTACTNPC
        0x0093 => Some(Fixed(2)),
        0x0096 => Some(Dynamic), // WisMessage
        0x0099 => Some(Dynamic), // Broadcast
        0x009e => Some(Fixed(17)),
        0x009f => Some(Fixed(6)), // TakeItem
        0x00a2 => Some(Fixed(6)), // DropItem
        0x00a7 => Some(Fixed(8)), // UseItem
        0x00a9 => Some(Fixed(6)), // EquipItem
        0x00ab => Some(Fixed(4)), // UnequipItem
        0x00ae => Some(Dynamic),
        0x00b2 => Some(Fixed(3)), // Restart
        0x00b8 => Some(Fixed(7)), // NpcSelectMenu
        0x00b9 => Some(Fixed(6)), // NpcNextClicked
        0x00ba => Some(Fixed(2)),
        0x00bb => Some(Fixed(5)), // StatusUp
        0x00bf => Some(Fixed(3)), // Emotion
        0x00c1 => Some(Fixed(2)), // HowManyConnections
        0x00c3 => Some(Fixed(8)),
        0x00c5 => Some(Fixed(7)), // NpcBuySellSelected
        0x00c6 => Some(Dynamic),
        0x00c8 => Some(Dynamic), // NpcBuyListSend
        0x00c9 => Some(Dynamic), // NpcSellListSend
        0x00ca => Some(Fixed(3)),
        0x00cb => Some(Fixed(3)),
        0x00cc => Some(Fixed(6)), // GMKick
        0x00cd => Some(Fixed(3)),
        0x00ce => Some(Fixed(2)), // GMKickAll
        0x00cf => Some(Fixed(27)), // PMIgnore
        0x00d0 => Some(Fixed(7)),
        0x00d3 => Some(Fixed(2)), // PMIgnoreList
        0x00d5 => Some(Dynamic), // CreateChatRoom
        0x00d9 => Some(Fixed(11)), // ChatAddMember
        0x00da => Some(Dynamic), // ChatRoomStatusChange
        0x00e0 => Some(Fixed(30)), // ChangeChatOwner
        0x00e2 => Some(Fixed(26)), // KickFromChat
        0x00e3 => Some(Fixed(2)),  // ChatLeave
        0x00e4 => Some(Fixed(6)),  // TradeRequest
        0x00e5 => Some(Fixed(26)),
        0x00e6 => Some(Fixed(3)),  // TradeAck
        0x00e8 => Some(Fixed(8)),  // TradeAddItem
        0x00ea => Some(Fixed(5)),
        0x00eb => Some(Fixed(2)),  // TradeOk
        0x00ed => Some(Fixed(2)),  // TradeCancel
        0x00ef => Some(Fixed(2)),  // TradeCommit
        0x00f3 => Some(Fixed(8)),  // MoveToKafra
        0x00f5 => Some(Fixed(8)),  // MoveFromKafra
        0x00f7 => Some(Fixed(2)),  // CloseKafra
        0x00f9 => Some(Fixed(26)), // CreateParty
        0x00fb => Some(Dynamic),
        0x00fc => Some(Fixed(26)), // PartyInvite
        0x00fd => Some(Fixed(27)),
        0x00ff => Some(Fixed(7)),  // ReplyPartyInvite
        0x0100 => Some(Fixed(2)),  // LeaveParty
        0x0101 => Some(Fixed(6)),
        0x0102 => Some(Fixed(6)),  // PartyChangeOption
        0x0103 => Some(Fixed(6)),  // RemovePartyMember
        0x0104 => Some(Fixed(79)),
        0x0108 => Some(Dynamic), // PartyMessage
        0x0109 => Some(Dynamic),
        0x0112 => Some(Fixed(4)),  // SkillUp
        0x0113 => Some(Fixed(10)), // UseSkillToId
        0x0114 => Some(Fixed(31)),
        0x0115 => Some(Fixed(35)),
        0x0116 => Some(Fixed(10)), // UseSkillToPos
        0x0118 => Some(Fixed(2)),  // StopAttack
        0x0119 => Some(Fixed(13)),
        0x011b => Some(Fixed(18)), // UseSkillMap
        0x011d => Some(Fixed(2)),  // RequestMemo
        0x011f => Some(Fixed(16)),
        0x0124 => Some(Fixed(7)),  // PutItemToCart
        0x0125 => Some(Fixed(7)),  // GetItemFromCart
        0x0126 => Some(Fixed(7)),  // MoveFromKafraToCart
        0x0127 => Some(Fixed(7)),  // MoveToKafraFromCart
        0x012a => Some(Fixed(2)),  // RemoveOption
        0x012e => Some(Fixed(2)),  // CloseVending
        0x012f => Some(Dynamic),   // OpenVending
        0x0130 => Some(Fixed(6)),  // VendingListReq
        0x0131 => Some(Dynamic),   // PurchaseReq
        0x0138 => Some(Fixed(3)),
        0x013f => Some(Fixed(26)), // GM_Item_Monster
        0x0140 => Some(Fixed(18)), // MapMove
        0x0142 => Some(Fixed(6)),  // NpcAmountInput
        0x0145 => Some(Fixed(19)),
        0x0146 => Some(Fixed(6)),  // NpcCloseClicked
        0x0147 => Some(Fixed(39)),
        0x0149 => Some(Fixed(9)),  // GMReqNoChat
        0x014a => Some(Fixed(6)),
        0x014d => Some(Fixed(2)),  // GuildCheckMaster
        0x014f => Some(Fixed(6)),  // GuildRequestInfo
        0x0150 => Some(Fixed(110)),
        0x0151 => Some(Fixed(6)),  // GuildRequestEmblem
        0x0152 => Some(Dynamic),
        0x0153 => Some(Dynamic),   // GuildChangeEmblem
        0x0154 => Some(Dynamic),
        0x0155 => Some(Dynamic),   // GuildChangeMemberPosition
        0x0157 => Some(Fixed(6)),
        0x0158 => Some(Dynamic),
        0x0159 => Some(Fixed(46)), // GuildLeave
        0x015a => Some(Fixed(46)), // GuildExpulsion
        0x015b => Some(Fixed(46)), // GuildBreak
        0x015f => Some(Fixed(42)),
        0x0161 => Some(Dynamic),   // GuildChangePositionInfo
        0x0164 => Some(Dynamic),
        0x0165 => Some(Fixed(30)), // CreateGuild
        0x0166 => Some(Dynamic),
        0x0167 => Some(Fixed(10)), // GuildInvite
        0x0168 => Some(Fixed(10)), // GuildReplyInvite
        0x016c => Some(Fixed(43)),
        0x016e => Some(Fixed(186)), // GuildChangeNotice
        0x0170 => Some(Fixed(14)), // GuildRequestAlliance
        0x0172 => Some(Fixed(10)), // GuildReplyAlliance
        0x0175 => Some(Fixed(6)),
        0x0176 => Some(Fixed(106)),
        0x0177 => Some(Dynamic),
        0x0178 => Some(Fixed(4)),  // ItemIdentify
        0x0179 => Some(Fixed(4)),  // UseCard
        0x017b => Some(Dynamic),
        0x017c => Some(Fixed(6)),  // InsertCard
        0x017e => Some(Dynamic),   // GuildMessage
        0x0180 => Some(Fixed(6)),  // GuildOpposition
        0x0182 => Some(Fixed(106)),
        0x0183 => Some(Fixed(10)), // GuildDelAlliance
        0x0185 => Some(Fixed(34)),
        0x0187 => Some(Fixed(6)),
        0x018a => Some(Fixed(4)),  // QuitGame
        0x018b => Some(Fixed(4)),
        0x018e => Some(Fixed(6)),  // ProduceMix
        0x0190 => Some(Fixed(90)), // UseSkillToPosMoreInfo
        0x0193 => Some(Fixed(6)),  // SolveCharName
        0x0196 => Some(Fixed(9)),
        0x0197 => Some(Fixed(2)),  // ResetChar
        0x0198 => Some(Fixed(8)),  // GMChangeMapType
        0x0199 => Some(Fixed(4)),
        0x019a => Some(Fixed(14)),
        0x019c => Some(Dynamic),   // LocalBroadcast
        0x019d => Some(Fixed(6)),  // GMHide
        0x019f => Some(Fixed(6)),  // CatchPet
        0x01a1 => Some(Fixed(3)),  // PetMenu
        0x01a3 => Some(Fixed(5)),
        0x01a5 => Some(Fixed(26)), // ChangePetName
        0x01a6 => Some(Dynamic),
        0x01a7 => Some(Fixed(4)),  // SelectEgg
        0x01a8 => Some(Fixed(4)),
        0x01a9 => Some(Fixed(6)),  // SendEmotion
        0x01ac => Some(Fixed(6)),
        0x01ad => Some(Dynamic),
        0x01ae => Some(Fixed(4)),  // SelectArrow
        0x01af => Some(Fixed(4)),  // ChangeCart
        0x01b0 => Some(Fixed(11)),
        0x01b1 => Some(Fixed(7)),
        0x01b2 => Some(Dynamic),   // OpenVending
        0x01b5 => Some(Fixed(18)),
        0x01b6 => Some(Fixed(114)),
        0x01b7 => Some(Fixed(6)),
        0x01b8 => Some(Fixed(3)),
        0x01ba => Some(Fixed(26)), // GMShift
        0x01bb => Some(Fixed(26)),
        0x01bc => Some(Fixed(26)), // GMRecall
        0x01bd => Some(Fixed(26)),
        0x01be => Some(Fixed(2)),
        0x01bf => Some(Fixed(3)),
        0x01c0 => Some(Fixed(2)),
        0x01c1 => Some(Fixed(14)),
        0x01c2 => Some(Fixed(10)),
        0x01c3 => Some(Dynamic),
        0x01c6 => Some(Fixed(4)),
        0x01c7 => Some(Fixed(2)),
        0x01cb => Some(Fixed(9)),
        0x01cc => Some(Fixed(9)),
        0x01cd => Some(Fixed(4)),  // AutoSpell
        0x01cf => Some(Fixed(28)),
        0x01d0 => Some(Fixed(8)),
        0x01d5 => Some(Dynamic),   // NpcStringInput
        0x01d7 => Some(Fixed(11)),
        0x01d8 => Some(Fixed(54)),
        0x01d9 => Some(Fixed(53)),
        0x01da => Some(Fixed(60)),
        0x01db => Some(Fixed(2)),
        0x01dd => Some(Fixed(47)),
        0x01de => Some(Fixed(33)),
        0x01df => Some(Fixed(6)),
        0x01e0 => Some(Fixed(30)),
        0x01e1 => Some(Fixed(8)),
        0x01e2 => Some(Fixed(34)),
        0x01e3 => Some(Fixed(14)),
        0x01e4 => Some(Fixed(2)),
        0x01e5 => Some(Fixed(6)),
        0x01e6 => Some(Fixed(26)),
        0x01e7 => Some(Fixed(2)),  // NoviceDoriDori
        0x01e8 => Some(Fixed(28)), // CreateParty2
        0x01ec => Some(Fixed(26)),
        0x01ed => Some(Fixed(2)),  // NoviceExplosionSpirits
        0x01f0 => Some(Dynamic),
        0x01f1 => Some(Dynamic),
        0x01f3 => Some(Fixed(10)),
        0x01f6 => Some(Fixed(34)),
        0x01f8 => Some(Fixed(2)),
        0x01f9 => Some(Fixed(6)),  // Adopt_request
        0x01fa => Some(Fixed(48)),
        0x01fb => Some(Fixed(56)),
        0x01fc => Some(Dynamic),
        0x0200 => Some(Fixed(26)),
        0x0201 => Some(Dynamic),
        0x0202 => Some(Fixed(26)), // FriendsListAdd
        0x0203 => Some(Fixed(10)), // FriendsListRemove
        0x0207 => Some(Fixed(34)),
        0x0208 => Some(Fixed(11)), // FriendsListReply
        0x0209 => Some(Fixed(36)),
        0x020a => Some(Fixed(10)),
        0x020d => Some(Dynamic),
        0x0229 => Some(Fixed(6)),  // Ping
        0x0281 => Some(Fixed(6)),
        0x035f => Some(Fixed(6)),  // CZ_REQUEST_TIME2
        0x0360 => Some(Fixed(6)),
        0x0361 => Some(Fixed(5)),  // CZ_REQUEST_MOVE2
        0x0369 => Some(Fixed(6)),
        0x0437 => Some(Fixed(5)),
        0x0438 => Some(Fixed(10)),
        0x08b3 => Some(Dynamic),
        0x0b93 => Some(Fixed(12)),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase0_incoming_packet_lengths() {
        let table = RathenaPacketLengthTable;
        let ver = ro_contracts::TARGET_PACKETVER;

        assert_eq!(table.packet_length(PacketOpcode(0x0064), ver), Some(PacketLength::Fixed(55)));
        assert_eq!(table.packet_length(PacketOpcode(0x0277), ver), Some(PacketLength::Fixed(84)));
        assert_eq!(table.packet_length(PacketOpcode(0x0065), ver), Some(PacketLength::Fixed(17)));
        assert_eq!(table.packet_length(PacketOpcode(0x0066), ver), Some(PacketLength::Fixed(3)));
        assert_eq!(table.packet_length(PacketOpcode(0x0072), ver), Some(PacketLength::Fixed(19)));
        assert_eq!(table.packet_length(PacketOpcode(0x007d), ver), Some(PacketLength::Fixed(2)));
        assert_eq!(table.packet_length(PacketOpcode(0x007e), ver), Some(PacketLength::Fixed(6)));
    }

    #[test]
    fn test_phase0_outgoing_packet_lengths() {
        let table = RathenaPacketLengthTable;
        let ver = ro_contracts::TARGET_PACKETVER;

        assert_eq!(table.packet_length(PacketOpcode(0x0069), ver), Some(PacketLength::Dynamic));
        assert_eq!(table.packet_length(PacketOpcode(0x006a), ver), Some(PacketLength::Fixed(23)));
        assert_eq!(table.packet_length(PacketOpcode(0x006b), ver), Some(PacketLength::Dynamic));
        assert_eq!(table.packet_length(PacketOpcode(0x0071), ver), Some(PacketLength::Fixed(28)));
        assert_eq!(table.packet_length(PacketOpcode(0x0073), ver), Some(PacketLength::Fixed(11)));
        assert_eq!(table.packet_length(PacketOpcode(0x007f), ver), Some(PacketLength::Fixed(6)));
        assert_eq!(table.packet_length(PacketOpcode(0x009d), ver), Some(PacketLength::Fixed(53)));
        assert_eq!(table.packet_length(PacketOpcode(0x00be), ver), Some(PacketLength::Fixed(5)));
    }

    #[test]
    fn test_unknown_packet_returns_none() {
        let table = RathenaPacketLengthTable;
        let ver = ro_contracts::TARGET_PACKETVER;

        assert_eq!(table.packet_length(PacketOpcode(0xffff), ver), None);
    }
}
