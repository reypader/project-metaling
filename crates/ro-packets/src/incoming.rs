//! Incoming client packet decoders supporting PACKETVER 20260219 opcodes.
//!
//! Covers:
//! - `CA_LOGIN` (0x0064 / 0x0277)
//! - `CH_ENTER` (0x0065)
//! - `CH_SELECT_CHAR` (0x0066)
//! - `CZ_ENTER` (0x0436)
//! - `CZ_NOTIFY_ACTORINIT` (0x007d)
//! - `CZ_REQUEST_TIME` (0x0360)
//! - `CZ_REQUEST_MOVE` (0x035f)
//! - `CZ_CHANGE_DIRECTION` (0x0361)
//! - `CZ_REQUEST_ACT` (0x0437)
//! - `CZ_REQNAME` (0x0368)

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

/// Reads a 2-byte little-endian unsigned integer from payload at offset.
///
/// Returns `PacketCodecError::TruncatedPayload` when insufficient bytes remain.
fn read_u16_le(
    payload: &[u8],
    offset: usize,
    opcode: PacketOpcode,
) -> Result<u16, PacketCodecError> {
    let expected = offset + 2;
    if payload.len() < expected {
        return Err(PacketCodecError::TruncatedPayload {
            opcode,
            expected,
            actual: payload.len(),
        });
    }
    let slice: [u8; 2] = [payload[offset], payload[offset + 1]];
    Ok(u16::from_le_bytes(slice))
}

/// Reads a 4-byte little-endian unsigned integer from payload at offset.
///
/// Returns `PacketCodecError::TruncatedPayload` when insufficient bytes remain.
fn read_u32_le(
    payload: &[u8],
    offset: usize,
    opcode: PacketOpcode,
) -> Result<u32, PacketCodecError> {
    let expected = offset + 4;
    if payload.len() < expected {
        return Err(PacketCodecError::TruncatedPayload {
            opcode,
            expected,
            actual: payload.len(),
        });
    }
    let slice: [u8; 4] = [
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ];
    Ok(u32::from_le_bytes(slice))
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
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 15,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        let login_id1 = read_u32_le(&raw.payload, 4, raw.opcode)?;
        let login_id2 = read_u32_le(&raw.payload, 8, raw.opcode)?;
        let client_version = read_u16_le(&raw.payload, 12, raw.opcode)?;
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
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 1,
                actual: raw.payload.len(),
            });
        }

        Ok(Self {
            slot: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::SelectCharacter { slot: self.slot }
    }
}

/// Decoded representation of `CZ_ENTER` (0x0436).
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
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 17,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        let char_id = CharId(read_u32_le(&raw.payload, 4, raw.opcode)?);
        let login_id1 = read_u32_le(&raw.payload, 8, raw.opcode)?;
        let client_tick = read_u32_le(&raw.payload, 12, raw.opcode)?;
        let sex = if raw.payload.len() >= 21 {
            parse_sex(raw.payload[20])
        } else {
            parse_sex(raw.payload[16])
        };

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

/// Decoded representation of `CZ_REQUEST_TIME` (0x0360).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestTimePacket {
    pub client_tick: u32,
}

impl CzRequestTimePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let client_tick = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { client_tick })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestTime {
            client_tick: self.client_tick,
        }
    }
}

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

/// Decoded representation of `CZ_CHANGE_DIRECTION` (0x0361).
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

        let head_dir = read_u16_le(&raw.payload, 0, raw.opcode)?;
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

/// Decoded representation of `CZ_REQUEST_ACT` (0x0437).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRequestActPacket {
    pub target_id: u32,
    pub action_type: u8,
}

impl CzRequestActPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 5 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 5,
                actual: raw.payload.len(),
            });
        }

        let target_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
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

/// Decoded representation of `CZ_REQNAME` (0x0368).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzReqNamePacket {
    pub entity_id: u32,
}

impl CzReqNamePacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let entity_id = read_u32_le(&raw.payload, 0, raw.opcode)?;
        Ok(Self { entity_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestEntityName {
            entity_id: self.entity_id,
        }
    }
}

/// Decoded representation of `CH_MAKE_CHAR` (0x0a39).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChMakeCharPacket {
    pub name: String,
    pub slot: u8,
    pub hair_color: u16,
    pub hair_style: u16,
    pub starting_job: u32,
    pub sex: Sex,
}

