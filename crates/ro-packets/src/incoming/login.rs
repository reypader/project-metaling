//! Incoming login client packet decoders supporting CA_LOGIN opcodes.

use crate::{PacketCodecError, PacketOpcode, RawPacket};
use ro_core::message::ClientIntent;

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
mod tests {
    use super::*;
    use crate::PacketIntentParser;
    use bytes::Bytes;
    use ro_core::TARGET_PACKETVER;

    use crate::incoming::frame::Phase0IntentParser;

    #[test]
    fn test_decode_ca_login_0064() {
        let mut payload = vec![0u8; 53];
        payload[0..4].copy_from_slice(&20211103u32.to_le_bytes());
        payload[4..14].copy_from_slice(b"testuser\0\0");
        payload[28..38].copy_from_slice(b"secretpass");
        payload[52] = 1;

        let raw = RawPacket::new(PacketOpcode(0x0064), Bytes::from(payload));
        let decoded = CaLoginPacket::decode(&raw).unwrap();

        assert_eq!(decoded.version, 20211103);
        assert_eq!(decoded.username, "testuser");
        assert_eq!(decoded.password, "secretpass");
        assert_eq!(decoded.client_type, 1);
        assert_eq!(decoded.ip, None);
    }

    #[test]
    fn test_decode_ca_login_captured_trace_failure_and_retry() {
        let parser = Phase0IntentParser;
        let ver = TARGET_PACKETVER;

        let mut payload_fail = vec![0u8; 53];
        payload_fail[0..4].copy_from_slice(&0x80000000u32.to_le_bytes());
        payload_fail[4..8].copy_from_slice(b"test");
        payload_fail[28..37].copy_from_slice(b"passwords");
        payload_fail[52] = 0;

        let raw_fail = RawPacket::new(PacketOpcode(0x0064), Bytes::from(payload_fail));
        let decoded_fail = CaLoginPacket::decode(&raw_fail).unwrap();
        assert_eq!(decoded_fail.version, 0x80000000);
        assert_eq!(decoded_fail.username, "test");
        assert_eq!(decoded_fail.password, "passwords");
        assert_eq!(decoded_fail.client_type, 0);

        let intent_fail = parser.parse_intent(&raw_fail, ver).unwrap().unwrap();
        assert_eq!(
            intent_fail,
            ClientIntent::Login {
                username: "test".to_string(),
                password: "passwords".to_string(),
                client_version: 0x80000000,
                client_type: 0,
            }
        );

        let mut payload_success = vec![0u8; 53];
        payload_success[0..4].copy_from_slice(&0x80000000u32.to_le_bytes());
        payload_success[4..8].copy_from_slice(b"test");
        payload_success[28..36].copy_from_slice(b"password");
        payload_success[52] = 0;

        let raw_success = RawPacket::new(PacketOpcode(0x0064), Bytes::from(payload_success));
        let decoded_success = CaLoginPacket::decode(&raw_success).unwrap();
        assert_eq!(decoded_success.version, 0x80000000);
        assert_eq!(decoded_success.username, "test");
        assert_eq!(decoded_success.password, "password");
        assert_eq!(decoded_success.client_type, 0);

        let intent_success = parser.parse_intent(&raw_success, ver).unwrap().unwrap();
        assert_eq!(
            intent_success,
            ClientIntent::Login {
                username: "test".to_string(),
                password: "password".to_string(),
                client_version: 0x80000000,
                client_type: 0,
            }
        );
    }

    #[test]
    fn test_decode_ca_login_pcbang_0277() {
        let mut payload = vec![0u8; 82];
        payload[0..4].copy_from_slice(&20211103u32.to_le_bytes());
        payload[4..12].copy_from_slice(b"pcbang_u");
        payload[28..36].copy_from_slice(b"pcbang_p");
        payload[52] = 2;
        payload[53..62].copy_from_slice(b"127.0.0.1");
        payload[69..81].copy_from_slice(b"001122334455");

        let raw = RawPacket::new(PacketOpcode(0x0277), Bytes::from(payload));
        let decoded = CaLoginPacket::decode(&raw).unwrap();

        assert_eq!(decoded.version, 20211103);
        assert_eq!(decoded.username, "pcbang_u");
        assert_eq!(decoded.password, "pcbang_p");
        assert_eq!(decoded.client_type, 2);
        assert_eq!(decoded.ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(decoded.mac.as_deref(), Some("001122334455"));
    }
}
