//! Integration tests for packet framing, coordinate packing, and packet codecs.

use bytes::BytesMut;
use ro_core::{
    AccountId, CharId, CharServerInfo, CharacterLooks, CharacterSummary, ClientIntent, MapPosition,
    ServerEvent, Sex, UnitAppearance, UnitLooks, UnitObjectType,
};
use ro_packets::{
    decode_frame, encode_ac_accept_login, encode_ac_refuse_login, encode_hc_notify_zonesvr,
    encode_zc_accept_enter, encode_zc_notify_act, encode_zc_notify_act_params,
    encode_zc_notify_playermove, encode_zc_notify_time, encode_zc_par_change, pack_pos2,
    pack_pos_dir, unpack_pos2, unpack_pos_dir, ActParams, CharServerEntry, DecodedFrame,
    EventEncoder, ModernEventEncoder, PacketDecoder, PacketOpcode, Phase0EventEncoder,
    RathenaPacketLengthTable,
};
use tokio_util::codec::Decoder;

mod login_char;
mod map;
mod stream;
mod table_lengths;
