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
//! - `ZC_NOTIFY_NEWENTRY` (0x09fe)
//! - `ZC_NOTIFY_STANDENTRY` (0x09ff)
//! - `ZC_NOTIFY_MOVE` (0x09fd)
//! - `ZC_NOTIFY_PLAYERMOVE` (0x0087)
//! - `ZC_NOTIFY_VANISH` (0x0080)
//! - `ZC_PAR_CHANGE` (0x00b0)
//! - `ZC_NOTIFY_ACT` (0x08c8)
//! - `ZC_RESTART_ACK` (0x00b3)

pub mod char;
pub mod encoder;
pub mod helpers;
pub mod login;
pub mod map;

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
pub use login::{
    encode_ac_accept_login, encode_ac_accept_login_modern, encode_ac_refuse_login,
    encode_ac_refuse_login_modern, CharServerEntry,
};
pub use map::{
    encode_zc_accept_enter, encode_zc_accept_enter2, encode_zc_aid,
    encode_zc_inventory_expansion_info, encode_zc_notify_act, encode_zc_notify_act_modern,
    encode_zc_notify_act_params, encode_zc_notify_move, encode_zc_notify_move_modern,
    encode_zc_notify_newentry, encode_zc_notify_newentry_modern, encode_zc_notify_playermove,
    encode_zc_notify_standentry, encode_zc_notify_standentry_modern, encode_zc_notify_time,
    encode_zc_notify_vanish, encode_zc_par_change, encode_zc_par_change_i32,
    encode_zc_restart_ack, ActParams, NewEntryParams,
};
