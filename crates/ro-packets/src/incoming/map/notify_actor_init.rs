use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_NOTIFY_ACTORINIT` (0x007d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzNotifyActorInitPacket;

impl CzNotifyActorInitPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::NotifyActorInit
    }
}
