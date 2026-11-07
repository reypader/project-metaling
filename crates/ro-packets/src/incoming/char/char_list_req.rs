use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CH_CHARLIST_REQ` (0x09a1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChCharListReqPacket;

impl ChCharListReqPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestCharacterList
    }
}
