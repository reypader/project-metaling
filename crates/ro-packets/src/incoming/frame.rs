//! High-level client frame decoding and intent parsing.

use crate::{PacketCodecError, PacketLength, PacketOpcode, RawPacket};
use ro_core::ClientIntent;

use crate::table::lookup_packet_length;

use super::char::{
    ChCharListReqPacket, ChDeleteChar3CancelPacket, ChDeleteChar3ReservedPacket, ChEnterPacket,
    ChMakeCharPacket, ChSelectCharPacket, PingPacket,
};
use super::login::CaLoginPacket;
use super::map::{
    CzAdvancedStatusChangePacket, CzBlockingPlayCancelPacket, CzCancelLockonPacket,
    CzChangeDirectionPacket, CzEnterPacket, CzItemPickupPacket, CzItemThrowPacket,
    CzNotifyActorInitPacket, CzReqDisconnectPacket, CzReqNamePacket, CzReqTakeoffEquipAllPacket,
    CzReqTakeoffEquipPacket, CzReqWearEquipPacket, CzRequestActPacket, CzRequestChatPacket,
    CzRequestMovePacket, CzRequestTimePacket, CzRestartPacket, CzStatusChangePacket,
    CzUpgradeSkillLevelPacket, CzUseItemPacket, CzUseSkillToIdPacket,
};

/// Enumeration of all recognized decoded client frames across login, char, and map servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedFrame {
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
        0x018a => {
            let p = CzReqDisconnectPacket::decode(raw)?;
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
        0x0447 => {
            let p = CzBlockingPlayCancelPacket::decode(raw)?;
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
        0x0362 => {
            let p = CzItemPickupPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0363 => {
            let p = CzItemThrowPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x00f3 => {
            let p = CzRequestChatPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0998 => {
            let p = CzReqWearEquipPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x00ab => {
            let p = CzReqTakeoffEquipPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0bf5 => {
            let p = CzReqTakeoffEquipAllPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x00bb => {
            let p = CzStatusChangePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0b24 => {
            let p = CzAdvancedStatusChangePacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0112 => {
            let p = CzUpgradeSkillLevelPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0118 => {
            let p = CzCancelLockonPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0438 => {
            let p = CzUseSkillToIdPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        0x0439 => {
            let p = CzUseItemPacket::decode(raw)?;
            Ok(DecodedFrame::Intent(p.to_intent()))
        }
        _ => {
            let total_len = match lookup_packet_length(raw.opcode, packetver) {
                Some(PacketLength::Fixed(len)) => len,
                Some(PacketLength::Dynamic) => raw.payload.len() + 4,
                None => raw.payload.len() + 2,
            };
            Ok(DecodedFrame::Unhandled {
                opcode: raw.opcode,
                len: total_len,
            })
        }
    }
}

#[cfg(test)]
mod tests;
