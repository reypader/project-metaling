//! Domain crate: `ro-packets`.
//!
//! Provides binary TCP packet framing, coordinate bit-packing,
//! length table lookups, incoming packet decoders, and outgoing packet serializers
//! for Ragnarok Online protocol version `TARGET_PACKETVER = 20260219`.

pub mod codec;
pub mod coords;
pub mod incoming;
pub mod outgoing;
pub mod table;

pub use codec::{PacketDecoder, PacketEncoder};
pub use coords::{
    pack_map_position, pack_pos2, pack_pos_dir, unpack_map_position, unpack_pos2, unpack_pos_dir,
};
pub use incoming::{
    decode_frame, parse_sex, read_fixed_string, CaLoginPacket, ChEnterPacket, ChSelectCharPacket,
    CzChangeDirectionPacket, CzEnterPacket, CzNotifyActorInitPacket, CzReqNamePacket,
    CzRequestActPacket, CzRequestMovePacket, CzRequestTimePacket, DecodedFrame, ModernIntentParser,
    Phase0IntentParser, RathenaIntentParser,
};
pub use outgoing::{
    encode_ac_accept_login, encode_ac_accept_login_modern, encode_ac_refuse_login,
    encode_ac_refuse_login_modern, encode_character_info, encode_hc_accept_enter,
    encode_hc_accept_enter2, encode_hc_ack_charinfo_per_page, encode_hc_notify_zonesvr,
    encode_zc_accept_enter, encode_zc_accept_enter2, encode_zc_notify_act,
    encode_zc_notify_act_modern, encode_zc_notify_act_params, encode_zc_notify_move_modern,
    encode_zc_notify_newentry, encode_zc_notify_newentry_modern, encode_zc_notify_playermove,
    encode_zc_notify_standentry_modern, encode_zc_notify_time, encode_zc_notify_vanish,
    encode_zc_par_change, encode_zc_par_change_i32, is_implemented_outbound_opcode,
    write_fixed_string, ActParams, CharServerEntry, ModernEventEncoder, NewEntryParams,
    Phase0EventEncoder, RathenaEventEncoder,
};
pub use table::{lookup_packet_length, RathenaPacketLengthTable};

pub use ro_contracts;
