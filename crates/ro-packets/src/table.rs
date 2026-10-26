//! Packet length lookup table for `TARGET_PACKETVER = 20260219`.

use crate::{PacketLength, PacketLengthTable, PacketOpcode};

/// Standard packet length table implementation referencing rAthena packet databases.
#[derive(Debug, Default, Clone, Copy)]
pub struct RathenaPacketLengthTable;

impl PacketLengthTable for RathenaPacketLengthTable {
    fn packet_length(&self, opcode: PacketOpcode, packetver: u32) -> Option<PacketLength> {
        lookup_packet_length(opcode, packetver)
    }
}

/// Looks up the expected length classification for a packet opcode under 20260219.
///
/// Returns:
/// - `Some(PacketLength::Fixed(n))` for invariant length frames.
/// - `Some(PacketLength::Dynamic)` for frames with a 2-byte length header at bytes 2..4.
/// - `None` if the opcode is legacy or unrecognized.
pub fn lookup_packet_length(opcode: PacketOpcode, _packetver: u32) -> Option<PacketLength> {
    use PacketLength::{Dynamic, Fixed};

    let op = opcode.raw();

    match op {
        0x0064 => Some(Fixed(55)), // CA_LOGIN
        0x0065 => Some(Fixed(17)), // CH_ENTER
        0x0066 => Some(Fixed(3)),  // CH_SELECT_CHAR
        0x006b => Some(Dynamic),   // HC_ACCEPT_ENTER
        0x006c => Some(Fixed(3)),  // HC_REFUSE_ENTER
        0x006e => Some(Fixed(3)),  // HC_REFUSE_MAKECHAR
        0x006f => Some(Fixed(2)),  // HC_ACCEPT_DELETECHAR
        0x0070 => Some(Fixed(3)),  // HC_REFUSE_DELETECHAR
        0x0075 => Some(Dynamic),
        0x0076 => Some(Fixed(9)),
        0x0077 => Some(Fixed(5)),
        0x0079 => Some(Fixed(53)),
        0x007a => Some(Fixed(58)),
        0x007b => Some(Fixed(60)),
        0x007c => Some(Fixed(44)),
        0x007d => Some(Fixed(2)),
        0x007f => Some(Fixed(6)), // ZC_NOTIFY_TIME
        0x0080 => Some(Fixed(7)), // ZC_NOTIFY_VANISH
        0x0082 => Some(Fixed(2)),
        0x0083 => Some(Fixed(2)),
        0x0084 => Some(Fixed(2)),
        0x0087 => Some(Fixed(12)), // ZC_NOTIFY_PLAYERMOVE
        0x0088 => Some(Fixed(10)), // ZC_STOPMOVE (struct size; not in rAthena packet DB)
        0x008b => Some(Fixed(2)),
        0x008c => Some(Fixed(14)),
        0x008d => Some(Dynamic),
        0x008e => Some(Dynamic),
        0x0090 => Some(Fixed(7)),
        0x0091 => Some(Fixed(22)), // ZC_NPCACK_MAPMOVE
        0x0093 => Some(Fixed(2)),
        0x0096 => Some(Dynamic),
        0x0099 => Some(Dynamic),
        0x009c => Some(Fixed(9)),  // ZC_CHANGE_DIRECTION (struct size)
        0x009d => Some(Fixed(19)), // ZC_ITEM_ENTRY
        0x009e => Some(Fixed(17)),
        0x009f => Some(Fixed(20)),
        0x00a1 => Some(Fixed(6)), // ZC_ITEM_DISAPPEAR
        0x00a2 => Some(Fixed(14)),
        0x00a7 => Some(Fixed(9)),
        0x00ab => Some(Fixed(4)),
        0x00ae => Some(Dynamic),
        0x00af => Some(Fixed(6)), // ZC_ITEM_THROW_ACK
        0x00b0 => Some(Fixed(8)), // ZC_PAR_CHANGE
        0x00b1 => Some(Fixed(8)), // ZC_LONGPAR_CHANGE
        0x00b2 => Some(Fixed(3)),
        0x00b3 => Some(Fixed(3)), // ZC_RESTART_ACK
        0x00b4 => Some(Dynamic),  // ZC_SAY_DIALOG (PF-21)
        0x00b6 => Some(Fixed(6)), // ZC_CLOSE_DIALOG (PF-21)
        0x00b8 => Some(Fixed(7)),
        0x00b9 => Some(Fixed(6)),
        0x00ba => Some(Fixed(2)),
        0x00bb => Some(Fixed(5)),
        0x00bc => Some(Fixed(6)),  // ZC_STATUS_CHANGE_ACK (specs/stats)
        0x00bd => Some(Fixed(44)), // ZC_STATUS (specs/map-entry-settings)
        0x00be => Some(Fixed(5)),  // ZC_STATUS_CHANGE
        0x00bf => Some(Fixed(3)),
        0x00c0 => Some(Fixed(7)), // ZC_EMOTION (PF-21)
        0x00c1 => Some(Fixed(2)),
        0x00c3 => Some(Fixed(8)),
        0x00c5 => Some(Fixed(7)),
        0x00c6 => Some(Dynamic),
        0x00c8 => Some(Dynamic),
        0x00c9 => Some(Dynamic),
        0x00ca => Some(Fixed(3)),
        0x00cb => Some(Fixed(3)),
        0x00cc => Some(Fixed(6)),
        0x00cd => Some(Fixed(3)),
        0x00ce => Some(Fixed(2)),
        0x00cf => Some(Fixed(27)),
        0x00d0 => Some(Fixed(3)),
        0x00d3 => Some(Fixed(2)),
        0x00d5 => Some(Dynamic),
        0x00d9 => Some(Fixed(14)),
        0x00de => Some(Dynamic),
        0x00e0 => Some(Fixed(30)),
        0x00e2 => Some(Fixed(26)),
        0x00e3 => Some(Fixed(2)),
        0x00e4 => Some(Fixed(6)),
        0x00e5 => Some(Fixed(26)),
        0x00e6 => Some(Fixed(3)),
        0x00e8 => Some(Fixed(8)),
        0x00ea => Some(Fixed(5)),
        0x00eb => Some(Fixed(2)),
        0x00ed => Some(Fixed(2)),
        0x00ef => Some(Fixed(2)),
        0x00f3 => Some(Dynamic),
        0x00f5 => Some(Fixed(11)),
        0x00f7 => Some(Fixed(17)),
        0x00f9 => Some(Fixed(26)),
        0x00fb => Some(Dynamic),
        0x00fc => Some(Fixed(6)),
        0x00fd => Some(Fixed(27)),
        0x00ff => Some(Fixed(10)),
        0x0100 => Some(Fixed(2)),
        0x0101 => Some(Fixed(6)),
        0x0102 => Some(Fixed(6)),
        0x0103 => Some(Fixed(30)),
        0x0104 => Some(Fixed(79)),
        0x0108 => Some(Dynamic),
        0x0109 => Some(Dynamic),
        0x010e => Some(Fixed(11)), // ZC_SKILLINFO_UPDATE (PF-21; its absence stalled equip_sequence_3)
        0x010f => Some(Dynamic),   // ZC_SKILLINFO_LIST (specs/map-entry-settings)
        0x0112 => Some(Fixed(4)),
        0x0113 => Some(Fixed(25)),
        0x0114 => Some(Fixed(31)),
        0x0115 => Some(Fixed(35)),
        0x0116 => Some(Fixed(17)),
        0x0118 => Some(Fixed(2)),
        0x0119 => Some(Fixed(13)),
        0x011b => Some(Fixed(20)),
        0x011d => Some(Fixed(2)),
        0x011f => Some(Fixed(16)),
        0x0126 => Some(Fixed(8)),
        0x0127 => Some(Fixed(8)),
        0x0128 => Some(Fixed(8)),
        0x0129 => Some(Fixed(8)),
        0x012a => Some(Fixed(2)),
        0x012e => Some(Fixed(2)),
        0x012f => Some(Dynamic),
        0x0130 => Some(Fixed(6)),
        0x0134 => Some(Dynamic),
        0x0138 => Some(Fixed(3)),
        0x0139 => Some(Fixed(16)), // ZC_ATTACK_FAILURE_FOR_DISTANCE (PF-21)
        0x013a => Some(Fixed(4)),  // ZC_ATTACK_RANGE (specs/map-entry-settings)
        0x013b => Some(Fixed(4)),  // ZC_ACTION_FAILURE (specs/equipment)
        0x013c => Some(Fixed(4)),  // ZC_EQUIP_ARROW (specs/equipment)
        0x013f => Some(Fixed(26)),
        0x0140 => Some(Fixed(22)),
        0x0141 => Some(Fixed(14)), // ZC_COUPLESTATUS (specs/map-entry-settings)
        0x0143 => Some(Fixed(10)),
        0x0145 => Some(Fixed(19)),
        0x0146 => Some(Fixed(6)),
        0x0147 => Some(Fixed(39)),
        0x0148 => Some(Fixed(8)), // ZC_RESURRECTION (PF-21)
        0x0149 => Some(Fixed(9)),
        0x014a => Some(Fixed(6)),
        0x014d => Some(Fixed(2)),
        0x014f => Some(Fixed(6)),
        0x0150 => Some(Fixed(110)),
        0x0151 => Some(Fixed(6)),
        0x0152 => Some(Dynamic),
        0x0153 => Some(Dynamic),
        0x0154 => Some(Dynamic),
        0x0155 => Some(Dynamic),
        0x0157 => Some(Fixed(6)),
        0x0158 => Some(Dynamic),
        0x0159 => Some(Fixed(54)),
        0x015b => Some(Fixed(54)),
        0x015d => Some(Fixed(42)),
        0x015f => Some(Fixed(42)),
        0x0161 => Some(Dynamic),
        0x0164 => Some(Dynamic),
        0x0165 => Some(Fixed(30)),
        0x0166 => Some(Dynamic),
        0x0168 => Some(Fixed(14)),
        0x016b => Some(Fixed(10)),
        0x016c => Some(Fixed(43)),
        0x016e => Some(Fixed(186)),
        0x0170 => Some(Fixed(14)),
        0x0172 => Some(Fixed(10)),
        0x0175 => Some(Fixed(6)),
        0x0176 => Some(Fixed(106)),
        0x0177 => Some(Dynamic),
        0x0178 => Some(Fixed(4)),
        0x017a => Some(Fixed(4)),
        0x017b => Some(Dynamic),
        0x017c => Some(Fixed(6)),
        0x017e => Some(Dynamic),
        0x0180 => Some(Fixed(6)),
        0x0182 => Some(Fixed(106)),
        0x0183 => Some(Fixed(10)),
        0x0185 => Some(Fixed(34)),
        0x0187 => Some(Fixed(6)), // PING
        0x018a => Some(Fixed(4)),
        0x018b => Some(Fixed(4)),
        0x018e => Some(Fixed(18)),
        0x0190 => Some(Fixed(23)),
        0x0193 => Some(Fixed(2)),
        0x0196 => Some(Fixed(9)),
        0x0197 => Some(Fixed(4)),
        0x0198 => Some(Fixed(8)),
        0x0199 => Some(Fixed(4)),
        0x019a => Some(Fixed(14)),
        0x019b => Some(Fixed(10)), // ZC_NOTIFY_EFFECT (PF-21)
        0x019c => Some(Dynamic),
        0x019d => Some(Fixed(6)),
        0x019f => Some(Fixed(6)),
        0x01a1 => Some(Fixed(3)),
        0x01a3 => Some(Fixed(5)),
        0x01a5 => Some(Fixed(26)),
        0x01a6 => Some(Dynamic),
        0x01a7 => Some(Fixed(4)),
        0x01a8 => Some(Fixed(4)),
        0x01a9 => Some(Fixed(6)),
        0x01ac => Some(Fixed(6)),
        0x01ad => Some(Dynamic),
        0x01ae => Some(Fixed(6)),
        0x01af => Some(Fixed(4)),
        0x01b0 => Some(Fixed(11)),
        0x01b1 => Some(Fixed(7)),
        0x01b2 => Some(Dynamic),
        0x01b5 => Some(Fixed(18)),
        0x01b6 => Some(Fixed(114)),
        0x01b7 => Some(Fixed(6)),
        0x01b8 => Some(Fixed(3)),
        0x01ba => Some(Fixed(26)),
        0x01bb => Some(Fixed(26)),
        0x01bc => Some(Fixed(26)),
        0x01bd => Some(Fixed(26)),
        0x01be => Some(Fixed(2)),
        0x01bf => Some(Fixed(3)),
        0x01c0 => Some(Fixed(2)),
        0x01c1 => Some(Fixed(14)),
        0x01c2 => Some(Fixed(10)),
        0x01c3 => Some(Dynamic),
        0x01c6 => Some(Fixed(4)),
        0x01c7 => Some(Fixed(2)),
        0x01c8 => Some(Fixed(15)),
        0x01ca => Some(Fixed(3)),
        0x01cb => Some(Fixed(9)),
        0x01cc => Some(Fixed(9)),
        0x01ce => Some(Fixed(6)),
        0x01cf => Some(Fixed(28)),
        0x01d0 => Some(Fixed(8)),
        0x01d5 => Some(Dynamic),
        0x01d7 => Some(Fixed(15)), // ZC_SPRITE_CHANGE2 (captured)
        0x01d8 => Some(Fixed(54)),
        0x01d9 => Some(Fixed(53)),
        0x01da => Some(Fixed(60)),
        0x01db => Some(Fixed(2)),  // CA_REQ_HASH
        0x01dc => Some(Dynamic),   // AC_ACK_HASH
        0x01dd => Some(Fixed(48)), // CA_LOGIN2
        0x01de => Some(Fixed(33)),
        0x01df => Some(Fixed(6)),
        0x01e0 => Some(Fixed(30)),
        0x01e1 => Some(Fixed(8)),
        0x01e2 => Some(Fixed(34)),
        0x01e3 => Some(Fixed(14)),
        0x01e4 => Some(Fixed(2)),
        0x01e5 => Some(Fixed(6)),
        0x01e6 => Some(Fixed(26)),
        0x01e7 => Some(Fixed(2)),
        0x01e8 => Some(Fixed(28)),
        0x01ec => Some(Fixed(26)),
        0x01ed => Some(Fixed(2)),
        0x01f0 => Some(Dynamic),
        0x01f1 => Some(Dynamic),
        0x01f3 => Some(Fixed(10)),
        0x01f6 => Some(Fixed(34)),
        0x01f7 => Some(Fixed(14)),
        0x01f8 => Some(Fixed(2)),
        0x01f9 => Some(Fixed(6)),
        0x01fa => Some(Fixed(48)), // CA_LOGIN3
        0x01fb => Some(Fixed(56)), // CH_DELETE_CHAR
        0x01fc => Some(Dynamic),
        0x01fd => Some(Fixed(25)),
        0x0200 => Some(Fixed(18)), // CA_CONNECT_INFO_CHANGED
        0x0201 => Some(Dynamic),
        0x0202 => Some(Fixed(26)),
        0x0203 => Some(Fixed(10)),
        0x0204 => Some(Fixed(18)), // CA_EXE_HASHCHECK
        0x0207 => Some(Fixed(34)),
        0x0208 => Some(Fixed(14)),
        0x0209 => Some(Fixed(36)),
        0x020a => Some(Fixed(10)),
        0x020d => Some(Dynamic),
        0x020e => Some(Fixed(32)),
        0x020f => Some(Fixed(10)),
        0x0210 => Some(Fixed(22)),
        0x0212 => Some(Fixed(26)),
        0x0213 => Some(Fixed(26)),
        0x0214 => Some(Fixed(42)),
        0x0216 => Some(Fixed(6)),
        0x021d => Some(Fixed(6)),
        0x021e => Some(Fixed(6)),
        0x021f => Some(Fixed(66)),
        0x0220 => Some(Fixed(10)),
        0x0221 => Some(Dynamic),
        0x0222 => Some(Fixed(6)),
        0x0227 => Some(Fixed(18)),
        0x0228 => Some(Fixed(18)),
        0x0229 => Some(Fixed(15)),
        0x022a => Some(Fixed(58)),
        0x022b => Some(Fixed(57)),
        0x022c => Some(Fixed(65)),
        0x022d => Some(Fixed(5)),
        0x022e => Some(Fixed(71)),
        0x022f => Some(Fixed(5)),
        0x0231 => Some(Fixed(26)),
        0x0232 => Some(Fixed(9)),
        0x0233 => Some(Fixed(11)),
        0x0234 => Some(Fixed(6)),
        0x023a => Some(Fixed(4)),
        0x023b => Some(Fixed(36)),
        0x023c => Some(Fixed(6)),
        0x023d => Some(Dynamic),
        0x023e => Some(Fixed(8)),
        0x023f => Some(Fixed(2)),
        0x0240 => Some(Dynamic),
        0x0241 => Some(Fixed(6)),
        0x0242 => Some(Dynamic),
        0x0243 => Some(Fixed(6)),
        0x0244 => Some(Fixed(6)),
        0x0245 => Some(Fixed(3)),
        0x0246 => Some(Fixed(4)),
        0x0247 => Some(Fixed(8)),
        0x0248 => Some(Dynamic),
        0x0249 => Some(Fixed(3)),
        0x024a => Some(Fixed(70)),
        0x024b => Some(Fixed(4)),
        0x024c => Some(Fixed(8)),
        0x024d => Some(Fixed(12)),
        0x024e => Some(Fixed(6)),
        0x024f => Some(Fixed(10)),
        0x0250 => Some(Fixed(3)),
        0x0251 => Some(Fixed(34)),
        0x0252 => Some(Dynamic),
        0x0253 => Some(Fixed(3)),
        0x0254 => Some(Fixed(3)),
        0x0255 => Some(Fixed(5)),
        0x0256 => Some(Fixed(5)),
        0x0257 => Some(Fixed(8)),
        0x0258 => Some(Fixed(2)),
        0x0259 => Some(Fixed(3)),
        0x025b => Some(Fixed(8)),
        0x025c => Some(Fixed(4)),
        0x025d => Some(Fixed(6)),
        0x025e => Some(Fixed(4)),
        0x025f => Some(Fixed(6)),
        0x0260 => Some(Fixed(6)),
        0x0261 => Some(Fixed(11)),
        0x0262 => Some(Fixed(11)),
        0x0263 => Some(Fixed(11)),
        0x0264 => Some(Fixed(20)),
        0x0265 => Some(Fixed(20)),
        0x0266 => Some(Fixed(30)),
        0x0267 => Some(Fixed(4)),
        0x0268 => Some(Fixed(4)),
        0x0269 => Some(Fixed(4)),
        0x026a => Some(Fixed(4)),
        0x026b => Some(Fixed(4)),
        0x026c => Some(Fixed(4)),
        0x026d => Some(Fixed(4)),
        0x026f => Some(Fixed(2)),
        0x0270 => Some(Fixed(2)),
        0x0271 => Some(Fixed(40)),
        0x0272 => Some(Fixed(44)),
        0x0273 => Some(Fixed(30)),
        0x0274 => Some(Fixed(8)),
        0x0277 => Some(Fixed(84)), // CA_LOGIN_PCBANG
        0x0278 => Some(Fixed(2)),
        0x0279 => Some(Fixed(2)),
        0x027a => Some(Dynamic),
        0x027b => Some(Fixed(14)),
        0x027c => Some(Fixed(84)), // CA_LOGIN4
        0x027d => Some(Fixed(62)),
        0x027e => Some(Dynamic),
        0x027f => Some(Fixed(8)),
        0x0280 => Some(Fixed(12)),
        0x0281 => Some(Dynamic),
        0x0282 => Some(Fixed(284)),
        0x0283 => Some(Fixed(6)),
        0x0284 => Some(Fixed(14)),
        0x0285 => Some(Fixed(6)),
        0x0286 => Some(Fixed(4)),
        0x0287 => Some(Dynamic),
        0x0288 => Some(Dynamic),
        0x0289 => Some(Fixed(12)),
        0x028b => Some(Dynamic),
        0x028c => Some(Fixed(46)),
        0x028d => Some(Fixed(26)), // CH_REQ_IS_VALID_CHARNAME
        0x028e => Some(Fixed(30)), // HC_ACK_IS_VALID_CHARNAME
        0x028f => Some(Fixed(6)),
        0x0290 => Some(Fixed(4)),
        0x0291 => Some(Fixed(4)), // ZC_MSG (specs/equipment)
        0x0292 => Some(Fixed(2)),
        0x0293 => Some(Fixed(70)),
        0x0294 => Some(Fixed(10)),
        0x029b => Some(Fixed(80)),
        0x029c => Some(Fixed(66)),
        0x029d => Some(Dynamic),
        0x029e => Some(Fixed(11)),
        0x029f => Some(Fixed(3)),
        0x02a0 => Some(Dynamic),
        0x02a1 => Some(Dynamic),
        0x02a2 => Some(Fixed(8)),
        0x02a3 => Some(Dynamic),
        0x02a4 => Some(Dynamic),
        0x02a5 => Some(Fixed(8)),
        0x02a6 => Some(Dynamic),
        0x02a7 => Some(Dynamic),
        0x02a8 => Some(Fixed(162)),
        0x02a9 => Some(Fixed(58)),
        0x02aa => Some(Fixed(4)),
        0x02ab => Some(Fixed(36)),
        0x02ac => Some(Fixed(6)),
        0x02ad => Some(Fixed(8)),
        0x02b0 => Some(Fixed(84)), // CA_LOGIN_CHANNEL
        0x02b1 => Some(Dynamic),
        0x02b2 => Some(Dynamic),
        0x02b3 => Some(Fixed(107)),
        0x02b4 => Some(Fixed(6)),
        0x02b5 => Some(Dynamic),
        0x02b6 => Some(Fixed(7)),
        0x02b7 => Some(Fixed(7)),
        0x02b9 => Some(Fixed(191)),
        0x02ba => Some(Fixed(11)),
        0x02bc => Some(Fixed(6)),
        0x02bf => Some(Dynamic),
        0x02c0 => Some(Dynamic),
        0x02c1 => Some(Dynamic),
        0x02c2 => Some(Dynamic),
        0x02c4 => Some(Fixed(26)),
        0x02c5 => Some(Fixed(30)),
        0x02c7 => Some(Fixed(7)),
        0x02c8 => Some(Fixed(3)),
        0x02c9 => Some(Fixed(3)), // ZC_PARTY_CONFIG (specs/map-entry-settings)
        0x02ca => Some(Fixed(3)),
        0x02cb => Some(Fixed(65)),
        0x02cc => Some(Fixed(4)),
        0x02cd => Some(Fixed(71)),
        0x02ce => Some(Fixed(10)),
        0x02cf => Some(Fixed(6)),
        0x02d5 => Some(Fixed(2)),
        0x02d6 => Some(Fixed(6)),
        0x02d8 => Some(Fixed(10)),
        0x02d9 => Some(Fixed(10)),
        0x02da => Some(Fixed(3)), // ZC_CONFIG_NOTIFY (specs/map-entry-settings)
        0x02db => Some(Dynamic),
        0x02dc => Some(Dynamic),
        0x02dd => Some(Fixed(32)),
        0x02de => Some(Fixed(6)),
        0x02df => Some(Fixed(36)),
        0x02e0 => Some(Fixed(34)),
        0x02e2 => Some(Fixed(20)),
        0x02e3 => Some(Fixed(22)),
        0x02e4 => Some(Fixed(11)),
        0x02e5 => Some(Fixed(9)),
        0x02e6 => Some(Fixed(6)),
        0x02e7 => Some(Dynamic),
        0x02eb => Some(Fixed(13)), // ZC_ACCEPT_ENTER
        0x02ec => Some(Fixed(67)),
        0x02ed => Some(Fixed(59)),
        0x02ee => Some(Fixed(60)),
        0x02ef => Some(Fixed(8)),
        0x02f0 => Some(Fixed(10)),
        0x02f1 => Some(Fixed(2)),
        0x02f2 => Some(Fixed(2)),
        0x02f3 => Some(Dynamic),
        0x02f4 => Some(Dynamic),
        0x02f5 => Some(Dynamic),
        0x02f6 => Some(Dynamic),
        0x02f7 => Some(Dynamic),
        0x02f8 => Some(Dynamic),
        0x02f9 => Some(Dynamic),
        0x02fa => Some(Dynamic),
        0x02fb => Some(Dynamic),
        0x02fc => Some(Dynamic),
        0x02fd => Some(Dynamic),
        0x02fe => Some(Dynamic),
        0x02ff => Some(Dynamic),
        0x0300 => Some(Dynamic),
        0x0301 => Some(Dynamic),
        0x0302 => Some(Dynamic),
        0x0303 => Some(Dynamic),
        0x0304 => Some(Dynamic),
        0x0305 => Some(Dynamic),
        0x0306 => Some(Dynamic),
        0x0307 => Some(Dynamic),
        0x0308 => Some(Dynamic),
        0x0309 => Some(Dynamic),
        0x030a => Some(Dynamic),
        0x030b => Some(Dynamic),
        0x030c => Some(Dynamic),
        0x030d => Some(Dynamic),
        0x030e => Some(Dynamic),
        0x030f => Some(Dynamic),
        0x0310 => Some(Dynamic),
        0x0311 => Some(Dynamic),
        0x0312 => Some(Dynamic),
        0x0313 => Some(Dynamic),
        0x0314 => Some(Dynamic),
        0x0315 => Some(Dynamic),
        0x0316 => Some(Dynamic),
        0x0317 => Some(Dynamic),
        0x0318 => Some(Dynamic),
        0x0319 => Some(Dynamic),
        0x031a => Some(Dynamic),
        0x031b => Some(Dynamic),
        0x031c => Some(Dynamic),
        0x031d => Some(Dynamic),
        0x031e => Some(Dynamic),
        0x031f => Some(Dynamic),
        0x0320 => Some(Dynamic),
        0x0321 => Some(Dynamic),
        0x0322 => Some(Dynamic),
        0x0323 => Some(Dynamic),
        0x0324 => Some(Dynamic),
        0x0325 => Some(Dynamic),
        0x0326 => Some(Dynamic),
        0x0327 => Some(Dynamic),
        0x0328 => Some(Dynamic),
        0x0329 => Some(Dynamic),
        0x032a => Some(Dynamic),
        0x032b => Some(Dynamic),
        0x032c => Some(Dynamic),
        0x032d => Some(Dynamic),
        0x032e => Some(Dynamic),
        0x032f => Some(Dynamic),
        0x0330 => Some(Dynamic),
        0x0331 => Some(Dynamic),
        0x0332 => Some(Dynamic),
        0x0333 => Some(Dynamic),
        0x0334 => Some(Dynamic),
        0x0335 => Some(Dynamic),
        0x0336 => Some(Dynamic),
        0x0337 => Some(Dynamic),
        0x0338 => Some(Dynamic),
        0x0339 => Some(Dynamic),
        0x033a => Some(Dynamic),
        0x033b => Some(Dynamic),
        0x033c => Some(Dynamic),
        0x033d => Some(Dynamic),
        0x033e => Some(Dynamic),
        0x033f => Some(Dynamic),
        0x0340 => Some(Dynamic),
        0x0341 => Some(Dynamic),
        0x0342 => Some(Dynamic),
        0x0343 => Some(Dynamic),
        0x0344 => Some(Dynamic),
        0x0345 => Some(Dynamic),
        0x0346 => Some(Dynamic),
        0x0347 => Some(Dynamic),
        0x0348 => Some(Dynamic),
        0x0349 => Some(Dynamic),
        0x034a => Some(Dynamic),
        0x034b => Some(Dynamic),
        0x034c => Some(Dynamic),
        0x034d => Some(Dynamic),
        0x034e => Some(Dynamic),
        0x034f => Some(Dynamic),
        0x0350 => Some(Dynamic),
        0x0351 => Some(Dynamic),
        0x0352 => Some(Dynamic),
        0x0353 => Some(Dynamic),
        0x0354 => Some(Dynamic),
        0x0355 => Some(Dynamic),
        0x0356 => Some(Dynamic),
        0x0357 => Some(Dynamic),
        0x0358 => Some(Dynamic),
        0x0359 => Some(Dynamic),
        0x035a => Some(Dynamic),
        0x035b => Some(Dynamic),
        0x035c => Some(Fixed(2)),
        0x035d => Some(Dynamic),
        0x035e => Some(Fixed(2)),
        0x035f => Some(Fixed(5)),  // CZ_REQUEST_MOVE
        0x0360 => Some(Fixed(6)),  // CZ_REQUEST_TIME
        0x0361 => Some(Fixed(5)),  // CZ_CHANGE_DIRECTION
        0x0362 => Some(Fixed(6)),  // CZ_TAKE_ITEM
        0x0363 => Some(Fixed(6)),  // CZ_DROP_ITEM
        0x0364 => Some(Fixed(8)),  // CZ_MOVE_TO_KAFRA
        0x0365 => Some(Fixed(8)),  // CZ_MOVE_FROM_KAFRA
        0x0366 => Some(Fixed(10)), // CZ_USE_SKILL_TO_POS
        0x0367 => Some(Fixed(31)), // CZ_USE_SKILL_TO_POS_MOREINFO
        0x0368 => Some(Fixed(6)),  // CZ_REQNAME
        0x0369 => Some(Fixed(6)),  // CZ_SOLVE_CHAR_NAME
        0x0389 => Some(Dynamic),
        0x040c => Some(Dynamic),
        0x040d => Some(Dynamic),
        0x040e => Some(Dynamic),
        0x040f => Some(Dynamic),
        0x0410 => Some(Dynamic),
        0x0411 => Some(Dynamic),
        0x0412 => Some(Dynamic),
        0x0413 => Some(Dynamic),
        0x0414 => Some(Dynamic),
        0x0415 => Some(Dynamic),
        0x0416 => Some(Dynamic),
        0x0417 => Some(Dynamic),
        0x0418 => Some(Dynamic),
        0x0419 => Some(Dynamic),
        0x041a => Some(Dynamic),
        0x041b => Some(Dynamic),
        0x041c => Some(Dynamic),
        0x041d => Some(Dynamic),
        0x041e => Some(Dynamic),
        0x041f => Some(Dynamic),
        0x0420 => Some(Dynamic),
        0x0421 => Some(Dynamic),
        0x0422 => Some(Dynamic),
        0x0423 => Some(Dynamic),
        0x0424 => Some(Dynamic),
        0x0425 => Some(Dynamic),
        0x0426 => Some(Dynamic),
        0x0427 => Some(Dynamic),
        0x0428 => Some(Dynamic),
        0x0429 => Some(Dynamic),
        0x042a => Some(Dynamic),
        0x042b => Some(Dynamic),
        0x042c => Some(Dynamic),
        0x042d => Some(Dynamic),
        0x042e => Some(Dynamic),
        0x042f => Some(Dynamic),
        0x0430 => Some(Dynamic),
        0x0431 => Some(Dynamic),
        0x0432 => Some(Dynamic),
        0x0433 => Some(Dynamic),
        0x0434 => Some(Dynamic),
        0x0435 => Some(Dynamic),
        0x0436 => Some(Fixed(23)), // CZ_ENTER
        0x0437 => Some(Fixed(7)),  // CZ_REQUEST_ACT
        0x0438 => Some(Fixed(10)), // CZ_USE_SKILL_TO_ID
        0x0439 => Some(Fixed(8)),
        0x043e => Some(Dynamic),
        0x043f => Some(Fixed(25)),
        0x0441 => Some(Fixed(4)), // ZC_SKILLINFO_DELETE (PF-21)
        0x0443 => Some(Fixed(8)),
        0x0444 => Some(Dynamic),
        0x0445 => Some(Fixed(10)),
        0x0446 => Some(Fixed(14)),
        0x0447 => Some(Fixed(2)),
        0x0448 => Some(Dynamic),
        0x0449 => Some(Fixed(4)),
        0x044a => Some(Fixed(6)),
        0x07d7 => Some(Fixed(8)),
        0x07d8 => Some(Fixed(8)),
        0x07d9 => Some(Fixed(268)),
        0x07da => Some(Fixed(6)),
        0x07e2 => Some(Fixed(8)),
        0x07e3 => Some(Fixed(6)),
        0x07e4 => Some(Dynamic),
        0x07e5 => Some(Fixed(8)),
        0x07e6 => Some(Fixed(8)), // ZC_MSG_SKILL (specs/equipment)
        0x07e7 => Some(Fixed(32)),
        0x07e8 => Some(Dynamic),
        0x07e9 => Some(Fixed(5)),
        0x07ec => Some(Fixed(8)),
        0x07f5 => Some(Fixed(6)),
        0x07f6 => Some(Fixed(14)),
        0x07f7 => Some(Dynamic),
        0x07f8 => Some(Dynamic),
        0x07f9 => Some(Dynamic),
        0x07fa => Some(Fixed(8)), // ZC_DELETE_ITEM_FROM_BODY (PF-21)
        0x07fc => Some(Fixed(10)),
        0x0801 => Some(Dynamic),
        0x0802 => Some(Fixed(18)),
        0x0803 => Some(Fixed(4)),
        0x0804 => Some(Fixed(14)),
        0x0805 => Some(Dynamic),
        0x0806 => Some(Fixed(2)),
        0x0807 => Some(Fixed(4)),
        0x0808 => Some(Fixed(14)),
        0x0809 => Some(Fixed(50)),
        0x080a => Some(Fixed(18)),
        0x080b => Some(Fixed(6)),
        0x080e => Some(Fixed(14)), // ZC_NOTIFY_HP_TO_GROUPM_R2 (captured, PF-21)
        0x0810 => Some(Fixed(3)),
        0x0811 => Some(Dynamic),
        0x0812 => Some(Fixed(8)),
        0x0815 => Some(Fixed(2)),
        0x0817 => Some(Fixed(6)),
        0x0819 => Some(Dynamic),
        0x081a => Some(Fixed(4)),
        0x081b => Some(Fixed(10)),
        0x081c => Some(Fixed(10)),
        0x081d => Some(Fixed(22)),
        0x0820 => Some(Fixed(11)),
        0x0825 => Some(Dynamic),   // CA_SSO_LOGIN_REQ
        0x0827 => Some(Fixed(6)),  // CH_DELETE_CHAR3_RESERVED
        0x0828 => Some(Fixed(14)), // HC_DELETE_CHAR3_RESERVED
        0x0829 => Some(Fixed(12)), // CH_DELETE_CHAR3
        0x082a => Some(Fixed(10)), // HC_DELETE_CHAR3
        0x082b => Some(Fixed(6)),  // CH_DELETE_CHAR3_CANCEL
        0x082c => Some(Fixed(10)), // HC_DELETE_CHAR3_CANCEL
        0x082d => Some(Fixed(29)), // HC_ACCEPT_ENTER2
        0x0835 => Some(Dynamic),
        0x0838 => Some(Fixed(2)),
        0x083b => Some(Fixed(2)),
        0x083c => Some(Fixed(14)), // CZ_SSILIST_ITEM_CLICK
        0x083e => Some(Fixed(26)), // AC_REFUSE_LOGIN
        0x0840 => Some(Dynamic),   // HC_NOTIFY_ACCESSIBLE_MAPNAME
        0x0841 => Some(Fixed(6)),  // CH_SELECT_ACCESSIBLE_MAPNAME
        0x0842 => Some(Fixed(6)),
        0x0843 => Some(Fixed(6)),
        0x0844 => Some(Fixed(2)),
        0x0846 => Some(Fixed(4)),
        0x0848 => Some(Dynamic),
        0x0849 => Some(Fixed(16)),
        0x084a => Some(Fixed(2)),
        0x084b => Some(Fixed(19)),
        0x0856 => Some(Dynamic),
        0x0857 => Some(Dynamic),
        0x0858 => Some(Dynamic),
        0x085a => Some(Fixed(90)),
        0x085d => Some(Fixed(18)),
        0x0861 => Some(Fixed(8)),
        0x0862 => Some(Fixed(10)),
        0x0863 => Some(Fixed(10)),
        0x0865 => Some(Fixed(6)),
        0x0868 => Some(Dynamic),
        0x086a => Some(Fixed(19)),
        0x086c => Some(Fixed(8)),
        0x086d => Some(Fixed(26)),
        0x086f => Some(Fixed(26)),
        0x0870 => Some(Dynamic),
        0x0871 => Some(Fixed(5)),
        0x0874 => Some(Fixed(8)),
        0x0879 => Some(Fixed(41)),
        0x0881 => Some(Fixed(5)),
        0x0884 => Some(Fixed(6)),
        0x0885 => Some(Fixed(5)),
        0x0886 => Some(Fixed(2)),
        0x0887 => Some(Fixed(6)),
        0x0888 => Some(Fixed(19)),
        0x0889 => Some(Fixed(90)),
        0x088a => Some(Fixed(6)),
        0x088b => Some(Fixed(2)),
        0x088d => Some(Fixed(26)),
        0x088e => Some(Fixed(7)),
        0x0890 => Some(Fixed(5)),
        0x0891 => Some(Fixed(6)),
        0x0893 => Some(Fixed(8)),
        0x0897 => Some(Fixed(5)),
        0x0898 => Some(Fixed(6)),
        0x089b => Some(Fixed(10)),
        0x089c => Some(Fixed(26)),
        0x089e => Some(Fixed(6)),
        0x089f => Some(Fixed(6)),
        0x08a0 => Some(Fixed(8)),
        0x08a1 => Some(Fixed(6)),
        0x08a2 => Some(Fixed(14)),
        0x08a5 => Some(Fixed(18)),
        0x08a6 => Some(Fixed(8)),
        0x08a8 => Some(Fixed(36)),
        0x08aa => Some(Fixed(7)),
        0x08ab => Some(Dynamic),
        0x08ac => Some(Fixed(8)),
        0x08ad => Some(Fixed(90)),
        0x08b3 => Some(Dynamic),
        0x08b8 => Some(Fixed(8)),  // CH_SECOND_PASSWD_ACK
        0x08b9 => Some(Fixed(12)), // HC_SECOND_PASSWD_LOGIN
        0x08ba => Some(Fixed(12)), // CH_MAKE_SECOND_PASSWD
        0x08bb => Some(Fixed(16)), // CH_EDIT_SECOND_PASSWD
        0x08bc => Some(Fixed(2)),  // CH_NOT_AVAILABLE_SECOND_PASSWD
        0x08bd => Some(Fixed(2)),  // CH_AVAILABLE_SECOND_PASSWD
        0x08c7 => Some(Fixed(20)),
        0x08c8 => Some(Fixed(34)), // ZC_NOTIFY_ACT
        0x08c9 => Some(Fixed(2)),
        0x08d2 => Some(Fixed(10)),
        0x08d7 => Some(Fixed(28)),
        0x08d8 => Some(Fixed(27)),
        0x08d9 => Some(Fixed(30)),
        0x08da => Some(Fixed(26)),
        0x08db => Some(Fixed(27)),
        0x08dc => Some(Fixed(26)),
        0x08dd => Some(Fixed(27)),
        0x08de => Some(Fixed(27)),
        0x08df => Some(Fixed(50)),
        0x08e0 => Some(Fixed(51)),
        0x08e1 => Some(Fixed(51)),
        0x08e3 => Some(Fixed(149)),
        0x08e5 => Some(Fixed(41)),
        0x08e6 => Some(Fixed(4)),
        0x08e7 => Some(Fixed(10)),
        0x08e8 => Some(Dynamic),
        0x08e9 => Some(Fixed(2)),
        0x08ea => Some(Fixed(4)),
        0x08eb => Some(Fixed(39)),
        0x08ec => Some(Fixed(73)),
        0x08ed => Some(Fixed(43)),
        0x08ee => Some(Fixed(6)),
        0x08ef => Some(Fixed(6)),
        0x08f0 => Some(Fixed(6)),
        0x08f1 => Some(Fixed(6)),
        0x08f2 => Some(Fixed(36)),
        0x08f3 => Some(Dynamic),
        0x08f4 => Some(Fixed(6)),
        0x08f5 => Some(Dynamic),
        0x08f6 => Some(Fixed(22)),
        0x08f7 => Some(Fixed(3)),
        0x08f8 => Some(Fixed(7)),
        0x08f9 => Some(Fixed(6)),
        0x08fa => Some(Fixed(6)),
        0x08fb => Some(Fixed(6)),
        0x08fc => Some(Fixed(46)), // CH_REQ_CHANGE_CHARNAME
        0x08fd => Some(Fixed(4)),  // HC_ACK_CHANGE_CHARNAME
        0x08fe => Some(Dynamic),
        0x0907 => Some(Fixed(5)),
        0x090a => Some(Fixed(26)),
        0x090f => Some(Dynamic),
        0x0914 => Some(Dynamic),
        0x0915 => Some(Dynamic),
        0x0916 => Some(Fixed(26)),
        0x091c => Some(Fixed(26)),
        0x091d => Some(Fixed(41)),
        0x0922 => Some(Dynamic),
        0x0929 => Some(Fixed(26)),
        0x0933 => Some(Fixed(6)),
        0x0938 => Some(Dynamic),
        0x093b => Some(Fixed(8)),
        0x093f => Some(Fixed(5)),
        0x0945 => Some(Dynamic),
        0x0947 => Some(Fixed(36)),
        0x094a => Some(Fixed(6)),
        0x094b => Some(Fixed(19)),
        0x094c => Some(Fixed(6)),
        0x094e => Some(Dynamic),
        0x0953 => Some(Fixed(5)),
        0x0959 => Some(Fixed(10)),
        0x0960 => Some(Fixed(5)),
        0x0961 => Some(Fixed(36)),
        0x0963 => Some(Fixed(8)),
        0x096a => Some(Fixed(6)),
        0x096e => Some(Dynamic),
        0x0974 => Some(Fixed(2)),
        0x0977 => Some(Fixed(14)),
        0x0978 => Some(Fixed(6)),
        0x0979 => Some(Fixed(50)),
        0x097a => Some(Dynamic),
        0x097c => Some(Fixed(4)),
        0x097f => Some(Dynamic),
        0x0980 => Some(Fixed(7)),
        0x0983 => Some(Fixed(29)),
        0x098d => Some(Dynamic),
        0x0998 => Some(Fixed(8)),
        0x0999 => Some(Fixed(11)), // ZC_ACK_WEAR_EQUIP_V5 (captured)
        0x099a => Some(Fixed(9)),  // ZC_ACK_TAKEOFF_EQUIP_V5 (captured)
        0x099b => Some(Fixed(8)),
        0x099f => Some(Fixed(22)),
        0x09a0 => Some(Fixed(6)), // HC_CHARLIST_NOTIFY
        0x09a1 => Some(Fixed(2)), // CH_CHARLIST_REQ
        0x09a2 => Some(Fixed(6)), // CH_REQ_CHANGE_CHARACTER_SLOT
        0x09a7 => Some(Fixed(10)),
        0x09a9 => Some(Fixed(10)),
        0x09ab => Some(Fixed(6)),
        0x09ac => Some(Dynamic),
        0x09ae => Some(Fixed(20)),
        0x09af => Some(Fixed(4)),
        0x09b0 => Some(Fixed(10)),
        0x09b1 => Some(Fixed(4)),
        0x09b4 => Some(Fixed(6)),
        0x09b6 => Some(Fixed(6)),
        0x09b8 => Some(Fixed(6)),
        0x09bc => Some(Fixed(6)),
        0x09c3 => Some(Fixed(8)),
        0x09ca => Some(Fixed(23)),
        0x09cb => Some(Fixed(17)), // ZC_USE_SKILL2 (PF-21)
        0x09cd => Some(Fixed(8)),  // ZC_MSG_COLOR (specs/equipment)
        0x09ce => Some(Fixed(102)),
        0x09d1 => Some(Fixed(14)),
        0x09d4 => Some(Fixed(2)),
        0x09d6 => Some(Dynamic),
        0x09d7 => Some(Dynamic),
        0x09d8 => Some(Fixed(2)),
        0x09db => Some(Dynamic),
        0x09dc => Some(Dynamic),
        0x09dd => Some(Dynamic),
        0x09e6 => Some(Fixed(22)),
        0x09e7 => Some(Fixed(3)),
        0x09e8 => Some(Fixed(11)),
        0x09e9 => Some(Fixed(2)),
        0x09ea => Some(Fixed(11)),
        0x09ec => Some(Dynamic),
        0x09ed => Some(Fixed(3)),
        0x09ee => Some(Fixed(11)),
        0x09ef => Some(Fixed(11)),
        0x09f0 => Some(Dynamic),
        0x09f1 => Some(Fixed(11)),
        0x09f2 => Some(Fixed(12)),
        0x09f3 => Some(Fixed(11)),
        0x09f4 => Some(Fixed(12)),
        0x09f5 => Some(Fixed(11)),
        0x09f6 => Some(Fixed(11)),
        0x09f7 => Some(Fixed(75)),
        0x09f8 => Some(Dynamic),
        0x09f9 => Some(Fixed(143)),
        0x09fa => Some(Dynamic),
        0x09fb => Some(Dynamic),
        0x09fc => Some(Fixed(6)),
        0x09fd => Some(Dynamic), // ZC_NOTIFY_MOVEENTRY11 (rAthena writes 114)
        0x09fe => Some(Dynamic), // ZC_NOTIFY_NEWENTRY11 (rAthena writes 107)
        0x09ff => Some(Dynamic), // ZC_NOTIFY_STANDENTRY11 (rAthena writes 108)
        0x0a00 => Some(Fixed(269)),
        0x0a01 => Some(Fixed(3)),
        0x0a03 => Some(Fixed(2)),
        0x0a04 => Some(Fixed(6)),
        0x0a06 => Some(Fixed(6)),
        0x0a07 => Some(Fixed(9)),
        0x0a08 => Some(Fixed(26)),
        0x0a0e => Some(Fixed(14)),
        0x0a12 => Some(Fixed(27)),
        0x0a13 => Some(Fixed(26)),
        0x0a14 => Some(Fixed(10)),
        0x0a19 => Some(Fixed(2)),
        0x0a1a => Some(Fixed(23)),
        0x0a1b => Some(Fixed(2)),
        0x0a1c => Some(Dynamic),
        0x0a1d => Some(Fixed(2)),
        0x0a1e => Some(Fixed(3)),
        0x0a1f => Some(Fixed(2)),
        0x0a20 => Some(Fixed(23)),
        0x0a21 => Some(Fixed(3)),
        0x0a22 => Some(Fixed(5)),
        0x0a23 => Some(Dynamic),
        0x0a24 => Some(Fixed(66)),
        0x0a25 => Some(Fixed(6)),
        0x0a26 => Some(Fixed(7)),
        0x0a2e => Some(Fixed(6)),
        0x0a2f => Some(Fixed(7)),
        0x0a30 => Some(Fixed(106)), // ZC_ACK_REQNAMEALL (struct size)
        0x0a32 => Some(Fixed(2)),
        0x0a35 => Some(Fixed(4)),
        0x0a39 => Some(Fixed(36)), // CH_MAKE_CHAR
        0x0a3d => Some(Fixed(20)),
        0x0a44 => Some(Dynamic),
        0x0a46 => Some(Fixed(14)),
        0x0a48 => Some(Fixed(2)),
        0x0a49 => Some(Fixed(22)),
        0x0a4a => Some(Fixed(6)),
        0x0a4b => Some(Fixed(22)),
        0x0a4c => Some(Fixed(28)),
        0x0a4f => Some(Dynamic),
        0x0a51 => Some(Fixed(34)),
        0x0a52 => Some(Fixed(20)),
        0x0a54 => Some(Dynamic),
        0x0a56 => Some(Fixed(6)),
        0x0a5a => Some(Fixed(2)),
        0x0a5c => Some(Fixed(18)),
        0x0a68 => Some(Fixed(3)),
        0x0a69 => Some(Fixed(6)),
        0x0a6c => Some(Fixed(7)),
        0x0a6e => Some(Dynamic),
        0x0a70 => Some(Fixed(2)),
        0x0a77 => Some(Fixed(15)),
        0x0a78 => Some(Fixed(15)),
        0x0a7d => Some(Dynamic),
        0x0a88 => Some(Fixed(2)),
        0x0a97 => Some(Fixed(8)),
        0x0a98 => Some(Fixed(10)),
        0x0a99 => Some(Fixed(4)),
        0x0a9a => Some(Fixed(10)),
        0x0a9b => Some(Dynamic),
        0x0a9c => Some(Fixed(2)),
        0x0a9d => Some(Fixed(4)),
        0x0aa1 => Some(Fixed(4)),
        0x0aa3 => Some(Fixed(9)),
        0x0aa4 => Some(Fixed(2)),
        0x0aa5 => Some(Dynamic),
        0x0ab5 => Some(Fixed(2)),
        0x0ab6 => Some(Fixed(8)),
        0x0ac0 => Some(Fixed(26)),
        0x0ac1 => Some(Fixed(26)),
        0x0ac4 => Some(Dynamic),    // AC_ACCEPT_LOGIN
        0x0ac5 => Some(Fixed(156)), // HC_NOTIFY_ZONESVR
        0x0acb => Some(Fixed(12)),  // ZC_LONGLONGPAR_CHANGE (specs/map-entry-settings)
        0x0acc => Some(Fixed(18)),
        0x0ace => Some(Fixed(4)),
        0x0add => Some(Fixed(24)), // ZC_ITEM_FALL_ENTRY (captured)
        0x0ade => Some(Fixed(6)),
        0x0adf => Some(Fixed(58)), // ZC_ACK_REQNAMEALL_NPC (struct size)
        0x0ae6 => Some(Fixed(10)),
        0x0ae8 => Some(Fixed(2)),
        0x0aef => Some(Fixed(2)),
        0x0af0 => Some(Fixed(10)),
        0x0af4 => Some(Fixed(11)),
        0x0af7 => Some(Fixed(32)), // ZC_ACK_REQNAME_BYGID
        0x0af8 => Some(Fixed(11)),
        0x0afc => Some(Fixed(16)),
        0x0b08 => Some(Dynamic), // ZC_INVENTORY_START
        0x0b09 => Some(Dynamic),
        0x0b0b => Some(Fixed(4)), // ZC_INVENTORY_END
        0x0b0f => Some(Dynamic),
        0x0b10 => Some(Fixed(10)),
        0x0b11 => Some(Fixed(4)),
        0x0b12 => Some(Fixed(2)),
        0x0b14 => Some(Fixed(2)),
        0x0b16 => Some(Fixed(2)),
        0x0b18 => Some(Fixed(4)), // ZC_INVENTORY_EXPANSION_INFO
        0x0b19 => Some(Fixed(2)),
        0x0b1a => Some(Fixed(29)), // ZC_USESKILL_ACK (PF-21)
        0x0b1b => Some(Fixed(2)),  // ZC_LOAD_CONFIRM (specs/map-entry-settings)
        0x0b1c => Some(Fixed(2)),
        0x0b1d => Some(Fixed(2)), // ZC_PING_LIVE (specs/map-entry-settings)
        0x0b20 => Some(Fixed(271)), // ZC_SHORTCUT_KEY_LIST_V4 (specs/map-entry-settings)
        0x0b21 => Some(Fixed(13)),
        0x0b22 => Some(Fixed(5)),
        0x0b24 => Some(Fixed(6)),
        0x0b28 => Some(Fixed(3)),
        0x0b2c => Some(Fixed(3)),
        0x0b35 => Some(Fixed(3)),
        0x0b39 => Some(Dynamic),
        0x0b41 => Some(Fixed(70)), // ZC_ITEM_PICKUP_ACK
        0x0b46 => Some(Fixed(10)),
        0x0b4c => Some(Fixed(2)),
        0x0b57 => Some(Dynamic),
        0x0b58 => Some(Fixed(2)),
        0x0b59 => Some(Fixed(4)),
        0x0b5b => Some(Fixed(14)),
        0x0b5c => Some(Fixed(2)),
        0x0b66 => Some(Fixed(26)),
        0x0b6d => Some(Fixed(6)),
        0x0b6f => Some(Fixed(177)), // HC_ACCEPT_MAKECHAR
        0x0b70 => Some(Dynamic),    // HC_ACK_CHANGE_CHARACTER_SLOT
        0x0b72 => Some(Dynamic),    // HC_ACK_CHARINFO_PER_PAGE
        0x0b8d => Some(Dynamic),    // ZC_REPUTE_INFO (specs/map-entry-settings)
        0x0b90 => Some(Fixed(2)),
        0x0b91 => Some(Fixed(8)),
        0x0b93 => Some(Fixed(12)),
        0x0b97 => Some(Fixed(27)),
        0x0b98 => Some(Fixed(6)),
        0x0b9b => Some(Fixed(12)),
        0x0b9c => Some(Fixed(16)),
        0x0b9d => Some(Fixed(14)),
        0x0b9e => Some(Fixed(12)),
        0x0ba0 => Some(Fixed(2)),
        0x0bae => Some(Fixed(3)), // ZC_ACK_TAKEOFF_EQUIP_ALL (specs/equipment)
        0x0baf => Some(Fixed(16)),
        0x0bb0 => Some(Fixed(9)),
        0x0bb1 => Some(Fixed(3)),
        0x0bdd => Some(Dynamic),
        0x0be2 => Some(Fixed(137)),
        0x0be9 => Some(Fixed(6)),
        0x0bf3 => Some(Dynamic),
        0x0bf5 => Some(Fixed(6)),
        0x0c0b => Some(Fixed(18)),
        0x0c22 => Some(Fixed(12)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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
}
