use ro_core::ClientIntent;

use crate::{PacketCodecError, RawPacket};

/// Head direction: 0 straight, 1 clockwise, 2 counter-clockwise.
const MAX_HEAD_DIR: u16 = 2;
/// Body direction: 0 (N) through 7 (NE).
const MAX_BODY_DIR: u8 = 7;

/// Decoded representation of `CZ_CHANGE_DIRECTION` (0x0361). Out-of-range directions are
/// `InvalidField`, which closes the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzChangeDirectionPacket {
    pub head_dir: u16,
    pub body_dir: u8,
}

impl CzChangeDirectionPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 3 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 3,
                actual: raw.payload.len(),
            });
        }

        // rAthena reads head_dir as one byte at frame offset 2 and never reads offset 3.
        let head_dir = u16::from(raw.payload[0]);
        let body_dir = raw.payload[2];
        if head_dir > MAX_HEAD_DIR {
            return Err(PacketCodecError::InvalidField {
                opcode: raw.opcode,
                field: "head_dir",
                value: head_dir.into(),
            });
        }
        if body_dir > MAX_BODY_DIR {
            return Err(PacketCodecError::InvalidField {
                opcode: raw.opcode,
                field: "body_dir",
                value: body_dir.into(),
            });
        }
        Ok(Self { head_dir, body_dir })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::ChangeDirection {
            head_dir: self.head_dir,
            body_dir: self.body_dir,
        }
    }
}
