use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQUEST_CHAT` (0x00f3): `"<name> : <message>"` with no NUL on
/// the wire. A client that sends one anyway has its text end there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestChatPacket {
    pub text: Vec<u8>,
}

impl CzRequestChatPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        let bytes = raw.payload.as_ref();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(Self {
            text: bytes[..end].to_vec(),
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::Chat {
            text: self.text.clone(),
        }
    }
}
