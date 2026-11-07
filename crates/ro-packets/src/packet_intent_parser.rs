use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Trait defining the parser contract that translates a `RawPacket` into a high-level `ClientIntent`.
pub trait PacketIntentParser: Send + Sync {
    fn parse_intent(
        &self,
        raw: &RawPacket,
        packetver: u32,
    ) -> Result<Option<ClientIntent>, PacketCodecError>;
}