impl ChMakeCharPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 34 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 34,
                actual: raw.payload.len(),
            });
        }

        let name = read_fixed_string(&raw.payload[0..24]);
        let slot = raw.payload[24];
        let hair_color = read_u16_le(&raw.payload, 25, raw.opcode)?;
        let hair_style = read_u16_le(&raw.payload, 27, raw.opcode)?;
        let starting_job = read_u32_le(&raw.payload, 29, raw.opcode)?;
        let sex = match raw.payload[33] {
            0 => Sex::Female,
            1 => Sex::Male,
            2 => Sex::Server,
            other => {
                return Err(PacketCodecError::InvalidField {
                    opcode: raw.opcode,
                    field: "sex",
                    value: other as u32,
                });
            }
        };

        Ok(Self {
            name,
            slot,
            hair_color,
            hair_style,
            starting_job,
            sex,
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CreateCharacter {
            name: self.name.clone(),
            slot: self.slot,
            hair_color: self.hair_color,
            hair_style: self.hair_style,
            starting_job: self.starting_job,
            sex: self.sex,
        }
    }
}

/// Decoded representation of `CH_DELETE_CHAR3_RESERVED` (0x0827).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChDeleteChar3ReservedPacket {
    pub char_id: CharId,
}

impl ChDeleteChar3ReservedPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let char_id = CharId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { char_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::ReserveDeleteCharacter {
            char_id: self.char_id,
        }
    }
}

/// Decoded representation of `CH_DELETE_CHAR3_CANCEL` (0x082b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChDeleteChar3CancelPacket {
    pub char_id: CharId,
}

impl ChDeleteChar3CancelPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let char_id = CharId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { char_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CancelDeleteCharacter {
            char_id: self.char_id,
        }
    }
}

/// Decoded representation of `CH_CHARLIST_REQ` (0x09a1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChCharListReqPacket;

impl ChCharListReqPacket {
    pub fn decode(_raw: &RawPacket) -> Result<Self, PacketCodecError> {
        Ok(Self)
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::RequestCharacterList
    }
}

/// Decoded representation of `PING` (0x0187).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingPacket {
    pub account_id: AccountId,
}

impl PingPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.len() < 4 {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 4,
                actual: raw.payload.len(),
            });
        }

        let account_id = AccountId(read_u32_le(&raw.payload, 0, raw.opcode)?);
        Ok(Self { account_id })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::CharPing {
            account_id: self.account_id,
        }
    }
}

/// Decoded representation of `CZ_RESTART` (0x00b2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CzRestartPacket {
    pub restart_type: u8,
}

impl CzRestartPacket {
    pub fn decode(raw: &RawPacket) -> Result<Self, PacketCodecError> {
        if raw.payload.is_empty() {
            return Err(PacketCodecError::TruncatedPayload {
                opcode: raw.opcode,
                expected: 1,
                actual: raw.payload.len(),
            });
        }

        Ok(Self {
            restart_type: raw.payload[0],
        })
    }

