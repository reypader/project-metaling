use ro_core::ClientIntent;

use crate::coords::unpack_pos_dir;
use crate::{PacketCodecError, RawPacket};

/// Decoded representation of `CZ_REQUEST_MOVE` (0x035f).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestMovePacket {
    pub dest_x: u16,
    pub dest_y: u16,
}

impl CzRequestMovePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 3 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 3,
                actual: raw.payload.len(),
            });
        }

        let buf = [raw.payload[0], raw.payload[1], raw.payload[2]];
        let (dest_x, dest_y, _) = unpack_pos_dir(&buf);
        Ok(Self { dest_x, dest_y })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestMove {
            dest_x: self.dest_x,
            dest_y: self.dest_y,
        }
    }
}
