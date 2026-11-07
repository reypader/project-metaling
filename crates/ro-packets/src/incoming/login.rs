//! Incoming login client packet decoders supporting CA_LOGIN opcodes.

use crate::{PacketCodecError, PacketOpcode, RawPacket};
use ro_core::ClientIntent;

use crate::incoming::helpers::{read_fixed_string, read_u32_le};

/// Decoded representation of `CA_LOGIN` (0x0064) or `CA_LOGIN_PCBANG` (0x0277).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaLoginPacket {
    pub opcode: PacketOpcode,
    pub version: u32,
    pub username: String,
    pub password: String,
    pub client_type: u8,
    pub ip: Option<String>,
    pub mac: Option<String>,
}

impl CaLoginPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        match raw.opcode.raw() {
            0x0064 => {
                if raw.payload.len() < 53 {
                    return Err(PacketCodecError::TruncatedPayload {
                        opcode: raw.opcode,
                        expected: 53,
                        actual: raw.payload.len(),
                    });
                }

                let version = read_u32_le(&raw.payload, 0, raw.opcode)?;
                let username = read_fixed_string(&raw.payload[4..28]);
                let password = read_fixed_string(&raw.payload[28..52]);
                let client_type = raw.payload[52];

                Ok(Self {
                    opcode: raw.opcode,
                    version,
                    username,
                    password,
                    client_type,
                    ip: None,
                    mac: None,
                })
            }
            0x0277 => {
                if raw.payload.len() < 82 {
                    return Err(PacketCodecError::TruncatedPayload {
                        opcode: raw.opcode,
                        expected: 82,
                        actual: raw.payload.len(),
                    });
                }

                let version = read_u32_le(&raw.payload, 0, raw.opcode)?;
                let username = read_fixed_string(&raw.payload[4..28]);
                let password = read_fixed_string(&raw.payload[28..52]);
                let client_type = raw.payload[52];
                let ip = Some(read_fixed_string(&raw.payload[53..69]));
                let mac = Some(read_fixed_string(&raw.payload[69..82]));

                Ok(Self {
                    opcode: raw.opcode,
                    version,
                    username,
                    password,
                    client_type,
                    ip,
                    mac,
                })
            }
            _ => Err(PacketCodecError::MalformedHeader(format!(
                "Invalid opcode {} for CaLoginPacket",
                raw.opcode
            ))),
        }
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::Login {
            username: self.username.clone(),
            password: self.password.clone(),
            client_version: self.version,
            client_type: self.client_type,
        }
    }
}

#[cfg(test)]
mod tests;
