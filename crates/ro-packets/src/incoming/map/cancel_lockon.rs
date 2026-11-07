use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_CANCEL_LOCKON` (0x0118): no fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CzCancelLockonPacket;

impl CzCancelLockonPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CancelAttack
    }
}
