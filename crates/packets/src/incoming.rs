//! Phase 0 incoming client packet decoders.
//!
//! Covers:
//! - `CA_LOGIN` (0x0064 / 0x0277)
//! - `CH_ENTER` (0x0065)
//! - `CH_SELECT_CHAR` (0x0066)
//! - `CZ_ENTER` (0x0072)
//! - `CZ_NOTIFY_ACTORINIT` (0x007d)
//! - `CZ_REQUEST_TIME` (0x007e)
//! - `CZ_REQUEST_MOVE` (0x0085)
//! - `CZ_CHANGE_DIRECTION` (0x009b)
//! - `CZ_REQUEST_ACT` (0x0089)
//! - `CZ_REQNAME` (0x0094)

use ro_contracts::ids::{AccountId, CharId, Sex};
use ro_contracts::packet::{ClientIntent, PacketCodecError, PacketIntentParser, PacketOpcode, RawPacket};

use crate::coords::unpack_pos_dir;
use crate::table::lookup_packet_length;

/// Reads a null-terminated or null-padded ASCII/UTF-8 string from fixed byte slice.
pub fn read_fixed_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// Helper converting raw wire byte to domain Sex enum.
pub fn parse_sex(byte: u8) -> Sex {
    match byte {
        0 => Sex::Female,
        1 => Sex::Male,
        _ => Sex::Server,
    }
}

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
                    return Err(PacketCodecError::MalformedHeader(format!(
                        "CA_LOGIN payload expected at least 53 bytes, got {}",
                        raw.payload.len()
                    )));
                }

                let version = u32::from_le_bytes(raw.payload[0..4].try_into().unwrap());
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
                    return Err(PacketCodecError::MalformedHeader(format!(
                        "CA_LOGIN_PCBANG payload expected at least 82 bytes, got {}",
                        raw.payload.len()
                    )));
                }

                let version = u32::from_le_bytes(raw.payload[0..4].try_into().unwrap());
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

/// Decoded representation of `CH_ENTER` (0x0065).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChEnterPacket {
    pub account_id: AccountId,
    pub login_id1: u32,
    pub login_id2: u32,
    pub client_version: u16,
    pub sex: Sex,
}

impl ChEnterPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 15 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CH_ENTER payload expected 15 bytes, got {}",
                raw.payload.len()
            )));
        }

        let account_id = AccountId(u32::from_le_bytes(raw.payload[0..4].try_into().unwrap()));
        let login_id1 = u32::from_le_bytes(raw.payload[4..8].try_into().unwrap());
        let login_id2 = u32::from_le_bytes(raw.payload[8..12].try_into().unwrap());
        let client_version = u16::from_le_bytes(raw.payload[12..14].try_into().unwrap());
        let sex = parse_sex(raw.payload[14]);

        Ok(Self {
            account_id,
            login_id1,
            login_id2,
            client_version,
            sex,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::EnterCharServer {
            account_id: self.account_id,
            login_id1: self.login_id1,
            login_id2: self.login_id2,
            client_version: self.client_version,
            sex: self.sex,
        }
    }
}

/// Decoded representation of `CH_SELECT_CHAR` (0x0066).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChSelectCharPacket {
    pub slot: u8,
}

impl ChSelectCharPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.is_empty() {
            return Err(PacketCodecError::MalformedHeader(
                "CH_SELECT_CHAR payload expected at least 1 byte".to_string(),
            ));
        }

        Ok(Self {
            slot: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::SelectCharacter { slot: self.slot }
    }
}

/// Decoded representation of `CZ_ENTER` (0x0072).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzEnterPacket {
    pub account_id: AccountId,
    pub char_id: CharId,
    pub login_id1: u32,
    pub client_tick: u32,
    pub sex: Sex,
}

