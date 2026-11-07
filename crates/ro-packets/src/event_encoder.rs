use bytes::Bytes;
use ro_core::ServerEvent;

use crate::PacketCodecError;

/// Trait defining the serializer contract that translates a domain `ServerEvent` into wire `Bytes`.
pub trait EventEncoder: Send + Sync {
    fn encode_event(&self, event: &ServerEvent, packetver: u32) -> Result<Bytes, PacketCodecError>;
}
