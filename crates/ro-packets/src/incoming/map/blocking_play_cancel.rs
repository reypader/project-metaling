use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_BLOCKING_PLAY_CANCEL` (0x0447): no fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzBlockingPlayCancelPacket;

impl CzBlockingPlayCancelPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::BlockingPlayCancel
    }
}