impl CzEnterPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 17 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CZ_ENTER payload expected 17 bytes, got {}",
                raw.payload.len()
            )));
        }

        let account_id = AccountId(u32::from_le_bytes(raw.payload[0..4].try_into().unwrap()));
        let char_id = CharId(u32::from_le_bytes(raw.payload[4..8].try_into().unwrap()));
        let login_id1 = u32::from_le_bytes(raw.payload[8..12].try_into().unwrap());
        let client_tick = u32::from_le_bytes(raw.payload[12..16].try_into().unwrap());
        let sex = parse_sex(raw.payload[16]);

        Ok(Self {
            account_id,
            char_id,
            login_id1,
            client_tick,
            sex,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::EnterMap {
            account_id: self.account_id,
            char_id: self.char_id,
            login_id1: self.login_id1,
            client_tick: self.client_tick,
            sex: self.sex,
        }
    }
}

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

/// Decoded representation of `CZ_REQUEST_TIME` (0x007e).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestTimePacket {
    pub client_tick: u32,
}

impl CzRequestTimePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CZ_REQUEST_TIME payload expected 4 bytes, got {}",
                raw.payload.len()
            )));
        }

        let client_tick = u32::from_le_bytes(raw.payload[0..4].try_into().unwrap());
        Ok(Self { client_tick })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestTime {
            client_tick: self.client_tick,
        }
    }
}

/// Decoded representation of `CZ_REQUEST_MOVE` (0x0085).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestMovePacket {
    pub dest_x: u16,
    pub dest_y: u16,
}

impl CzRequestMovePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 3 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CZ_REQUEST_MOVE payload expected 3 bytes, got {}",
                raw.payload.len()
            )));
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

/// Decoded representation of `CZ_CHANGE_DIRECTION` (0x009b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzChangeDirectionPacket {
    pub head_dir: u16,
    pub body_dir: u8,
}

impl CzChangeDirectionPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 3 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CZ_CHANGE_DIRECTION payload expected 3 bytes, got {}",
                raw.payload.len()
            )));
        }

        let head_dir = u16::from_le_bytes(raw.payload[0..2].try_into().unwrap());
        let body_dir = raw.payload[2];
        Ok(Self { head_dir, body_dir })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::ChangeDirection {
            head_dir: self.head_dir,
            body_dir: self.body_dir,
        }
    }
}

/// Decoded representation of `CZ_REQUEST_ACT` (0x0089).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestActPacket {
    pub target_id: u32,
    pub action_type: u8,
}

impl CzRequestActPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 5 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CZ_REQUEST_ACT payload expected 5 bytes, got {}",
                raw.payload.len()
            )));
        }

        let target_id = u32::from_le_bytes(raw.payload[0..4].try_into().unwrap());
        let action_type = raw.payload[4];
        Ok(Self {
            target_id,
            action_type,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestAction {
            target_id: self.target_id,
            action_type: self.action_type,
        }
    }
}

/// Decoded representation of `CZ_REQNAME` (0x0094).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzReqNamePacket {
    pub entity_id: u32,
}

impl CzReqNamePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::MalformedHeader(format!(
                "CZ_REQNAME payload expected 4 bytes, got {}",
                raw.payload.len()
            )));
        }

        let entity_id = u32::from_le_bytes(raw.payload[0..4].try_into().unwrap());
        Ok(Self { entity_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestEntityName {
            entity_id: self.entity_id,
        }
    }
}

/// Enumeration of all recognized decoded client frames across login, char, and map servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedFrame {
    CaLogin(CaLoginPacket),
    ChEnter(ChEnterPacket),
    ChSelectChar(ChSelectCharPacket),
    Intent(ClientIntent),
    Unhandled { opcode: PacketOpcode, len: usize },
}