    pub fn to_intent(&self) -> ClientIntent {
        ClientIntent::Restart {
            restart_type: self.restart_type,
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
        0x0a39 => {
            let p = ChMakeCharPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0827 => {
            let p = ChDeleteChar3ReservedPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x082b => {
            let p = ChDeleteChar3CancelPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x09a1 => {
            let p = ChCharListReqPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0187 => {
            let p = PingPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x00b2 => {
            let p = CzRestartPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0436 => {
            let p = CzEnterPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x007d => {
            let p = CzNotifyActorInitPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0360 => {
            let p = CzRequestTimePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x035f => {
            let p = CzRequestMovePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0361 => {
            let p = CzChangeDirectionPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0437 => {
            let p = CzRequestActPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0368 => {
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
            0x0a39 => Ok(Some(ChMakeCharPacket::decode(raw)?.to_intent())),
            0x0827 => Ok(Some(ChDeleteChar3ReservedPacket::decode(raw)?.to_intent())),
            0x082b => Ok(Some(ChDeleteChar3CancelPacket::decode(raw)?.to_intent())),
            0x09a1 => Ok(Some(ChCharListReqPacket::decode(raw)?.to_intent())),
            0x0187 => Ok(Some(PingPacket::decode(raw)?.to_intent())),
            0x00b2 => Ok(Some(CzRestartPacket::decode(raw)?.to_intent())),
            0x0436 => Ok(Some(CzEnterPacket::decode(raw)?.to_intent())),
            0x007d => Ok(Some(CzNotifyActorInitPacket::decode(raw)?.to_intent())),
            0x0360 => Ok(Some(CzRequestTimePacket::decode(raw)?.to_intent())),
            0x035f => Ok(Some(CzRequestMovePacket::decode(raw)?.to_intent())),
            0x0361 => Ok(Some(CzChangeDirectionPacket::decode(raw)?.to_intent())),
            0x0437 => Ok(Some(CzRequestActPacket::decode(raw)?.to_intent())),
            0x0368 => Ok(Some(CzReqNamePacket::decode(raw)?.to_intent())),
            _ => Ok(None),
        }
    }
}

/// Modern alias for the intent parser.
pub type ModernIntentParser = Phase0IntentParser;

/// Rathena-compatible alias for the intent parser.
pub type RathenaIntentParser = Phase0IntentParser;

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
    fn test_decode_ca_login_captured_trace_failure_and_retry() {
        let parser = ModernIntentParser::default();
        let ver = ro_contracts::TARGET_PACKETVER;

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
        let mut payload = vec![0u8; 21];
        payload[0..4].copy_from_slice(&2000001u32.to_le_bytes());
        payload[4..8].copy_from_slice(&150001u32.to_le_bytes());
        payload[8..12].copy_from_slice(&11111u32.to_le_bytes());
        payload[12..16].copy_from_slice(&54321u32.to_le_bytes());
        payload[20] = 0; // Female

        let raw = RawPacket::new(PacketOpcode(0x0436), Bytes::from(payload));
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
        let raw = RawPacket::new(PacketOpcode(0x0360), Bytes::from(payload));
        let decoded = CzRequestTimePacket::decode(&raw).unwrap();

        assert_eq!(decoded.client_tick, 98765);
    }

    #[test]
    fn test_decode_cz_request_move() {
        let pos = pack_pos_dir(120, 240, 4);
        let raw = RawPacket::new(PacketOpcode(0x035f), Bytes::copy_from_slice(&pos));
        let decoded = CzRequestMovePacket::decode(&raw).unwrap();

        assert_eq!(decoded.dest_x, 120);
        assert_eq!(decoded.dest_y, 240);
    }

    #[test]
    fn test_phase0_intent_parser() {
        let parser = Phase0IntentParser;
        let ver = ro_contracts::TARGET_PACKETVER;

        let raw_time = RawPacket::new(PacketOpcode(0x0360), Bytes::copy_from_slice(&55555u32.to_le_bytes()));
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

    #[test]
    fn test_modern_client_intent_decoding() {
        let parser = Phase0IntentParser;
        let ver = ro_contracts::TARGET_PACKETVER;

        // 1. CZ_REQUEST_MOVE (0x035f)
        let pos = pack_pos_dir(250, 300, 2);
        let raw_move = RawPacket::new(PacketOpcode(0x035f), Bytes::copy_from_slice(&pos));
        let move_intent = parser.parse_intent(&raw_move, ver).unwrap().unwrap();
        assert_eq!(
            move_intent,
            ClientIntent::RequestMove {
                dest_x: 250,
                dest_y: 300,
            }
        );
        let frame_move = decode_frame(&raw_move, ver).unwrap();
        assert_eq!(frame_move, DecodedFrame::Intent(move_intent));

        // 2. CZ_REQUEST_TIME (0x0360)
        let raw_time = RawPacket::new(PacketOpcode(0x0360), Bytes::copy_from_slice(&77777u32.to_le_bytes()));
        let time_intent = parser.parse_intent(&raw_time, ver).unwrap().unwrap();
        assert_eq!(
            time_intent,
            ClientIntent::RequestTime {
                client_tick: 77777,
            }
        );
        let frame_time = decode_frame(&raw_time, ver).unwrap();
        assert_eq!(frame_time, DecodedFrame::Intent(time_intent));

        // 3. CZ_CHANGE_DIRECTION (0x0361)
        let mut dir_payload = vec![0u8; 3];
        dir_payload[0..2].copy_from_slice(&6u16.to_le_bytes());
        dir_payload[2] = 4;
        let raw_dir = RawPacket::new(PacketOpcode(0x0361), Bytes::from(dir_payload));
        let dir_intent = parser.parse_intent(&raw_dir, ver).unwrap().unwrap();
        assert_eq!(
            dir_intent,
            ClientIntent::ChangeDirection {
                head_dir: 6,
                body_dir: 4,
            }
        );
        let frame_dir = decode_frame(&raw_dir, ver).unwrap();
        assert_eq!(frame_dir, DecodedFrame::Intent(dir_intent));

        // 4. CZ_REQUEST_ACT (0x0437)
        let mut act_payload = vec![0u8; 5];
        act_payload[0..4].copy_from_slice(&100050u32.to_le_bytes());
        act_payload[4] = 2; // sit
        let raw_act = RawPacket::new(PacketOpcode(0x0437), Bytes::from(act_payload));
        let act_intent = parser.parse_intent(&raw_act, ver).unwrap().unwrap();
        assert_eq!(
            act_intent,
            ClientIntent::RequestAction {
                target_id: 100050,
                action_type: 2,
            }
        );
        let frame_act = decode_frame(&raw_act, ver).unwrap();
        assert_eq!(frame_act, DecodedFrame::Intent(act_intent));

        // 5. CZ_REQNAME (0x0368)
        let raw_reqname = RawPacket::new(PacketOpcode(0x0368), Bytes::copy_from_slice(&1002u32.to_le_bytes()));
        let reqname_intent = parser.parse_intent(&raw_reqname, ver).unwrap().unwrap();
        assert_eq!(
            reqname_intent,
            ClientIntent::RequestEntityName {
                entity_id: 1002,
            }
        );
        let frame_reqname = decode_frame(&raw_reqname, ver).unwrap();
        assert_eq!(frame_reqname, DecodedFrame::Intent(reqname_intent));

        // 6. CZ_ENTER (0x0436)
        let mut cz_enter_payload = vec![0u8; 21];
        cz_enter_payload[0..4].copy_from_slice(&2000002u32.to_le_bytes());
        cz_enter_payload[4..8].copy_from_slice(&150002u32.to_le_bytes());
        cz_enter_payload[8..12].copy_from_slice(&33333u32.to_le_bytes());
        cz_enter_payload[12..16].copy_from_slice(&65432u32.to_le_bytes());
        cz_enter_payload[20] = 1; // Male
        let raw_enter = RawPacket::new(PacketOpcode(0x0436), Bytes::from(cz_enter_payload));
        let enter_intent = parser.parse_intent(&raw_enter, ver).unwrap().unwrap();
        assert_eq!(
            enter_intent,
            ClientIntent::EnterMap {
                account_id: AccountId(2000002),
                char_id: CharId(150002),
                login_id1: 33333,
                client_tick: 65432,
                sex: Sex::Male,
            }
        );
    }

    #[test]
    fn test_character_and_session_incoming_packets() {
        let parser = Phase0IntentParser;
        let ver = ro_contracts::TARGET_PACKETVER;

        // 1. CH_MAKE_CHAR (0x0a39) from login_sequence.log
        let make_char_bytes: [u8; 36] = [
            0x39, 0x0a, 0x74, 0x65, 0x73, 0x74, 0x35, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let raw_make = RawPacket::new(PacketOpcode(0x0a39), Bytes::copy_from_slice(&make_char_bytes[2..]));
        let intent_make = parser.parse_intent(&raw_make, ver).unwrap().unwrap();
        assert_eq!(
            intent_make,
            ClientIntent::CreateCharacter {
                name: "test5".to_string(),
                slot: 4,
                hair_color: 0,
                hair_style: 22,
                starting_job: 0,
                sex: Sex::Female,
            }
        );
        let frame_make = decode_frame(&raw_make, ver).unwrap();
        assert_eq!(frame_make, DecodedFrame::Intent(intent_make));

        // 2. CH_DELETE_CHAR3_RESERVED (0x0827) from login_sequence.log
        let del_res_bytes: [u8; 6] = [0x27, 0x08, 0xf4, 0x49, 0x02, 0x00];
        let raw_del_res = RawPacket::new(PacketOpcode(0x0827), Bytes::copy_from_slice(&del_res_bytes[2..]));
        let intent_del_res = parser.parse_intent(&raw_del_res, ver).unwrap().unwrap();
        assert_eq!(
            intent_del_res,
            ClientIntent::ReserveDeleteCharacter {
                char_id: CharId(150004),
            }
        );
        let frame_del_res = decode_frame(&raw_del_res, ver).unwrap();
        assert_eq!(frame_del_res, DecodedFrame::Intent(intent_del_res));

        // 3. CH_DELETE_CHAR3_CANCEL (0x082b)
        let del_cancel_bytes: [u8; 6] = [0x2b, 0x08, 0xf4, 0x49, 0x02, 0x00];
        let raw_del_cancel = RawPacket::new(PacketOpcode(0x082b), Bytes::copy_from_slice(&del_cancel_bytes[2..]));
        let intent_del_cancel = parser.parse_intent(&raw_del_cancel, ver).unwrap().unwrap();
        assert_eq!(
            intent_del_cancel,
            ClientIntent::CancelDeleteCharacter {
                char_id: CharId(150004),
            }
        );
        let frame_del_cancel = decode_frame(&raw_del_cancel, ver).unwrap();
        assert_eq!(frame_del_cancel, DecodedFrame::Intent(intent_del_cancel));

        // 4. CH_CHARLIST_REQ (0x09a1)
        let raw_charlist = RawPacket::new(PacketOpcode(0x09a1), Bytes::new());
        let intent_charlist = parser.parse_intent(&raw_charlist, ver).unwrap().unwrap();
        assert_eq!(intent_charlist, ClientIntent::RequestCharacterList);
        let frame_charlist = decode_frame(&raw_charlist, ver).unwrap();
        assert_eq!(frame_charlist, DecodedFrame::Intent(intent_charlist));

        // 5. PING (0x0187)
        let ping_bytes: [u8; 6] = [0x87, 0x01, 0x01, 0x00, 0x1f, 0x00];
        let raw_ping = RawPacket::new(PacketOpcode(0x0187), Bytes::copy_from_slice(&ping_bytes[2..]));
        let intent_ping = parser.parse_intent(&raw_ping, ver).unwrap().unwrap();
        assert_eq!(
            intent_ping,
            ClientIntent::CharPing {
                account_id: AccountId(2031617),
            }
        );
        let frame_ping = decode_frame(&raw_ping, ver).unwrap();
        assert_eq!(frame_ping, DecodedFrame::Intent(intent_ping));

        // 6. CZ_RESTART (0x00b2) from login_sequence.log
        let restart_bytes: [u8; 3] = [0xb2, 0x00, 0x01];
        let raw_restart = RawPacket::new(PacketOpcode(0x00b2), Bytes::copy_from_slice(&restart_bytes[2..]));
        let intent_restart = parser.parse_intent(&raw_restart, ver).unwrap().unwrap();
        assert_eq!(
            intent_restart,
            ClientIntent::Restart {
                restart_type: 1,
            }
        );
        let frame_restart = decode_frame(&raw_restart, ver).unwrap();
        assert_eq!(frame_restart, DecodedFrame::Intent(intent_restart));
    }

    #[test]
    fn test_ch_make_char_invalid_sex() {
        let mut payload = vec![0u8; 34];
        payload[0..4].copy_from_slice(b"Hero");
        payload[24] = 0;
        payload[25..27].copy_from_slice(&1u16.to_le_bytes());
        payload[27..29].copy_from_slice(&2u16.to_le_bytes());
        payload[29..33].copy_from_slice(&0u32.to_le_bytes());
        payload[33] = 99;

        let raw = RawPacket::new(PacketOpcode(0x0a39), Bytes::from(payload));
        match ChMakeCharPacket::decode(&raw) {
            Err(PacketCodecError::InvalidField { opcode, field, value }) => {
                assert_eq!(opcode, PacketOpcode(0x0a39));
                assert_eq!(field, "sex");
                assert_eq!(value, 99);
            }
            other => panic!("Expected InvalidField error, got {:?}", other),
        }
    }

    #[test]
    fn test_truncated_payload_errors() {
        let short_payload = Bytes::from_static(&[0u8; 2]);

        let raw_ch_enter = RawPacket::new(PacketOpcode(0x0065), short_payload.clone());
        match ChEnterPacket::decode(&raw_ch_enter) {
            Err(PacketCodecError::TruncatedPayload { opcode, expected, actual }) => {
                assert_eq!(opcode, PacketOpcode(0x0065));
                assert_eq!(expected, 15);
                assert_eq!(actual, 2);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }

        let raw_cz_enter = RawPacket::new(PacketOpcode(0x0436), short_payload.clone());
        match CzEnterPacket::decode(&raw_cz_enter) {
            Err(PacketCodecError::TruncatedPayload { opcode, expected, actual }) => {
                assert_eq!(opcode, PacketOpcode(0x0436));
                assert_eq!(expected, 17);
                assert_eq!(actual, 2);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }

        let raw_time = RawPacket::new(PacketOpcode(0x0360), short_payload);
        match CzRequestTimePacket::decode(&raw_time) {
            Err(PacketCodecError::TruncatedPayload { opcode, expected, actual }) => {
                assert_eq!(opcode, PacketOpcode(0x0360));
                assert_eq!(expected, 4);
                assert_eq!(actual, 2);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }

        let empty_payload = Bytes::new();
        let raw_select = RawPacket::new(PacketOpcode(0x0066), empty_payload);
        match ChSelectCharPacket::decode(&raw_select) {
            Err(PacketCodecError::TruncatedPayload { opcode, expected, actual }) => {
                assert_eq!(opcode, PacketOpcode(0x0066));
                assert_eq!(expected, 1);
                assert_eq!(actual, 0);
            }
            other => panic!("Expected TruncatedPayload error, got {:?}", other),
        }
    }
}
