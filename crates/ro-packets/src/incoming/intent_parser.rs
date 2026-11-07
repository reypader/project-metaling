use ro_core::ClientIntent;

use crate::{decode_frame, DecodedFrame, PacketCodecError, PacketIntentParser, RawPacket};

/// Phase 0 implementation of `PacketIntentParser`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Phase0IntentParser;

impl PacketIntentParser for Phase0IntentParser {
    fn parse_intent(
        &self,
        raw: &RawPacket,
        packetver: u32,
    ) -> Result<Option<ClientIntent>, PacketCodecError> {
        match decode_frame(raw, packetver)? {
            DecodedFrame::Intent(intent) => Ok(Some(intent)),
            DecodedFrame::Unhandled { .. } => Ok(None),
        }
    }
}

/// Modern alias for the intent parser.
pub type ModernIntentParser = Phase0IntentParser;

/// Rathena-compatible alias for the intent parser.
pub type RathenaIntentParser = Phase0IntentParser;
