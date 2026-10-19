//! Incoming client packet decoders supporting PACKETVER 20260219 opcodes.
//!
//! Covers:
//! - `CA_LOGIN` (0x0064 / 0x0277)
//! - `CH_ENTER` (0x0065)
//! - `CH_SELECT_CHAR` (0x0066)
//! - `CH_MAKE_CHAR` (0x0a39)
//! - `CH_DELETE_CHAR3_RESERVED` (0x0827)
//! - `CH_DELETE_CHAR3_CANCEL` (0x082b)
//! - `CH_CHARLIST_REQ` (0x09a1)
//! - `PING` (0x0187)
//! - `CZ_ENTER` (0x0436)
//! - `CZ_NOTIFY_ACTORINIT` (0x007d)
//! - `CZ_REQUEST_TIME` (0x0360)
//! - `CZ_REQUEST_MOVE` (0x035f)
//! - `CZ_CHANGE_DIRECTION` (0x0361)
//! - `CZ_REQUEST_ACT` (0x0437)
//! - `CZ_REQNAME` (0x0368)
//! - `CZ_RESTART` (0x00b2)

pub mod char;
pub mod frame;
pub mod helpers;
pub mod login;
pub mod map;

pub use char::{
    ChCharListReqPacket, ChDeleteChar3CancelPacket, ChDeleteChar3ReservedPacket, ChEnterPacket,
    ChMakeCharPacket, ChSelectCharPacket, PingPacket,
};
pub use frame::{
    decode_frame, DecodedFrame, ModernIntentParser, Phase0IntentParser, RathenaIntentParser,
};
pub use helpers::{parse_sex, read_fixed_string};
pub use login::CaLoginPacket;
pub use map::{
    CzChangeDirectionPacket, CzEnterPacket, CzNotifyActorInitPacket, CzReqNamePacket,
    CzRequestActPacket, CzRequestMovePacket, CzRequestTimePacket, CzRestartPacket,
};