/// Decodes any incoming `RawPacket` into a high-level `DecodedFrame`.
///
/// If the opcode is recognized by packetdb but not handled in Phase 0 domain logic,
/// returns `DecodedFrame::Unhandled { opcode, len }` to allow connection persistence.
pub fn decode_frame(raw: &RawPacket, packetver: u32) -> Result<DecodedFrame, PacketCodecError> {
    match raw.opcode.raw() {
        0x0064 | 0x0277 => {
            let p = CaLoginPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0065 => {
            let p = ChEnterPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0066 => {
            let p = ChSelectCharPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0072 => {
            let p = CzEnterPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x007d => {
            let p = CzNotifyActorInitPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x007e => {
            let p = CzRequestTimePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0085 => {
            let p = CzRequestMovePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x009b => {
            let p = CzChangeDirectionPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0089 => {
            let p = CzRequestActPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0094 => {
            let p = CzReqNamePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        _ => {
            let total_len = match lookup_packet_length(raw.opcode, packetver) {
                Some(ro_contracts::packet::PacketLength::Fixed(len)) => len,
                Some(ro_contracts::packet::PacketLength::Dynamic) => raw.payload.len() + 4,
                None => raw.payload.len() + 2,
            };
            Ok(DecodedFrame::Unhandled {
                opcode: raw.opcode,
                len: total_len,
            })
        }
    }
}

/// Domain intent parser implementing contracts trait.
#[derive(Debug, Default, Clone, Copy)]
pub struct Phase0IntentParser;

impl PacketIntentParser for Phase0IntentParser {
    fn parse_intent(
        &self,
        raw: &RawPacket,
        _packetver: u32,
    ) -> Result<Option<ClientIntent>, PacketCodecError> {
        match raw.opcode.raw() {
            0x0064 | 0x0277 => Ok(Some(CaLoginPacket::decode(raw)?.to_intent())),
            0x0065 => Ok(Some(ChEnterPacket::decode(raw)?.to_intent())),
            0x0066 => Ok(Some(ChSelectCharPacket::decode(raw)?.to_intent())),
            0x0072 => Ok(Some(CzEnterPacket::decode(raw)?.to_intent())),
            0x007d => Ok(Some(CzNotifyActorInitPacket::decode(raw)?.to_intent())),
            0x007e => Ok(Some(CzRequestTimePacket::decode(raw)?.to_intent())),
            0x0085 => Ok(Some(CzRequestMovePacket::decode(raw)?.to_intent())),
            0x009b => Ok(Some(CzChangeDirectionPacket::decode(raw)?.to_intent())),
            0x0089 => Ok(Some(CzRequestActPacket::decode(raw)?.to_intent())),
            0x0094 => Ok(Some(CzReqNamePacket::decode(raw)?.to_intent())),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use crate::coords::pack_pos_dir;

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

    #[test]
    fn test_decode_ch_enter() {
        let mut payload = vec![0u8; 15];
        payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        payload[4..8].copy_from_slice(&11111u32.to_le_bytes());
        payload[8..12].copy_from_slice(&22222u32.to_le_bytes());
        payload[12..14].copy_from_slice(&20u16.to_le_bytes());
        payload[14] = 1; // Male

        let raw = RawPacket::new(PacketOpcode(0x0065), Bytes::from(payload));
        let decoded = ChEnterPacket::decode(&raw).unwrap();

        assert_eq!(decoded.account_id, AccountId(2000001));
        assert_eq!(decoded.login_id1, 11111);
        assert_eq!(decoded.login_id2, 22222);
        assert_eq!(decoded.client_version, 20);
        assert_eq!(decoded.sex, Sex::Male);
    }

    #[test]
    fn test_decode_ch_select_char() {
        let payload = vec![3u8];
        let raw = RawPacket::new(PacketOpcode(0x0066), Bytes::from(payload));
        let decoded = ChSelectCharPacket::decode(&raw).unwrap();

        assert_eq!(decoded.slot, 3);
    }

    #[test]
    fn test_decode_cz_enter() {
        let mut payload = vec![0u8; 17];
        payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        payload[4..8].copy_from_slice(&150001u32.to_le_bytes());
        payload[8..12].copy_from_slice(&11111u32.to_le_bytes());
        payload[12..16].copy_from_slice(&54321u32.to_le_bytes());
        payload[16] = 0; // Female

        let raw = RawPacket::new(PacketOpcode(0x0072), Bytes::from(payload));
        let decoded = CzEnterPacket::decode(&raw).unwrap();

        assert_eq!(decoded.account_id, AccountId(2000001));
        assert_eq!(decoded.char_id, CharId(150001));
        assert_eq!(decoded.login_id1, 11111);
        assert_eq!(decoded.client_tick, 54321);
        assert_eq!(decoded.sex, Sex::Female);

        let intent = decoded.to_intent();
        assert_eq!(
            intent,
            ClientIntent::EnterMap {
                account_id: AccountId(2000001),
                char_id: CharId(150001),
                login_id1: 11111,
                client_tick: 54321,
                sex: Sex::Female,
            }
        );
    }

    #[test]
    fn test_decode_cz_request_time() {
        let payload = 98765u32.to_le_bytes().to_vec();
        let raw = RawPacket::new(PacketOpcode(0x007e), Bytes::from(payload));
        let decoded = CzRequestTimePacket::decode(&raw).unwrap();

        assert_eq!(decoded.client_tick, 98765);
    }

    #[test]
    fn test_decode_cz_request_move() {
        let pos = pack_pos_dir(120, 240, 4);
        let raw = RawPacket::new(PacketOpcode(0x0085), Bytes::copy_from_slice(&pos));
        let decoded = CzRequestMovePacket::decode(&raw).unwrap();

        assert_eq!(decoded.dest_x, 120);
        assert_eq!(decoded.dest_y, 240);
    }

    #[test]
    fn test_phase0_intent_parser() {
        let parser = Phase0IntentParser;
        let ver = ro_contracts::TARGET_PACKETVER;

        let raw_time = RawPacket::new(PacketOpcode(0x007e), Bytes::copy_from_slice(&55555u32.to_le_bytes()));
        let intent = parser.parse_intent(&raw_time, ver).unwrap().unwrap();
        assert_eq!(
            intent,
            ClientIntent::RequestTime {
                client_tick: 55555,
            }
        );

        let raw_unknown = RawPacket::new(PacketOpcode(0x009f), Bytes::copy_from_slice(&[1, 2, 3, 4]));
        assert_eq!(parser.parse_intent(&raw_unknown, ver).unwrap(), None);

        // Test CA_LOGIN intent parsing
        let mut login_payload = vec![0u8; 53];
        login_payload[0..4].copy_from_slice(&20211103u32.to_le_bytes());
        login_payload[4..12].copy_from_slice(b"alice123");
        login_payload[28..36].copy_from_slice(b"password");
        login_payload[52] = 1;
        let raw_login = RawPacket::new(PacketOpcode(0x0064), Bytes::from(login_payload));
        let login_intent = parser.parse_intent(&raw_login, ver).unwrap().unwrap();
        assert_eq!(
            login_intent,
            ClientIntent::Login {
                username: "alice123".to_string(),
                password: "password".to_string(),
                client_version: 20211103,
                client_type: 1,
            }
        );
        let frame_login = decode_frame(&raw_login, ver).unwrap();
        assert_eq!(frame_login, DecodedFrame::Intent(login_intent));

        // Test CH_ENTER intent parsing
        let mut enter_payload = vec![0u8; 15];
        enter_payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        enter_payload[4..8].copy_from_slice(&11111u32.to_le_bytes());
        enter_payload[8..12].copy_from_slice(&22222u32.to_le_bytes());
        enter_payload[12..14].copy_from_slice(&20u16.to_le_bytes());
        enter_payload[14] = 1;
        let raw_enter = RawPacket::new(PacketOpcode(0x0065), Bytes::from(enter_payload));
        let enter_intent = parser.parse_intent(&raw_enter, ver).unwrap().unwrap();
        assert_eq!(
            enter_intent,
            ClientIntent::EnterCharServer {
                account_id: AccountId(2000001),
                login_id1: 11111,
                login_id2: 22222,
                client_version: 20,
                sex: Sex::Male,
            }
        );
        let frame_enter = decode_frame(&raw_enter, ver).unwrap();
        assert_eq!(frame_enter, DecodedFrame::Intent(enter_intent));

        // Test CH_SELECT_CHAR intent parsing
        let select_payload = vec![2u8];
        let raw_select = RawPacket::new(PacketOpcode(0x0066), Bytes::from(select_payload));
        let select_intent = parser.parse_intent(&raw_select, ver).unwrap().unwrap();
        assert_eq!(select_intent, ClientIntent::SelectCharacter { slot: 2 });
        let frame_select = decode_frame(&raw_select, ver).unwrap();
        assert_eq!(frame_select, DecodedFrame::Intent(select_intent));
    }
}
