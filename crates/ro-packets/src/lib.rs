//! Domain crate: `ro-packets`.
//!
//! Provides binary TCP packet framing, coordinate bit-packing,
//! length table lookups, incoming packet decoders, and outgoing packet serializers
//! for Ragnarok Online protocol version `TARGET_PACKETVER = 20260219`.

mod codec;
mod coords;
mod incoming;
mod opcode;
mod outgoing;
mod table;
mod wire;

pub use codec::{PacketDecoder, PacketEncoder};
pub use coords::{
    pack_map_position, pack_pos2, pack_pos_dir, unpack_map_position, unpack_pos2, unpack_pos_dir,
};
pub use incoming::{
    decode_frame, parse_sex, read_fixed_string, CaLoginPacket, ChCharListReqPacket,
    ChDeleteChar3CancelPacket, ChDeleteChar3ReservedPacket, ChEnterPacket, ChMakeCharPacket,
    ChSelectCharPacket, CzBlockingPlayCancelPacket, CzChangeDirectionPacket, CzEnterPacket,
    CzItemPickupPacket, CzItemThrowPacket, CzNotifyActorInitPacket, CzReqDisconnectPacket,
    CzReqNamePacket, CzReqTakeoffEquipAllPacket, CzReqTakeoffEquipPacket, CzReqWearEquipPacket,
    CzRequestActPacket, CzRequestChatPacket, CzRequestMovePacket, CzRequestTimePacket,
    CzRestartPacket, CzStatusChangePacket, DecodedFrame, ModernIntentParser, Phase0IntentParser,
    PingPacket, RathenaIntentParser,
};
pub use opcode::PacketOpcode;
pub use outgoing::{
    encode_ac_accept_login, encode_ac_accept_login_modern, encode_ac_refuse_login,
    encode_ac_refuse_login_modern, encode_character_info, encode_hc_accept_enter,
    encode_hc_accept_enter2, encode_hc_accept_makechar, encode_hc_ack_charinfo_per_page,
    encode_hc_delete_char3_cancel, encode_hc_delete_char3_reserved, encode_hc_notify_zonesvr,
    encode_hc_refuse_enter, encode_hc_refuse_makechar, encode_zc_accept_enter,
    encode_zc_accept_enter2, encode_zc_ach_update_totals, encode_zc_ack_req_disconnect,
    encode_zc_ack_reqnameall, encode_zc_ack_reqnameall_npc, encode_zc_ack_takeoff_equip_all,
    encode_zc_action_failure, encode_zc_aid, encode_zc_attack_range, encode_zc_change_direction,
    encode_zc_config, encode_zc_config_notify, encode_zc_couplestatus, encode_zc_equip_arrow,
    encode_zc_equipment_itemlist, encode_zc_equipswitch_list_empty, encode_zc_friends_list_empty,
    encode_zc_inventory_end, encode_zc_inventory_expansion_info, encode_zc_inventory_start,
    encode_zc_item_disappear, encode_zc_item_entry, encode_zc_item_fall_entry,
    encode_zc_item_pickup_ack, encode_zc_item_throw_ack, encode_zc_load_confirm,
    encode_zc_longlongpar_change, encode_zc_msg, encode_zc_msg_state_change3,
    encode_zc_normal_itemlist, encode_zc_notify_act, encode_zc_notify_act_modern,
    encode_zc_notify_act_params, encode_zc_notify_chat, encode_zc_notify_effect,
    encode_zc_notify_mapproperty2, encode_zc_notify_moveentry11, encode_zc_notify_newentry11,
    encode_zc_notify_playerchat, encode_zc_notify_playermove, encode_zc_notify_standentry11,
    encode_zc_notify_time, encode_zc_notify_unreadmail, encode_zc_notify_vanish,
    encode_zc_npcack_mapmove, encode_zc_overweight_percent, encode_zc_par_change,
    encode_zc_par_change_i32, encode_zc_party_config, encode_zc_repute_info_default,
    encode_zc_req_takeoff_equip_ack, encode_zc_req_wear_equip_ack, encode_zc_restart_ack,
    encode_zc_shortcut_key_list_empty, encode_zc_skillinfo_list, encode_zc_sprite_change2,
    encode_zc_status, encode_zc_status_change, encode_zc_status_change_ack, encode_zc_stopmove,
    is_implemented_outbound_opcode, write_fixed_string, ActParams, CharServerEntry,
    ModernEventEncoder, Phase0EventEncoder, RathenaEventEncoder,
};
pub use table::{lookup_packet_length, RathenaPacketLengthTable};
pub use wire::{
    EventEncoder, PacketCodecError, PacketIntentParser, PacketLength, PacketLengthTable, RawPacket,
    MAX_PACKET_SIZE,
};
