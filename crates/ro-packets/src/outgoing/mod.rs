//! Outgoing server packet encoders and serializers.
//!
//! Covers:
//! - `AC_ACCEPT_LOGIN` (0x0ac4)
//! - `AC_REFUSE_LOGIN` (0x083e)
//! - `HC_ACCEPT_ENTER` (0x006b)
//! - `HC_REFUSE_ENTER` (0x006c)
//! - `HC_ACCEPT_ENTER2` (0x082d)
//! - `HC_ACK_CHARINFO_PER_PAGE` (0x0b72)
//! - `HC_ACCEPT_MAKECHAR` (0x0b6f)
//! - `HC_REFUSE_MAKECHAR` (0x006e)
//! - `HC_DELETE_CHAR3_RESERVED` (0x0828)
//! - `HC_DELETE_CHAR3_CANCEL` (0x082c)
//! - `HC_NOTIFY_ZONESVR` (0x0ac5)
//! - `ZC_AID` (0x0283)
//! - `ZC_INVENTORY_EXPANSION_INFO` (0x0b18)
//! - `ZC_ACCEPT_ENTER` (0x02eb)
//! - `ZC_NOTIFY_TIME` (0x007f)
//! - `ZC_NOTIFY_NEWENTRY11` (0x09fe)
//! - `ZC_NOTIFY_STANDENTRY11` (0x09ff)
//! - `ZC_NOTIFY_MOVEENTRY11` (0x09fd)
//! - `ZC_NOTIFY_PLAYERMOVE` (0x0087)
//! - `ZC_STOPMOVE` (0x0088)
//! - `ZC_NPCACK_MAPMOVE` (0x0091)
//! - `ZC_CHANGE_DIRECTION` (0x009c)
//! - `ZC_ACK_REQNAMEALL` (0x0a30)
//! - `ZC_NOTIFY_VANISH` (0x0080)
//! - `ZC_PAR_CHANGE` (0x00b0)
//! - `ZC_NOTIFY_ACT` (0x08c8)
//! - `ZC_RESTART_ACK` (0x00b3)
//! - `ZC_ACK_REQ_DISCONNECT` (0x018b)
//! - inventory lists (0x0b08, 0x0b09, 0x0b39, 0x0b0b, 0x0a9b), `ZC_ITEM_PICKUP_ACK` (0x0b41),
//!   `ZC_ITEM_THROW_ACK` (0x00af)
//! - floor items (0x0add, 0x009d, 0x00a1)
//! - chat (0x008d, 0x008e)
//! - map-entry settings (0x0201, 0x010f, 0x0b20, 0x02c9, 0x02da, 0x02d9, 0x0b8d) and the load
//!   confirm (0x0b1b)
//! - character status (0x0acb, 0x00bd, 0x0141, 0x013a, 0x00be)
//! - the rest of the entry burst: 0x01d7, 0x099b, 0x09e7, 0x0a24, 0x0ade

pub mod char;
pub mod encoder;
pub mod helpers;
pub mod item;
pub mod login;
pub mod map;
pub mod settings;
pub mod status;

pub use char::{
    encode_character_info, encode_hc_accept_enter, encode_hc_accept_enter2,
    encode_hc_accept_makechar, encode_hc_ack_charinfo_per_page, encode_hc_delete_char3_cancel,
    encode_hc_delete_char3_reserved, encode_hc_notify_zonesvr, encode_hc_refuse_enter,
    encode_hc_refuse_makechar,
};
pub use encoder::{
    is_implemented_outbound_opcode, ModernEventEncoder, Phase0EventEncoder, RathenaEventEncoder,
};
pub use helpers::write_fixed_string;
pub use item::{
    encode_zc_equipment_itemlist, encode_zc_equipswitch_list_empty, encode_zc_inventory_end,
    encode_zc_inventory_start, encode_zc_item_disappear, encode_zc_item_entry,
    encode_zc_item_fall_entry, encode_zc_item_pickup_ack, encode_zc_item_throw_ack,
    encode_zc_normal_itemlist, encode_zc_notify_chat, encode_zc_notify_playerchat,
};
pub use login::{
    encode_ac_accept_login, encode_ac_accept_login_modern, encode_ac_refuse_login,
    encode_ac_refuse_login_modern, CharServerEntry,
};
pub use map::{
    encode_zc_accept_enter, encode_zc_accept_enter2, encode_zc_ack_req_disconnect,
    encode_zc_ack_reqnameall, encode_zc_ack_reqnameall_npc, encode_zc_aid,
    encode_zc_change_direction, encode_zc_inventory_expansion_info, encode_zc_notify_act,
    encode_zc_notify_act_modern, encode_zc_notify_act_params, encode_zc_notify_moveentry11,
    encode_zc_notify_newentry11, encode_zc_notify_playermove, encode_zc_notify_standentry11,
    encode_zc_notify_time, encode_zc_notify_vanish, encode_zc_npcack_mapmove, encode_zc_par_change,
    encode_zc_par_change_i32, encode_zc_restart_ack, encode_zc_sprite_change2, encode_zc_stopmove,
    ActParams,
};
pub use settings::{
    encode_zc_ach_update_totals, encode_zc_config, encode_zc_config_notify,
    encode_zc_friends_list_empty, encode_zc_load_confirm, encode_zc_notify_mapproperty2,
    encode_zc_notify_unreadmail, encode_zc_overweight_percent, encode_zc_party_config,
    encode_zc_repute_info_default, encode_zc_shortcut_key_list_empty, encode_zc_skillinfo_list,
};
pub use status::{
    encode_zc_attack_range, encode_zc_couplestatus, encode_zc_longlongpar_change, encode_zc_status,
    encode_zc_status_change,
};
